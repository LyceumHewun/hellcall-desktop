use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use cpal::traits::StreamTrait;
use log::debug;
use vosk::{Model, Recognizer};
use webrtc_vad::{SampleRate, Vad, VadMode};

use super::microphone::{
    default_input_device_name, open_input_stream, run_processed_audio_pipeline,
    MicPassthroughHandle,
};

static VOSK_SAMPLE_RATE: f32 = 16000.0;

#[derive(Debug, Clone)]
pub struct AudioRecognizerConfig {
    /// 音频识别的时间段 (秒)
    pub chunk_time: f32,
    /// 音频识别的语法字典
    pub grammar: Vec<String>,
    /// 语音结束后的静音持续时间 (ms)
    pub vad_silence_duration: u64,
    /// 是否为按键说话模式
    pub is_ptt: bool,
}

impl Default for AudioRecognizerConfig {
    fn default() -> Self {
        Self {
            chunk_time: 0.2,
            grammar: Vec::new(),
            vad_silence_duration: 500,
            is_ptt: false,
        }
    }
}

impl AudioRecognizerConfig {
    pub fn set_grammar(&mut self, grammar: Vec<String>) {
        self.grammar = grammar;
    }
}

#[derive(Debug, Clone)]
pub struct RecognitionResult {
    pub text: String,
    pub is_partial: bool,
}

pub struct AudioRecognizer {
    model: Arc<Model>,
    recognizer: Recognizer,
    pub config: AudioRecognizerConfig,
    is_speaking: Arc<AtomicBool>,
    silence_start: Mutex<Option<std::time::Instant>>,
    is_finalized: Arc<AtomicBool>,
    audio_cache: Vec<i16>,
    max_cache_samples: usize,
}

impl Clone for AudioRecognizer {
    fn clone(&self) -> Self {
        let recognizer = if self.config.grammar.is_empty() {
            Recognizer::new(&self.model, VOSK_SAMPLE_RATE)
                .expect("Failed to create Vosk recognizer")
        } else {
            Recognizer::new_with_grammar(&self.model, VOSK_SAMPLE_RATE, &self.config.grammar)
                .expect("Failed to create Vosk recognizer")
        };

        Self {
            model: self.model.clone(),
            recognizer,
            config: self.config.clone(),
            is_speaking: Arc::clone(&self.is_speaking),
            silence_start: Mutex::new(self.silence_start.lock().unwrap().clone()),
            is_finalized: Arc::clone(&self.is_finalized),
            audio_cache: self.audio_cache.clone(),
            max_cache_samples: self.max_cache_samples,
        }
    }
}

impl AudioRecognizer {
    pub fn new(model_path: &str, config: AudioRecognizerConfig) -> Result<Self> {
        let model = Model::new(model_path)
            .with_context(|| format!("Failed to load Vosk model from {}", model_path))?;
        let recognizer = if config.grammar.is_empty() {
            Recognizer::new(&model, VOSK_SAMPLE_RATE)
        } else {
            Recognizer::new_with_grammar(&model, VOSK_SAMPLE_RATE, &config.grammar)
        }
        .context("Failed to create Vosk recognizer")?;

        let samples_per_frame = VOSK_SAMPLE_RATE as usize * 20 / 1000;
        let vad_samples = 4 * samples_per_frame;
        let chunk_samples = (config.chunk_time * VOSK_SAMPLE_RATE) as usize;
        let max_cache_samples = chunk_samples + vad_samples;

        Ok(Self {
            model: Arc::new(model),
            recognizer,
            config,
            is_speaking: Arc::new(AtomicBool::new(false)),
            silence_start: Mutex::new(None),
            is_finalized: Arc::new(AtomicBool::new(false)),
            audio_cache: Vec::with_capacity(max_cache_samples),
            max_cache_samples,
        })
    }

