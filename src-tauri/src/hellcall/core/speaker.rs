use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use log::{info, warn};
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::sync::{mpsc::Sender, Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

const MAX_MIC_BUFFER_MS: usize = 500;
const MIN_PLAYBACK_SPEED: f32 = 0.05;

#[derive(Clone, Debug)]
pub struct SpeakerRuntimeConfig {
    pub volume: f32,
    pub speed: f32,
    pub sleep_until_end: bool,
    pub monitor_local_playback: bool,
    pub virtual_mic_enabled: bool,
    pub virtual_mic_device: Option<String>,
    pub virtual_mic_macro_volume: f32,
    pub virtual_mic_input_volume: f32,
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

struct VirtualMicMixer {
    stream: OutputStream,
    mic_passthrough: MicPassthroughHandle,
    mic_volume: Arc<RwLock<f32>>,
}

impl VirtualMicMixer {
    fn new(device_name: &str, mic_sample_rate: u32, mic_volume: f32) -> Result<Self> {
        let device = find_output_device_by_name(device_name)
            .with_context(|| format!("virtual mic output device '{}' not found", device_name))?;
        let mut stream = OutputStreamBuilder::from_device(device)
            .context("open virtual mic output stream builder failed")?
            .open_stream_or_fallback()
            .context("open virtual mic output stream failed")?;
        stream.log_on_drop(false);

        let mic_passthrough = MicPassthroughHandle::new(mic_sample_rate);
        let mic_volume = Arc::new(RwLock::new(mic_volume));
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

    fn mic_passthrough(&self) -> MicPassthroughHandle {
        self.mic_passthrough.clone()
    }

    fn update_mic_volume(&self, volume: f32) {
        *self
            .mic_volume
            .write()
            .expect("virtual mic input volume poisoned") = volume;
    }
}

struct DecodedAudio {
    channels: u16,
    sample_rate: u32,
    samples: Vec<f32>,
}

pub struct Speaker {
    tx: Sender<String>,
    config: Arc<RwLock<SpeakerRuntimeConfig>>,
    mic_passthrough: Option<MicPassthroughHandle>,
    _thread_handle: JoinHandle<Result<()>>,
}

impl Speaker {
    pub fn new(config: SpeakerRuntimeConfig, input_device_name: &str) -> Result<Self> {
        let local_stream = if config.monitor_local_playback {
            Some(
                OutputStreamBuilder::open_default_stream()
                    .context("open default stream failed")?,
            )
        } else {
            None
        };

        let virtual_mic = if config.virtual_mic_enabled {
            let device_name = config
                .virtual_mic_device
                .clone()
                .filter(|name| !name.trim().is_empty())
                .context("virtual mic output device is required when virtual mic is enabled")?;
            let mic_sample_rate = resolve_input_device_sample_rate(input_device_name)?;
            Some(VirtualMicMixer::new(
                &device_name,
                mic_sample_rate,
                config.virtual_mic_input_volume,
            )?)
        } else {
            None
        };

        let mic_passthrough = virtual_mic.as_ref().map(VirtualMicMixer::mic_passthrough);
        let config = Arc::new(RwLock::new(config));
        let (tx, handle) = Self::init_thread(local_stream, virtual_mic, Arc::clone(&config));

        Ok(Self {
            tx,
            config,
            mic_passthrough,
            _thread_handle: handle,
        })
    }

    fn init_thread(
        local_stream: Option<OutputStream>,
        virtual_mic: Option<VirtualMicMixer>,
        config: Arc<RwLock<SpeakerRuntimeConfig>>,
    ) -> (Sender<String>, JoinHandle<Result<()>>) {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let handle = std::thread::spawn(move || -> Result<()> {
            while let Ok(audio_path) = rx.recv() {
                info!("play audio: {}", &audio_path);
                let playback_config = config.read().expect("speaker config poisoned").clone();
                let audio = decode_audio_file(&audio_path)?;

                let local_sink = if playback_config.monitor_local_playback {
                    create_sink(local_stream.as_ref(), &audio, |sink| {
                        sink.set_volume(playback_config.volume);
                        sink.set_speed(playback_speed(playback_config.speed));
                    })
                } else {
                    None
                };

                if let Some(virtual_mic) = &virtual_mic {
                    virtual_mic.update_mic_volume(playback_config.virtual_mic_input_volume);
                }
                let virtual_sink = if playback_config.virtual_mic_enabled {
                    create_sink(virtual_mic.as_ref(), &audio, |sink| {
                        sink.set_volume(playback_config.virtual_mic_macro_volume);
                        sink.set_speed(playback_speed(playback_config.speed));
                    })
                } else {
                    None
                };

                if local_sink.is_none() && virtual_sink.is_none() {
                    warn!("audio playback ignored because no playback target is enabled");
                    continue;
                }

                if playback_config.sleep_until_end {
                    if let Some(sink) = local_sink {
                        sink.sleep_until_end();
                    }
                    if let Some(sink) = virtual_sink {
                        sink.sleep_until_end();
                    }
                } else {
                    if let Some(sink) = local_sink {
                        sink.detach();
                    }
                    if let Some(sink) = virtual_sink {
                        sink.detach();
                    }
                }
            }

            Ok(())
        });
        (tx, handle)
    }

    pub fn update_config(&self, config: SpeakerRuntimeConfig) {
        *self.config.write().expect("speaker config poisoned") = config;
    }

    pub fn mic_passthrough(&self) -> Option<MicPassthroughHandle> {
        self.mic_passthrough.clone()
    }

    pub fn play_wav(&self, path: &str) -> Result<()> {
        if Path::new(path).exists() {
            self.tx.send(path.to_string())?;
        } else {
            warn!("audio file not found: {}", path)
        }
        Ok(())
    }
}

fn create_sink<F>(sink_factory: Option<F>, audio: &DecodedAudio, configure: impl Fn(&Sink)) -> Option<Sink>
where
    F: SinkTarget,
{
    let sink = sink_factory?.create_sink();
    configure(&sink);
    sink.append(SamplesBuffer::new(
        audio.channels,
        audio.sample_rate,
        audio.samples.clone(),
    ));
    Some(sink)
}

trait SinkTarget {
    fn create_sink(&self) -> Sink;
}

impl<T> SinkTarget for &T
where
    T: SinkTarget + ?Sized,
{
    fn create_sink(&self) -> Sink {
        (*self).create_sink()
    }
}

impl SinkTarget for OutputStream {
    fn create_sink(&self) -> Sink {
        Sink::connect_new(self.mixer())
    }
}

impl SinkTarget for VirtualMicMixer {
    fn create_sink(&self) -> Sink {
        Sink::connect_new(self.stream.mixer())
    }
}

fn decode_audio_file(path: &str) -> Result<DecodedAudio> {
    let file = File::open(path).context("open file failed")?;
    let decoder = Decoder::try_from(file).context("decode audio file failed")?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples = decoder.collect::<Vec<f32>>();

    Ok(DecodedAudio {
        channels,
        sample_rate,
        samples,
    })
}

fn resolve_input_device_sample_rate(input_device_name: &str) -> Result<u32> {
    let host = cpal::default_host();
    let device = host
        .input_devices()?
        .find_map(|device| match device.name() {
            Ok(name) if name == input_device_name => Some(device),
            _ => None,
        })
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

fn playback_speed(speed: f32) -> f32 {
    speed.max(MIN_PLAYBACK_SPEED)
}
