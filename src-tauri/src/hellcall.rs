pub mod config;
pub mod core;
pub mod utils;

pub use config::Config;

use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait};
use log::{info, warn};
use rand::seq::IndexedRandom;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

use self::config::TalkMode;
use self::core::audio::*;
use self::core::command::*;
use self::core::keypress::*;
use self::core::matcher::*;
use self::core::speaker::*;
use self::utils::*;

static AUDIO_DIR: &str = "audio";

/// 复用句柄，stop() 后由调用方持有，传给 restart() 使用。
/// 包含不能被中断的 rdev listener 线程和 KeyPresser，避免重复 spawn。
pub struct EngineHandle {
    key_presser: Arc<KeyPresser>,
    listener_handle: thread::JoinHandle<()>,
}

impl EngineHandle {
    /// 用已有的 listener 线程和 KeyPresser 重启引擎。
    pub fn restart(
        self,
        config: Config,
        model_path: &str,
        input_device_name: Option<String>,
        audio_dir: Option<String>,
    ) -> Result<HellcallEngine> {
        HellcallEngine::start_inner(
            config,
            model_path,
            input_device_name,
            audio_dir,
            Some((self.key_presser, self.listener_handle)),
        )
    }
}

pub struct HellcallEngine {
    // 字段 drop 顺序 = 声明顺序：
    // 1. _processor 先 drop → 音频线程 join → on_result 闭包 drop → Arc<Command> drop → 命令闭包 drop
    // 2. _speaker 后 drop → 命令闭包里的 speaker_ref 都已 drop，此时引用计数归零 → Speaker 线程退出
    // 3. cancel_flag 最后 drop
    _processor: AudioBufferProcessor,
    _speaker: Arc<Speaker>,
    cancel_flag: Arc<AtomicBool>,
    // 以下两个字段由 stop(self) 转移给 EngineHandle，不在这里 drop。
    _key_presser: Arc<KeyPresser>,
    _listener_handle: Option<thread::JoinHandle<()>>,
}

impl HellcallEngine {
    pub fn start(
        config: Config,
        model_path: &str,
        input_device_name: Option<String>,
        audio_dir: Option<String>,
    ) -> Result<Self> {
        Self::start_inner(config, model_path, input_device_name, audio_dir, None)
    }