    pub fn process_audio_chunk(
        &mut self,
        audio_chunk: &[i16],
    ) -> Result<Option<RecognitionResult>> {
        if self.is_speaking.load(Ordering::Acquire) {
            if !self.audio_cache.is_empty() {
                self.recognizer
                    .accept_waveform(&self.audio_cache)
                    .context("Failed to accept cached waveform")?;
                self.audio_cache.clear();
            }

            self.recognizer
                .accept_waveform(audio_chunk)
                .context("Failed to accept waveform")?;
            let result = self.recognizer.partial_result();
            let result = RecognitionResult {
                text: result.partial.to_string(),
                is_partial: true,
            };

            debug!("partial result: {:?}", result);

            return Ok(Some(result));
        } else {
            self.audio_cache.extend_from_slice(audio_chunk);
            if self.audio_cache.len() > self.max_cache_samples {
                let overflow = self.audio_cache.len() - self.max_cache_samples;
                self.audio_cache.drain(0..overflow);
            }
        }

        Ok(None)
    }

    pub fn finalize(&mut self) -> Result<Option<RecognitionResult>> {
        if !self.is_finalized.load(Ordering::Acquire) {
            return Ok(None);
        }

        let result = self.recognizer.final_result();
        let recognition_result = RecognitionResult {
            text: result
                .single()
                .context("Failed to get final result")?
                .text
                .to_string(),
            is_partial: false,
        };

        self.reset();

        debug!("final result: {:?}", recognition_result);

        Ok(Some(recognition_result))
    }

    /// 检测语音活动
    pub fn detect_speech(&mut self, audio_chunk: &[i16], vad: &mut Vad) -> Result<()> {
        if self.config.is_ptt {
            return Ok(());
        }

        if audio_chunk.is_empty() {
            return Ok(());
        }

        // 切分帧
        // 采样率 * 每帧时间(ms) / 1000
        let samples_per_frame = VOSK_SAMPLE_RATE as usize * 20 / 1000;

        // 连续帧
        let mut active_frames = 0;
        let mut non_active_frames = 0;
        for frame in audio_chunk.chunks_exact(samples_per_frame) {
            let is_active = vad
                .is_voice_segment(frame)
                .map_err(|e| anyhow::anyhow!("Failed to detect speech: {:?}", e))?;

            if is_active {
                active_frames += 1;
                non_active_frames = 0;
            } else {
                active_frames = 0;
                non_active_frames += 1;
            }

            // 连续 3 帧为活动状态，认为是语音
            if active_frames > 3 {
                self.update_speech_state(true);
            }

            // 连续 5 帧为非活动状态，认为是静音
            if non_active_frames > 5 {
                self.update_speech_state(false);
            }
        }
        Ok(())
    }

    /// 更新语音状态
    fn update_speech_state(&mut self, is_speech: bool) {
        let mut silence_start = self.silence_start.lock().unwrap();
        if is_speech {
            *silence_start = None;
            self.is_speaking.store(true, Ordering::Release);
        } else if self.is_speaking.load(Ordering::Acquire) {
            let now = std::time::Instant::now();
            // 没有检测到语音，但之前处于说话状态
            if let Some(start) = *silence_start {
                // 检查静音持续时间是否超过阈值
                if now.duration_since(start).as_millis() > self.config.vad_silence_duration as u128
                {
                    self.is_speaking.store(false, Ordering::Release);
                    *silence_start = None;
                    self.is_finalized.store(true, Ordering::Release);
                }
            } else {
                // 开始静音计时
                *silence_start = Some(now);
            }
        }
    }

    pub fn reset(&mut self) {
        self.recognizer.reset();
        self.is_speaking.store(false, Ordering::Release);
        *self.silence_start.lock().unwrap() = None;
        self.is_finalized.store(false, Ordering::Release);
        self.audio_cache.clear();
    }
}

#[derive(Clone)]
pub struct AudioSpeechController {
    is_speaking: Option<Arc<AtomicBool>>,
    is_finalized: Option<Arc<AtomicBool>>,
}

