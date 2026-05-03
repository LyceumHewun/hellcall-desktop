use anyhow::{Context, Result};
use log::{info, warn};
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::fs::File;
use std::path::Path;
use std::collections::VecDeque;
use std::sync::{
    mpsc::{Receiver, Sender},
    Arc, RwLock,
};
use std::thread::JoinHandle;
use std::time::Duration;

use super::microphone::{MicPassthroughHandle, VirtualMicMixer};

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

struct DecodedAudio {
    channels: u16,
    sample_rate: u32,
    samples: Vec<f32>,
}

enum PlaybackRequest {
    File(String),
    Samples(DecodedAudio, Option<Sender<()>>),
    Stop,
}

pub struct Speaker {
    tx: Sender<PlaybackRequest>,
    config: Arc<RwLock<SpeakerRuntimeConfig>>,
    mic_passthrough: Option<MicPassthroughHandle>,
    virtual_mic_input_volume: Option<Arc<RwLock<f32>>>,
    _thread_handle: JoinHandle<Result<()>>,
}

impl Speaker {
    pub fn new(
        config: SpeakerRuntimeConfig,
        input_device_name: &str,
        microphone_enable_denoise: bool,
    ) -> Result<Self> {
        let local_stream = if config.monitor_local_playback {
            Some(
                OutputStreamBuilder::open_default_stream()
                    .context("open default stream failed")?,
            )
        } else {
            None
        };

        let virtual_mic = if let Some(device_name) = config
            .virtual_mic_device
            .clone()
            .filter(|name| !name.trim().is_empty())
        {
            Some(VirtualMicMixer::new(
                input_device_name,
                &device_name,
                effective_virtual_mic_input_volume(&config),
                microphone_enable_denoise,
            )?)
        } else {
            None
        };

        let mic_passthrough = virtual_mic.as_ref().map(VirtualMicMixer::mic_passthrough);
        let virtual_mic_input_volume = virtual_mic
            .as_ref()
            .map(VirtualMicMixer::input_volume_handle);
        let config = Arc::new(RwLock::new(config));
        let (tx, handle) = Self::init_thread(local_stream, virtual_mic, Arc::clone(&config));

        Ok(Self {
            tx,
            config,
            mic_passthrough,
            virtual_mic_input_volume,
            _thread_handle: handle,
        })
    }

    fn init_thread(
        local_stream: Option<OutputStream>,
        virtual_mic: Option<VirtualMicMixer>,
        config: Arc<RwLock<SpeakerRuntimeConfig>>,
    ) -> (Sender<PlaybackRequest>, JoinHandle<Result<()>>) {
        let (tx, rx) = std::sync::mpsc::channel::<PlaybackRequest>();
        let handle = std::thread::spawn(move || -> Result<()> {
            let mut pending_requests = VecDeque::new();
            loop {
                let request = match pending_requests.pop_front() {
                    Some(request) => request,
                    None => match rx.recv() {
                        Ok(request) => request,
                        Err(_) => break,
                    },
                };
                if matches!(request, PlaybackRequest::Stop) {
                    continue;
                }

                let playback_config = config.read().expect("speaker config poisoned").clone();
                let (audio, completion) = match request {
                    PlaybackRequest::File(audio_path) => {
                        info!("play audio: {}", &audio_path);
                        (decode_audio_file(&audio_path)?, None)
                    }
                    PlaybackRequest::Samples(audio, completion) => (audio, completion),
                    PlaybackRequest::Stop => unreachable!(),
                };

                let local_sink = if playback_config.monitor_local_playback {
                    create_sink(local_stream.as_ref(), &audio, |sink| {
                        sink.set_volume(playback_config.volume);
                        sink.set_speed(playback_speed(playback_config.speed));
                    })
                } else {
                    None
                };

                if let Some(virtual_mic) = &virtual_mic {
                    virtual_mic
                        .update_input_volume(effective_virtual_mic_input_volume(&playback_config));
                }
                let virtual_sink = if virtual_mic.is_some() {
                    create_sink(virtual_mic.as_ref(), &audio, |sink| {
                        sink.set_volume(playback_config.virtual_mic_macro_volume);
                        sink.set_speed(playback_speed(playback_config.speed));
                    })
                } else {
                    None
                };

                if local_sink.is_none() && virtual_sink.is_none() {
                    warn!("audio playback ignored because no playback target is enabled");
                    if let Some(completion) = completion {
                        let _ = completion.send(());
                    }
                    continue;
                }

                if playback_config.sleep_until_end {
                    wait_for_sinks_or_stop(
                        &rx,
                        &mut pending_requests,
                        local_sink,
                        virtual_sink,
                    );
                } else {
                    if let Some(sink) = local_sink {
                        sink.detach();
                    }
                    if let Some(sink) = virtual_sink {
                        sink.detach();
                    }
                }
                if let Some(completion) = completion {
                    let _ = completion.send(());
                }
            }

            Ok(())
        });
        (tx, handle)
    }

