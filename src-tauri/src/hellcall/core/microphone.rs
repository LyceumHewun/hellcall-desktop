use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::traits::StreamTrait;
use log::{debug, info};
use rodio::{OutputStream, OutputStreamBuilder, Sink};
use std::collections::VecDeque;
use std::sync::{mpsc::Receiver, Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const MAX_MIC_BUFFER_MS: usize = 500;
const VOSK_SAMPLE_RATE: f32 = 16000.0;

pub struct MicrophoneInputStream {
    pub stream: cpal::Stream,
    pub sample_rate: u32,
    pub rx: Receiver<Vec<f32>>,
}

#[derive(Clone)]
pub struct MicPassthroughHandle {
    state: Arc<Mutex<MicPassthroughState>>,
}

struct MicPassthroughState {
    queue: VecDeque<f32>,
    max_samples: usize,
}

impl MicPassthroughHandle {
    fn new(sample_rate: u32) -> Self {
        let max_samples = ((sample_rate as usize) * MAX_MIC_BUFFER_MS) / 1000;
        Self {
            state: Arc::new(Mutex::new(MicPassthroughState {
                queue: VecDeque::with_capacity(max_samples.max(1)),
                max_samples: max_samples.max(1),
            })),
        }
    }

    pub fn push_samples(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let mut state = self.state.lock().expect("mic passthrough poisoned");
        if samples.len() >= state.max_samples {
            let max_samples = state.max_samples;
            state.queue.clear();
            state
                .queue
                .extend(samples[samples.len() - max_samples..].iter().copied());
            return;
        }

        let overflow = state
            .queue
            .len()
            .saturating_add(samples.len())
            .saturating_sub(state.max_samples);
        for _ in 0..overflow {
            let _ = state.queue.pop_front();
        }
        state.queue.extend(samples.iter().copied());
    }
}

struct LiveMicSource {
    state: Arc<Mutex<MicPassthroughState>>,
    volume: Arc<RwLock<f32>>,
    sample_rate: u32,
}

impl LiveMicSource {
    fn new(
        state: Arc<Mutex<MicPassthroughState>>,
        volume: Arc<RwLock<f32>>,
        sample_rate: u32,
    ) -> Self {
        Self {
            state,
            volume,
            sample_rate,
        }
    }
}

impl Iterator for LiveMicSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = {
            let mut state = self.state.lock().expect("mic passthrough poisoned");
            state.queue.pop_front().unwrap_or(0.0)
        };
        let volume = *self.volume.read().expect("mic source volume poisoned");
        Some(sample * volume)
    }
}

impl rodio::Source for LiveMicSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        1
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }

    fn try_seek(
        &mut self,
        _: Duration,
    ) -> std::result::Result<(), rodio::source::SeekError> {
        Err(rodio::source::SeekError::NotSupported {
            underlying_source: std::any::type_name::<Self>(),
        })
    }
}

pub struct VirtualMicMixer {
    stream: OutputStream,
    mic_passthrough: MicPassthroughHandle,
    mic_volume: Arc<RwLock<f32>>,
}

impl VirtualMicMixer {
    pub fn new(
        input_device_name: &str,
        output_device_name: &str,
        input_volume: f32,
        enable_denoise: bool,
    ) -> Result<Self> {
        let device = find_output_device_by_name(output_device_name)
            .with_context(|| format!("virtual mic output device '{}' not found", output_device_name))?;
        let mut stream = OutputStreamBuilder::from_device(device)
            .context("open virtual mic output stream builder failed")?
            .open_stream_or_fallback()
            .context("open virtual mic output stream failed")?;
        stream.log_on_drop(false);

        let mic_sample_rate = if enable_denoise {
            48_000
        } else {
            resolve_input_device_sample_rate(input_device_name)?
        };
        let mic_passthrough = MicPassthroughHandle::new(mic_sample_rate);
        let mic_volume = Arc::new(RwLock::new(input_volume));
        stream.mixer().add(LiveMicSource::new(
            Arc::clone(&mic_passthrough.state),
            Arc::clone(&mic_volume),
            mic_sample_rate,
        ));

        Ok(Self {
            stream,
            mic_passthrough,
            mic_volume,
        })
    }

    pub fn create_sink(&self) -> Sink {
        Sink::connect_new(self.stream.mixer())
    }

    pub fn mic_passthrough(&self) -> MicPassthroughHandle {
        self.mic_passthrough.clone()
    }