impl AudioSpeechController {
    pub fn set_is_speaking(&self, speaking: bool) {
        if let Some(is_speaking) = &self.is_speaking {
            is_speaking.store(speaking, Ordering::Release);
            if !speaking {
                if let Some(is_finalized) = &self.is_finalized {
                    is_finalized.store(true, Ordering::Release);
                }
            }
        }
    }
}

pub struct AudioBufferProcessor {
    recognizer: Option<AudioRecognizer>,
    input_device_name: String,
    enable_denoise: bool,
    stream: Option<cpal::Stream>,
    thread_handle: Option<JoinHandle<Result<AudioRecognizer>>>,
    is_speaking: Option<Arc<AtomicBool>>,
    is_finalized: Option<Arc<AtomicBool>>,
    mic_passthrough: Option<MicPassthroughHandle>,
}

impl AudioBufferProcessor {
    pub fn new(recognizer: AudioRecognizer, enable_denoise: bool) -> Result<Self> {
        let input_device_name = default_input_device_name()?;

        let is_speaking = Arc::clone(&recognizer.is_speaking);
        let is_finalized = Arc::clone(&recognizer.is_finalized);

        Ok(Self {
            recognizer: Some(recognizer),
            input_device_name,
            enable_denoise,
            stream: None,
            thread_handle: None,
            is_speaking: Some(is_speaking),
            is_finalized: Some(is_finalized),
            mic_passthrough: None,
        })
    }

    pub fn new_with_input_device_name(
        recognizer: AudioRecognizer,
        input_device_name: String,
        enable_denoise: bool,
        mic_passthrough: Option<MicPassthroughHandle>,
    ) -> Result<Self> {
        let is_speaking = Arc::clone(&recognizer.is_speaking);
        let is_finalized = Arc::clone(&recognizer.is_finalized);

        Ok(Self {
            recognizer: Some(recognizer),
            input_device_name,
            enable_denoise,
            stream: None,
            thread_handle: None,
            is_speaking: Some(is_speaking),
            is_finalized: Some(is_finalized),
            mic_passthrough,
        })
    }

    pub fn get_speech_controller(&self) -> AudioSpeechController {
        AudioSpeechController {
            is_speaking: self.is_speaking.clone(),
            is_finalized: self.is_finalized.clone(),
        }
    }

    pub fn start(&mut self, on_result: Box<dyn Fn(RecognitionResult) + Send>) -> Result<()> {
        if self.is_start() {
            self.stop()?;
        }

        let mut recognizer = self
            .recognizer
            .take()
            .ok_or_else(|| anyhow::anyhow!("Recognizer is already running or missing"))?;

        let microphone_input = open_input_stream(&self.input_device_name)?;
        let sample_rate = microphone_input.sample_rate;
        let rx = microphone_input.rx;
        let stream = microphone_input.stream;

        stream.play()?;
        self.stream = Some(stream);

        let chunk_time = recognizer.config.chunk_time;
        let enable_denoise = self.enable_denoise;
        let mic_passthrough = self.mic_passthrough.clone();

        let handle = std::thread::spawn(move || -> Result<AudioRecognizer> {
            let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);
            run_processed_audio_pipeline(
                rx,
                sample_rate,
                chunk_time,
                enable_denoise,
                mic_passthrough,
                |chunk| {
                    recognizer.detect_speech(chunk, &mut vad)?;
                    let _ = recognizer.process_audio_chunk(chunk)?;
                    if let Some(result) = recognizer.finalize()? {
                        on_result(result);
                    }
                    Ok(())
                },
            )?;
            Ok(recognizer)
        });
        self.thread_handle = Some(handle);

        Ok(())
    }

    pub fn is_start(&self) -> bool {
        self.stream.is_some()
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stream = None;
        if let Some(handle) = self.thread_handle.take() {
            match handle.join() {
                Ok(Ok(mut r)) => {
                    r.reset();
                    self.recognizer = Some(r);
                }
                Ok(Err(e)) => log::error!("Audio processing thread error: {}", e),
                Err(e) => log::error!("Audio processing thread panicked: {:?}", e),
            }
        }
        Ok(())
    }
}

impl Drop for AudioBufferProcessor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