    fn start_inner(
        config: Config,
        model_path: &str,
        input_device_name: Option<String>,
        audio_dir: Option<String>,
        existing: Option<(Arc<KeyPresser>, thread::JoinHandle<()>)>,
    ) -> Result<Self> {
        // 选择输入设备
        let input_device = if let Some(name) = input_device_name.filter(|n| !n.is_empty()) {
            name
        } else {
            let host = cpal::default_host();
            let default_device = host
                .default_input_device()
                .ok_or_else(|| anyhow!("No default input device found"))?;
            default_device.name()?
        };
        info!("input_device_name: {}", input_device);

        // 选择音频目录
        let audio_dir = if let Some(dir) = audio_dir.filter(|d| !d.is_empty()) {
            dir
        } else {
            AUDIO_DIR.to_string()
        };

        // 初始化 KeyPresser 和 listener（首次创建或复用）
        let key_presser_config = config.key_presser.clone();
        let shortcut = config
            .commands
            .iter()
            .filter(|cmd| cmd.shortcut.is_some())
            .map(|cmd| (cmd.shortcut.clone().unwrap(), cmd.keys.clone()))
            .collect::<HashMap<_, _>>();

        let (key_presser, listener_handle) = if let Some((kp, lh)) = existing {
            kp.update_config(key_presser_config, config.key_map.clone(), shortcut)?;
            (kp, lh)
        } else {
            let kp = Arc::new(KeyPresser::new(
                key_presser_config,
                config.key_map.clone(),
                shortcut,
            )?);
            let kp_clone = Arc::clone(&kp);
            let lh = thread::spawn(move || {
                if let Err(e) = kp_clone.listen() {
                    log::error!("Key presser error: {}", e);
                }
            });
            (kp, lh)
        };

        // clear listen_key_map
        key_presser.clear_listen_map();

        // 初始化 Speaker（每次都新建，stop 时会随 Engine 一起 drop）
        let speaker = Arc::new(Speaker::new()?);

        // 构建命令表
        let command_map: HashMap<String, Box<dyn Fn() + Send + Sync>> = config
            .commands
            .iter()
            .map(|cmd| -> Result<(String, Box<dyn Fn() + Send + Sync>)> {
                let key_presser_ref = Arc::clone(&key_presser);
                let speaker_ref = Arc::clone(&speaker);
                let keys = cmd.keys.clone();
                let audio_files = cmd.audio_files.clone();
                let audio_dir = audio_dir.clone();

                if cmd.command.is_empty() {
                    return Err(anyhow!("command must not be empty"));
                };
                KeyPresser::has_validity(keys.as_slice())?;

                Ok((
                    cmd.command.clone(),
                    Box::new(move || {
                        key_presser_ref.push(keys.as_slice());
                        if let Some(audio_path) = audio_files.choose(&mut rand::rng()) {
                            let audio_path = std::env::current_dir()
                                .unwrap()
                                .join(&audio_dir)
                                .join(audio_path);
                            let _ = speaker_ref.play_wav(audio_path.to_str().unwrap());
                        }
                    }) as Box<dyn Fn() + Send + Sync>,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let command = Arc::new(Command::new(command_map));
        let command_dic = command.keys().map(|x| x.to_string()).collect::<Vec<_>>();
        let matcher = Arc::new(Mutex::new(LevenshteinMatcher::new(command_dic)));
        let trigger = config.trigger.clone();

        let mut audio_recognizer_config: AudioRecognizerConfig = config.recognizer.clone().into();
        let mut grammar: Vec<String> = config
            .commands
            .iter()
            .map(|cmd| {
                let grammar = cmd.grammar.clone();
                if let Some(grammar) = grammar {
                    if !grammar.is_empty() {
                        return grammar;
                    }
                }
                cmd.command.clone().add_between_chars(" ")
            })
            .collect();

        if let Some(hit_word_grammar) = trigger.hit_word_grammar.clone().filter(|g| !g.is_empty()) {
            grammar.push(hit_word_grammar);
        } else if !&trigger.hit_word.is_empty() {
            grammar.push(trigger.hit_word.clone().unwrap().add_between_chars(" "));
        }

        audio_recognizer_config.set_grammar(grammar);
        let recognizer = AudioRecognizer::new(model_path, audio_recognizer_config)?;
        let mut processor =
            AudioBufferProcessor::new_with_input_device_name(recognizer, input_device)?;

        match config.recognizer.talk_mode {
            TalkMode::PushToTalk => {
                // listen push-to-talk key
                if let Some(ptt_input) = config.key_map.get(&LocalKey::PTT).cloned() {
                    let speech_ctrl = processor.get_speech_controller();
                    key_presser.listen_key(ptt_input, move |speaking| {
                        speech_ctrl.set_is_speaking(speaking);
                    });
                }
            }
            TalkMode::VoiceActivation => {
                // nothing to do
            }
        }

        let command_ref = Arc::clone(&command);
        let matcher_ref = Arc::clone(&matcher);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = Arc::clone(&cancel_flag);

        let on_result = Box::new(move |result: RecognitionResult| {
            if cancel_flag_clone.load(Ordering::Relaxed) {
                return;
            }

            let speech = result.text.trim();
            if speech.is_empty() {
                return;
            }

            let speech = speech.replace(" ", "");
            let hit_word = trigger.hit_word.clone();
            let command_to_match = if hit_word.is_empty() {
                info!("speech: {}", speech);
                speech
            } else {
                let hit_word = hit_word.unwrap();
                if let Some(pos) = speech.rfind(hit_word.as_str()) {
                    let command_str = &speech[pos + hit_word.len()..];
                    info!("speech: {} {}", hit_word, command_str);
                    command_str.to_string()
                } else {
                    warn!("miss required word '{}': {}", hit_word, speech);
                    return;
                }
            };

            if let Some(command) = matcher_ref
                .lock()
                .unwrap()
                .match_str(command_to_match.as_str())
            {
                info!("hit command: {}", command);
                command_ref.execute(command.as_str());
            } else {
                warn!("no matching command found: {}", command_to_match);
            }
        });

        processor.start(on_result)?;

        Ok(HellcallEngine {
            _processor: processor,
            _speaker: speaker,
            cancel_flag,
            _key_presser: key_presser,
            _listener_handle: Some(listener_handle),
        })
    }

    /// 停止引擎，消耗 self。
    ///
    /// drop 顺序（由字段声明顺序保证）：
    ///   1. `_processor` drop → 音频线程 join → on_result 闭包 drop → Arc<Command> drop
    ///      → 命令闭包 drop → 闭包内的 speaker_ref Arc 和 key_presser_ref Arc 全部 release
    ///   2. `_speaker` drop → 上一步所有 speaker_ref 已 release，此时引用计数归零
    ///      → Speaker::drop → tx drop → Speaker 线程退出
    ///   3. `cancel_flag` drop
    ///
    /// `_key_presser` 和 `_listener_handle` 转移给 `EngineHandle` 供 restart 复用，不在此 drop。
    /// （即使引擎持有的 Arc<KeyPresser> 被 release，rdev listener 线程也持有一份 clone，
    ///  KeyPresser worker 线程会随 listener 一起存活——这是设计决策，不是 bug。）
    pub fn stop(self) -> EngineHandle {
        self.cancel_flag.store(true, Ordering::Relaxed);
        let handle = EngineHandle {
            key_presser: self._key_presser,
            listener_handle: self._listener_handle.unwrap(),
        };
        // self 中剩余的 _processor、_speaker、cancel_flag 此处 drop（按声明顺序）
        handle
    }
}