    pub fn update_input_volume(&self, volume: f32) {
        *self
            .mic_volume
            .write()
            .expect("virtual mic input volume poisoned") = volume;
    }

    pub fn input_volume_handle(&self) -> Arc<RwLock<f32>> {
        Arc::clone(&self.mic_volume)
    }
}

pub fn resolve_input_device_name(selected_name: Option<String>) -> Result<String> {
    if let Some(name) = selected_name.filter(|name| !name.trim().is_empty()) {
        return Ok(name);
    }

    default_input_device_name()
}

pub fn default_input_device_name() -> Result<String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("Failed to get default input device")?;
    let input_device_name = device.name().context("Failed to get device name")?;
    info!("default input device name: {}", &input_device_name);
    Ok(input_device_name)
}

pub fn open_input_stream(input_device_name: &str) -> Result<MicrophoneInputStream> {
    let device = find_input_device_by_name(input_device_name)?
        .or_else(|| cpal::default_host().default_input_device())
        .context("Failed to find input device")?;

    let config = device
        .default_input_config()
        .context("Failed to get default input config")?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();

    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(100);
    let error_callback = |err| log::error!("an error occurred on stream: {}", err);

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
        cpal::SampleFormat::F32 => {
            let tx_clone = tx.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    let mono = process_to_mono_f32(data, channels, |x| x.clamp(-1.0, 1.0));
                    if let Err(std::sync::mpsc::TrySendError::Full(_)) = tx_clone.try_send(
                        mono.iter().map(|sample| sample * 32768.0).collect(),
                    ) {
                        log::warn!("Audio processing is too slow, dropping frames");
                    }
                },
                error_callback,
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let tx_clone = tx.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    let mono = process_to_mono_f32(data, channels, |x| x as f32 / 32768.0);
                    if let Err(std::sync::mpsc::TrySendError::Full(_)) = tx_clone.try_send(
                        mono.iter().map(|sample| sample * 32768.0).collect(),
                    ) {
                        log::warn!("Audio processing is too slow, dropping frames");
                    }
                },
                error_callback,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let tx_clone = tx.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &_| {
                    let mono = process_to_mono_f32(data, channels, |x| {
                        (x as f32 - 32768.0) / 32768.0
                    });
                    if let Err(std::sync::mpsc::TrySendError::Full(_)) = tx_clone.try_send(
                        mono.iter().map(|sample| sample * 32768.0).collect(),
                    ) {
                        log::warn!("Audio processing is too slow, dropping frames");
                    }
                },
                error_callback,
                None,
            )?
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported sample format {:?}",
                sample_format
            ));
        }
    };

    Ok(MicrophoneInputStream {
        stream,
        sample_rate,
        rx,
    })
}

pub fn open_volume_meter_stream(
    selected_name: Option<String>,
    enable_denoise: bool,
    mut on_volume: impl FnMut(f32) + Send + 'static,
) -> Result<cpal::Stream> {
    let input_device_name = resolve_input_device_name(selected_name)?;
    let microphone_input = open_input_stream(&input_device_name)?;
    let sample_rate = microphone_input.sample_rate;
    let rx = microphone_input.rx;
    let stream = microphone_input.stream;
    stream.play().context("Failed to play mic test stream")?;

    std::thread::spawn(move || {
        let last_emit = Arc::new(Mutex::new(Instant::now()));
        let result = run_processed_audio_pipeline(
            rx,
            sample_rate,
            0.06,
            enable_denoise,
            None,
            |chunk| {
                let mut last_emit_guard = last_emit.lock().expect("mic test timer poisoned");
                if last_emit_guard.elapsed() < Duration::from_millis(60) || chunk.is_empty() {
                    return Ok(());
                }

                let sum_squares: f32 = chunk
                    .iter()
                    .map(|sample| {
                        let sample = *sample as f32 / 32768.0;
                        sample * sample
                    })
                    .sum();
                let rms = (sum_squares / chunk.len() as f32).sqrt();
                on_volume(rms);
                *last_emit_guard = Instant::now();
                Ok(())
            },
        );

        if let Err(error) = result {
            log::error!("mic test processing failed: {}", error);
        }
    });

    Ok(stream)
}