    pub fn update_config(&self, config: SpeakerRuntimeConfig) {
        if let Some(volume) = &self.virtual_mic_input_volume {
            *volume
                .write()
                .expect("virtual mic input volume poisoned") =
                effective_virtual_mic_input_volume(&config);
        }
        *self.config.write().expect("speaker config poisoned") = config;
    }

    pub fn mic_passthrough(&self) -> Option<MicPassthroughHandle> {
        self.mic_passthrough.clone()
    }

    pub fn play_wav(&self, path: &str) -> Result<()> {
        if Path::new(path).exists() {
            self.tx.send(PlaybackRequest::File(path.to_string()))?;
        } else {
            warn!("audio file not found: {}", path)
        }
        Ok(())
    }

    pub fn play_pcm_f32(
        &self,
        channels: u16,
        sample_rate: u32,
        samples: Vec<f32>,
    ) -> Result<()> {
        self.tx.send(PlaybackRequest::Samples(
            DecodedAudio {
                channels,
                sample_rate,
                samples,
            },
            None,
        ))?;
        Ok(())
    }

    pub fn play_pcm_f32_and_wait(
        &self,
        channels: u16,
        sample_rate: u32,
        samples: Vec<f32>,
    ) -> Result<()> {
        let done_rx = self.play_pcm_f32_with_completion(channels, sample_rate, samples)?;
        done_rx.recv()?;
        Ok(())
    }

    pub fn play_pcm_f32_with_completion(
        &self,
        channels: u16,
        sample_rate: u32,
        samples: Vec<f32>,
    ) -> Result<Receiver<()>> {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        self.tx.send(PlaybackRequest::Samples(
            DecodedAudio {
                channels,
                sample_rate,
                samples,
            },
            Some(done_tx),
        ))?;
        Ok(done_rx)
    }

    pub fn stop(&self) -> Result<()> {
        self.tx.send(PlaybackRequest::Stop)?;
        Ok(())
    }
}

fn wait_for_sinks_or_stop(
    rx: &std::sync::mpsc::Receiver<PlaybackRequest>,
    pending_requests: &mut VecDeque<PlaybackRequest>,
    local_sink: Option<Sink>,
    virtual_sink: Option<Sink>,
) {
    let local_sink = local_sink;
    let virtual_sink = virtual_sink;

    loop {
        if local_sink.as_ref().is_none_or(Sink::empty)
            && virtual_sink.as_ref().is_none_or(Sink::empty)
        {
            return;
        }

        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(PlaybackRequest::Stop) => {
                if let Some(sink) = &local_sink {
                    sink.stop();
                }
                if let Some(sink) = &virtual_sink {
                    sink.stop();
                }
                pending_requests.clear();
                return;
            }
            Ok(next_request) => {
                pending_requests.push_back(next_request);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
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
        VirtualMicMixer::create_sink(self)
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

fn playback_speed(speed: f32) -> f32 {
    speed.max(MIN_PLAYBACK_SPEED)
}

fn effective_virtual_mic_input_volume(config: &SpeakerRuntimeConfig) -> f32 {
    if config.virtual_mic_enabled {
        config.virtual_mic_input_volume
    } else {
        0.0
    }
}
