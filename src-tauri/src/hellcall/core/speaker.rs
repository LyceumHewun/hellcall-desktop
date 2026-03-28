use anyhow::{Context, Result};
use log::{info, warn};
use rodio::{OutputStream, OutputStreamBuilder};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{mpsc::Sender, Arc, RwLock};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug)]
pub struct SpeakerRuntimeConfig {
    pub volume: f32,
    pub speed: f32,
    pub sleep_until_end: bool,
}

pub struct Speaker {
    tx: Sender<String>,
    config: Arc<RwLock<SpeakerRuntimeConfig>>,
    _thread_handle: JoinHandle<Result<()>>,
}

impl Speaker {
    pub fn new(config: SpeakerRuntimeConfig) -> Result<Self> {
        let stream_handle =
            OutputStreamBuilder::open_default_stream().context("open default stream failed")?;
        let config = Arc::new(RwLock::new(config));
        let (tx, handle) = Self::init_thread(stream_handle, Arc::clone(&config));

        Ok(Self {
            tx,
            config,
            _thread_handle: handle,
        })
    }

    fn init_thread(
        stream_handle: OutputStream,
        config: Arc<RwLock<SpeakerRuntimeConfig>>,
    ) -> (Sender<String>, JoinHandle<Result<()>>) {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let handle = std::thread::spawn(move || -> Result<()> {
            while let Ok(audio_path) = rx.recv() {
                let file = BufReader::new(File::open(&audio_path).context("open file failed")?);
                info!("play audio: {}", &audio_path);
                let sink = rodio::play(stream_handle.mixer(), file).context("play wav failed")?;
                let playback_config = *config.read().expect("speaker config poisoned");
                sink.set_volume(playback_config.volume);
                sink.set_speed(playback_config.speed);
                if playback_config.sleep_until_end {
                    sink.sleep_until_end();
                } else {
                    sink.detach();
                }
            }

            Ok(())
        });
        (tx, handle)
    }

    pub fn update_config(&self, config: SpeakerRuntimeConfig) {
        *self.config.write().expect("speaker config poisoned") = config;
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
