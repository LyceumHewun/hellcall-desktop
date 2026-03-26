use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{debug, info};
use vosk::{Model, Recognizer};
use webrtc_vad::{SampleRate, Vad, VadMode};

static VOSK_SAMPLE_RATE: f32 = 16000.0;

#[derive(Debug, Clone)]
pub struct AudioRecognizerConfig {
    /// 音频识别的时间段 (秒)
    pub chunk_time: f32,
    /// 音频识别的语法字典
    pub grammar: Vec<String>,
    /// 语音结束后的静音持续时间 (ms)
    pub vad_silence_duration: u64,
    /// 是否开启降噪
    pub enable_denoise: bool,
    /// 是否为按键说话模式
    pub is_ptt: bool,
}

impl Default for AudioRecognizerConfig {
    fn default() -> Self {
        Self {
            chunk_time: 0.2,
            grammar: Vec::new(),
            vad_silence_duration: 500,
            enable_denoise: false,
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
        let recognizer =
            Recognizer::new_with_grammar(&self.model, VOSK_SAMPLE_RATE, &self.config.grammar)
                .expect("Failed to create Vosk recognizer");

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
        let recognizer = Recognizer::new_with_grammar(&model, VOSK_SAMPLE_RATE, &config.grammar)
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
    stream: Option<cpal::Stream>,
    thread_handle: Option<JoinHandle<Result<AudioRecognizer>>>,
    is_speaking: Option<Arc<AtomicBool>>,
    is_finalized: Option<Arc<AtomicBool>>,
}

impl AudioBufferProcessor {
    pub fn new(recognizer: AudioRecognizer) -> Result<Self> {
        // get default input device name
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("Failed to get default input device")?;
        let input_device_name = device.name().context("Failed to get device name")?;

        info!("default input device name: {}", &input_device_name);

        let is_speaking = Arc::clone(&recognizer.is_speaking);
        let is_finalized = Arc::clone(&recognizer.is_finalized);

        Ok(Self {
            recognizer: Some(recognizer),
            input_device_name,
            stream: None,
            thread_handle: None,
            is_speaking: Some(is_speaking),
            is_finalized: Some(is_finalized),
        })
    }

    pub fn new_with_input_device_name(
        recognizer: AudioRecognizer,
        input_device_name: String,
    ) -> Result<Self> {
        let is_speaking = Arc::clone(&recognizer.is_speaking);
        let is_finalized = Arc::clone(&recognizer.is_finalized);

        Ok(Self {
            recognizer: Some(recognizer),
            input_device_name,
            stream: None,
            thread_handle: None,
            is_speaking: Some(is_speaking),
            is_finalized: Some(is_finalized),
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

        let host = cpal::default_host();
        let mut target_device = None;
        for device in host.input_devices()? {
            if let std::result::Result::Ok(name) = device.name() {
                if name == self.input_device_name {
                    target_device = Some(device);
                    break;
                }
            }
        }
        let device = target_device
            .or_else(|| host.default_input_device())
            .context("Failed to find input device")?;

        let config = device
            .default_input_config()
            .context("Failed to get default input config")?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();

        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(100);

        let error_callback = |err| log::error!("an error occurred on stream: {}", err);

        let tx_clone = tx.clone();

        fn process_to_mono_f32<T: Copy>(
            data: &[T],
            channels: u16,
            to_f32: impl Fn(T) -> f32,
        ) -> Vec<f32> {
            let mut output = Vec::with_capacity(data.len() / channels as usize);
            for frame in data.chunks(channels as usize) {
                let mut sum = 0.0;
                for &sample in frame {
                    sum += to_f32(sample);
                }
                output.push(sum / channels as f32);
            }
            output
        }

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    if let Err(std::sync::mpsc::TrySendError::Full(_)) =
                        tx_clone.try_send(process_to_mono_f32(data, channels, |x| x * 32768.0))
                    {
                        log::warn!("Audio processing is too slow, dropping frames");
                    }
                },
                error_callback,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    if let Err(std::sync::mpsc::TrySendError::Full(_)) =
                        tx_clone.try_send(process_to_mono_f32(data, channels, |x| x as f32))
                    {
                        log::warn!("Audio processing is too slow, dropping frames");
                    }
                },
                error_callback,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &_| {
                    if let Err(std::sync::mpsc::TrySendError::Full(_)) = tx_clone
                        .try_send(process_to_mono_f32(data, channels, |x| x as f32 - 32768.0))
                    {
                        log::warn!("Audio processing is too slow, dropping frames");
                    }
                },
                error_callback,
                None,
            )?,
            _ => {
                self.recognizer = Some(recognizer);
                return Err(anyhow::anyhow!(
                    "Unsupported sample format {:?}",
                    sample_format
                ));
            }
        };

        stream.play()?;
        self.stream = Some(stream);

        let chunk_time = recognizer.config.chunk_time;
        let samples_per_chunk = (chunk_time * VOSK_SAMPLE_RATE) as usize;
        let enable_denoise = recognizer.config.enable_denoise;

        let handle = std::thread::spawn(move || -> Result<AudioRecognizer> {
            use nnnoiseless::DenoiseState;
            use rubato::{
                Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType,
                WindowFunction,
            };

            let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);
            let mut i16_buffer: Vec<i16> = Vec::new();

            if enable_denoise {
                debug!("Denoise enabled (Device SR -> 48k -> Denoise -> 16k)");
                let mut denoiser = DenoiseState::new();
                const FRAME_SIZE: usize = 480;
                let mut f32_48k_buffer: Vec<f32> = Vec::new();

                let mut resampler_out = SincFixedIn::<f32>::new(
                    16000.0 / 48000.0,
                    2.0,
                    SincInterpolationParameters {
                        sinc_len: 256,
                        f_cutoff: 0.95,
                        interpolation: SincInterpolationType::Linear,
                        oversampling_factor: 256,
                        window: WindowFunction::BlackmanHarris2,
                    },
                    FRAME_SIZE,
                    1,
                )
                .map_err(|e| anyhow::anyhow!("Failed to create out resampler: {}", e))?;

                if sample_rate == 48000 {
                    for pcm in rx.iter() {
                        f32_48k_buffer.extend(pcm);

                        while f32_48k_buffer.len() >= FRAME_SIZE {
                            let chunk: Vec<f32> = f32_48k_buffer.drain(..FRAME_SIZE).collect();
                            let mut in_frame = [0.0f32; FRAME_SIZE];
                            in_frame.copy_from_slice(&chunk);

                            let mut out_frame = [0.0f32; FRAME_SIZE];
                            let _ = denoiser.process_frame(&mut out_frame, &in_frame);

                            let waves_in = vec![out_frame.to_vec()];
                            let resampled = resampler_out
                                .process(&waves_in, None)
                                .map_err(|e| anyhow::anyhow!("Resampling out error: {}", e))?;

                            for &sample in &resampled[0] {
                                i16_buffer.push((sample as f32).clamp(-32768.0, 32767.0) as i16);
                            }

                            while i16_buffer.len() >= samples_per_chunk {
                                let chunk: Vec<i16> =
                                    i16_buffer.drain(..samples_per_chunk).collect();
                                recognizer.detect_speech(&chunk, &mut vad)?;
                                let _ = recognizer.process_audio_chunk(&chunk)?;
                                if let Some(result) = recognizer.finalize()? {
                                    on_result(result);
                                }
                            }
                        }
                    }
                } else {
                    let in_chunk_size = 1024;
                    let mut resampler_in = SincFixedIn::<f32>::new(
                        48000.0 / sample_rate as f64,
                        2.0,
                        SincInterpolationParameters {
                            sinc_len: 256,
                            f_cutoff: 0.95,
                            interpolation: SincInterpolationType::Linear,
                            oversampling_factor: 256,
                            window: WindowFunction::BlackmanHarris2,
                        },
                        in_chunk_size,
                        1,
                    )
                    .map_err(|e| anyhow::anyhow!("Failed to create in resampler: {}", e))?;

                    let mut f32_in_buffer: Vec<f32> = Vec::new();

                    for pcm in rx.iter() {
                        f32_in_buffer.extend(pcm);

                        while f32_in_buffer.len() >= in_chunk_size {
                            let input_chunk: Vec<f32> =
                                f32_in_buffer.drain(..in_chunk_size).collect();
                            let waves_in = vec![input_chunk];
                            let resampled_in = resampler_in
                                .process(&waves_in, None)
                                .map_err(|e| anyhow::anyhow!("Resampling in error: {}", e))?;

                            f32_48k_buffer.extend(&resampled_in[0]);

                            while f32_48k_buffer.len() >= FRAME_SIZE {
                                let chunk: Vec<f32> = f32_48k_buffer.drain(..FRAME_SIZE).collect();
                                let mut in_frame = [0.0f32; FRAME_SIZE];
                                in_frame.copy_from_slice(&chunk);

                                let mut out_frame = [0.0f32; FRAME_SIZE];
                                let _ = denoiser.process_frame(&mut out_frame, &in_frame);

                                let waves_out = vec![out_frame.to_vec()];
                                let resampled_out = resampler_out
                                    .process(&waves_out, None)
                                    .map_err(|e| anyhow::anyhow!("Resampling out error: {}", e))?;

                                for &sample in &resampled_out[0] {
                                    i16_buffer
                                        .push((sample as f32).clamp(-32768.0, 32767.0) as i16);
                                }

                                while i16_buffer.len() >= samples_per_chunk {
                                    let chunk: Vec<i16> =
                                        i16_buffer.drain(..samples_per_chunk).collect();
                                    recognizer.detect_speech(&chunk, &mut vad)?;
                                    let _ = recognizer.process_audio_chunk(&chunk)?;
                                    if let Some(result) = recognizer.finalize()? {
                                        on_result(result);
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                debug!("Denoise disabled (Device SR -> 16k)");
                // Fast path: No denoise, resample directly to 16kHz
                if sample_rate == 16000 {
                    for pcm in rx.iter() {
                        for &sample in &pcm {
                            i16_buffer.push((sample as f32).clamp(-32768.0, 32767.0) as i16);
                        }

                        while i16_buffer.len() >= samples_per_chunk {
                            let chunk: Vec<i16> = i16_buffer.drain(..samples_per_chunk).collect();
                            recognizer.detect_speech(&chunk, &mut vad)?;
                            let _ = recognizer.process_audio_chunk(&chunk)?;
                            if let Some(result) = recognizer.finalize()? {
                                on_result(result);
                            }
                        }
                    }
                } else {
                    let chunk_size = 1024;
                    let mut resampler = SincFixedIn::<f32>::new(
                        16000.0 / sample_rate as f64,
                        2.0,
                        SincInterpolationParameters {
                            sinc_len: 256,
                            f_cutoff: 0.95,
                            interpolation: SincInterpolationType::Linear,
                            oversampling_factor: 256,
                            window: WindowFunction::BlackmanHarris2,
                        },
                        chunk_size,
                        1,
                    )
                    .map_err(|e| anyhow::anyhow!("Failed to create resampler: {}", e))?;

                    let mut f32_buffer: Vec<f32> = Vec::new();

                    for pcm in rx.iter() {
                        f32_buffer.extend(pcm);

                        while f32_buffer.len() >= chunk_size {
                            let input_chunk: Vec<f32> = f32_buffer.drain(..chunk_size).collect();
                            let waves_in = vec![input_chunk];
                            let resampled = resampler
                                .process(&waves_in, None)
                                .map_err(|e| anyhow::anyhow!("Resampling error: {}", e))?;

                            for &sample in &resampled[0] {
                                i16_buffer.push((sample as f32).clamp(-32768.0, 32767.0) as i16);
                            }

                            while i16_buffer.len() >= samples_per_chunk {
                                let chunk: Vec<i16> =
                                    i16_buffer.drain(..samples_per_chunk).collect();
                                recognizer.detect_speech(&chunk, &mut vad)?;
                                let _ = recognizer.process_audio_chunk(&chunk)?;
                                if let Some(result) = recognizer.finalize()? {
                                    on_result(result);
                                }
                            }
                        }
                    }
                }
            }
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