pub fn run_processed_audio_pipeline(
    rx: Receiver<Vec<f32>>,
    sample_rate: u32,
    chunk_time: f32,
    enable_denoise: bool,
    mic_passthrough: Option<MicPassthroughHandle>,
    mut on_chunk: impl FnMut(&[i16]) -> Result<()>,
) -> Result<()> {
    use nnnoiseless::DenoiseState;
    use rubato::{
        Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType,
        WindowFunction,
    };

    let samples_per_chunk = (chunk_time * VOSK_SAMPLE_RATE) as usize;
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

                    if let Some(mic_passthrough) = &mic_passthrough {
                        push_passthrough_frame(mic_passthrough, &out_frame);
                    }

                    let waves_in = vec![out_frame.to_vec()];
                    let resampled = resampler_out
                        .process(&waves_in, None)
                        .map_err(|e| anyhow::anyhow!("Resampling out error: {}", e))?;

                    for &sample in &resampled[0] {
                        i16_buffer.push((sample as f32).clamp(-32768.0, 32767.0) as i16);
                    }

                    emit_ready_chunks(&mut i16_buffer, samples_per_chunk, &mut on_chunk)?;
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
                    let input_chunk: Vec<f32> = f32_in_buffer.drain(..in_chunk_size).collect();
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

                        if let Some(mic_passthrough) = &mic_passthrough {
                            push_passthrough_frame(mic_passthrough, &out_frame);
                        }

                        let waves_out = vec![out_frame.to_vec()];
                        let resampled_out = resampler_out
                            .process(&waves_out, None)
                            .map_err(|e| anyhow::anyhow!("Resampling out error: {}", e))?;

                        for &sample in &resampled_out[0] {
                            i16_buffer.push((sample as f32).clamp(-32768.0, 32767.0) as i16);
                        }

                        emit_ready_chunks(&mut i16_buffer, samples_per_chunk, &mut on_chunk)?;
                    }
                }
            }
        }
    } else {
        debug!("Denoise disabled (Device SR -> 16k)");
        if sample_rate == 16000 {
            for pcm in rx.iter() {
                if let Some(mic_passthrough) = &mic_passthrough {
                    push_passthrough_buffer(mic_passthrough, &pcm);
                }
                for &sample in &pcm {
                    i16_buffer.push((sample as f32).clamp(-32768.0, 32767.0) as i16);
                }

                emit_ready_chunks(&mut i16_buffer, samples_per_chunk, &mut on_chunk)?;
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
                if let Some(mic_passthrough) = &mic_passthrough {
                    push_passthrough_buffer(mic_passthrough, &pcm);
                }
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

                    emit_ready_chunks(&mut i16_buffer, samples_per_chunk, &mut on_chunk)?;
                }
            }
        }
    }

    Ok(())
}

fn emit_ready_chunks(
    i16_buffer: &mut Vec<i16>,
    samples_per_chunk: usize,
    on_chunk: &mut impl FnMut(&[i16]) -> Result<()>,
) -> Result<()> {
    while i16_buffer.len() >= samples_per_chunk {
        let chunk: Vec<i16> = i16_buffer.drain(..samples_per_chunk).collect();
        on_chunk(&chunk)?;
    }
    Ok(())
}

fn push_passthrough_frame(mic_passthrough: &MicPassthroughHandle, frame: &[f32]) {
    let normalized = frame
        .iter()
        .map(|sample| (sample / 32768.0).clamp(-1.0, 1.0))
        .collect::<Vec<f32>>();
    mic_passthrough.push_samples(&normalized);
}

fn push_passthrough_buffer(mic_passthrough: &MicPassthroughHandle, buffer: &[f32]) {
    let normalized = buffer
        .iter()
        .map(|sample| (sample / 32768.0).clamp(-1.0, 1.0))
        .collect::<Vec<f32>>();
    mic_passthrough.push_samples(&normalized);
}

fn find_input_device_by_name(input_device_name: &str) -> Result<Option<cpal::Device>> {
    let host = cpal::default_host();
    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            if name == input_device_name {
                return Ok(Some(device));
            }
        }
    }
    Ok(None)
}

fn resolve_input_device_sample_rate(input_device_name: &str) -> Result<u32> {
    let host = cpal::default_host();
    let device = find_input_device_by_name(input_device_name)?
        .or_else(|| host.default_input_device())
        .context("failed to find input device for virtual mic mixer")?;

    Ok(device
        .default_input_config()
        .context("failed to get virtual mic input config")?
        .sample_rate()
        .0)
}

fn find_output_device_by_name(device_name: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    host.output_devices()?
        .find_map(|device| match device.name() {
            Ok(name) if name == device_name => Some(device),
            _ => None,
        })
        .context("failed to find output device")
}
