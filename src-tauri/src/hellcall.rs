pub mod config;
pub mod core;

pub use config::{load_config_from_path, save_config_to_path, Config};

use anyhow::{anyhow, Result};
use log::{info, warn};
use rand::seq::IndexedRandom;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;

use self::config::TalkMode;
use self::core::audio::*;
use self::core::command::*;
use self::core::keypress::*;
use self::core::matcher::*;
use self::core::microphone::*;
use self::core::speaker::*;
use self::core::vision::YoloEngine;
use crate::utils::StringUtils;

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
        vosk_model_path: &str,
        input_device_name: Option<String>,
        audio_dir: Option<String>,
        vision_model_path: Option<String>,
    ) -> Result<HellcallEngine> {
        HellcallEngine::start_inner(
            config,
            vosk_model_path,
            input_device_name,
            audio_dir,
            vision_model_path,
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
    /// YOLO 推理引擎（可选，仅在 VISION key 绑定时加载）
    _yolo_engine: Option<Arc<YoloEngine>>,
}

impl HellcallEngine {
    pub fn start(
        config: Config,
        vosk_model_path: &str,
        input_device_name: Option<String>,
        audio_dir: Option<String>,
        vision_model_path: Option<String>,
    ) -> Result<Self> {
        Self::start_inner(
            config,
            vosk_model_path,
            input_device_name,
            audio_dir,
            vision_model_path,
            None,
        )
    }

    fn start_inner(
        config: Config,
        vosk_model_path: &str,
        input_device_name: Option<String>,
        audio_dir: Option<String>,
        vision_model_path: Option<String>,
        existing: Option<(Arc<KeyPresser>, thread::JoinHandle<()>)>,
    ) -> Result<Self> {
        // 选择输入设备
        let input_device = resolve_input_device_name(input_device_name)?;
        info!("input_device_name: {}", input_device);

        // 选择音频目录
        let audio_dir = if let Some(dir) = audio_dir.filter(|d| !d.is_empty()) {
            dir
        } else {
            AUDIO_DIR.to_string()
        };

        if config.commands.is_empty() {
            return Err(anyhow!("at least one command must be configured"));
        }

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
        let speaker = Arc::new(Speaker::new(
            config.speaker.clone().into(),
            &input_device,
            config.microphone.enable_denoise,
        )?);

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
        let mut normalized_command_lookup = HashMap::new();
        let command_dic = command
            .keys()
            .filter_map(|command| {
                let normalized = command.normalize_text_for_matching();
                if normalized.is_empty() {
                    return None;
                }

                if let Some(existing) = normalized_command_lookup.get(&normalized) {
                    warn!(
                        "normalized command collision: '{}' and '{}' both map to '{}'",
                        existing, command, normalized
                    );
                } else {
                    normalized_command_lookup.insert(normalized.clone(), command.to_string());
                }

                Some(normalized)
            })
            .collect::<Vec<_>>();
        let normalized_command_lookup = Arc::new(normalized_command_lookup);
        let matcher = Arc::new(Mutex::new(LevenshteinMatcher::new(command_dic)));
        let trigger = config.trigger.clone();

        let mut audio_recognizer_config: AudioRecognizerConfig = config.recognizer.clone().into();
        let mut grammar: Vec<String> = config
            .commands
            .iter()
            .filter_map(|cmd| {
                cmd.grammar
                    .as_deref()
                    .map(StringUtils::collapse_whitespace)
                    .filter(|grammar| !grammar.is_empty())
                    .or_else(|| cmd.command.build_default_vosk_grammar())
            })
            .collect();

        if let Some(hit_word_grammar) = trigger.hit_word_grammar.clone().filter(|g| !g.is_empty()) {
            grammar.push(hit_word_grammar.collapse_whitespace());
        } else if let Some(hit_word) = trigger.hit_word.as_deref() {
            if let Some(default_hit_word_grammar) = hit_word.build_default_vosk_grammar() {
                grammar.push(default_hit_word_grammar);
            }
        }

        audio_recognizer_config.set_grammar(grammar);
        let recognizer = AudioRecognizer::new(vosk_model_path, audio_recognizer_config)?;
        let mut processor = AudioBufferProcessor::new_with_input_device_name(
            recognizer,
            input_device,
            config.microphone.enable_denoise,
            speaker.mic_passthrough(),
        )?;

        match config.recognizer.talk_mode {
            TalkMode::PushToTalk => {
                // listen push-to-talk key
                if let Some(ptt_input) = config.key_map.get(&LocalKey::PTT).cloned() {
                    let speech_ctrl = processor.get_speech_controller();
                    key_presser.listen_key(ptt_input, move |speaking, _push_fn| {
                        speech_ctrl.set_is_speaking(speaking);
                    });
                }
            }
            TalkMode::VoiceActivation => {
                // nothing to do
            }
        }

        let vision_config = &config.vision;
        let yolo_engine = if vision_config.enable_occ {
            let loaded_engine = if let Some(vision_model_path) =
                vision_model_path.filter(|path| !path.is_empty())
            {
                match YoloEngine::new(&vision_model_path) {
                    Ok(engine) => {
                        log::info!("YoloEngine loaded: {:?}", vision_model_path);
                        Some(Arc::new(engine))
                    }
                    Err(e) => {
                        log::warn!("YoloEngine failed to load: {}", e);
                        None
                    }
                }
            } else {
                log::warn!("Vision model is not downloaded; OCC will remain unavailable");
                None
            };

            // engine 成功加载注册热键监听
            if let (Some(engine_arc), Some(occ_input)) =
                (&loaded_engine, config.key_map.get(&LocalKey::OCC).cloned())
            {
                let engine_ref = engine_arc.clone();
                let capture_ratio = vision_config.capture_ratio;

                key_presser.listen_key(occ_input, move |pressed, push_fn| {
                    if !pressed {
                        return;
                    }

                    log::trace!("OCC triggered");
                    let engine_clone = engine_ref.clone();

                    std::thread::spawn(move || {
                        // 4. 拉平线程内部逻辑，使用 Early Return
                        let sequence = match crate::hellcall::core::vision::recognize_console_arrows(&engine_clone, capture_ratio) {
                            Ok(seq) => seq,
                            Err(e) => {
                                log::error!("Vision pipeline failed: {}", e);
                                return;
                            }
                        };

                        if sequence.is_empty() {
                            log::warn!("No valid sequence found");
                            return;
                        }

                        log::debug!("Vision sequence recognized: {:?}", sequence);

                        let keys: Vec<LocalKey> = sequence
                            .iter()
                            .filter_map(|cmd| match cmd.as_str() {
                                "UP" => Some(LocalKey::UP),
                                "DOWN" => Some(LocalKey::DOWN),
                                "LEFT" => Some(LocalKey::LEFT),
                                "RIGHT" => Some(LocalKey::RIGHT),
                                _ => None,
                            })
                            .collect();

                        push_fn(keys, true);
                    });
                });
            }

            loaded_engine
        } else {
            None
        };

        let command_ref = Arc::clone(&command);
        let normalized_command_lookup_ref = Arc::clone(&normalized_command_lookup);
        let matcher_ref = Arc::clone(&matcher);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = Arc::clone(&cancel_flag);
        let normalized_hit_word = trigger
            .hit_word
            .as_deref()
            .map(StringUtils::normalize_text_for_matching)
            .filter(|hit_word| !hit_word.is_empty());

        let on_result = Box::new(move |result: RecognitionResult| {
            if cancel_flag_clone.load(Ordering::Relaxed) {
                return;
            }

            let speech = result.text.trim();
            if speech.is_empty() {
                return;
            }

            let speech = speech.normalize_text_for_matching();
            let command_to_match = if normalized_hit_word.is_none() {
                info!("speech: {}", speech);
                speech
            } else {
                let hit_word = normalized_hit_word.as_ref().unwrap();
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
                if let Some(original_command) = normalized_command_lookup_ref.get(&command) {
                    info!("hit command: {}", original_command);
                    command_ref.execute(original_command.as_str());
                } else {
                    warn!(
                        "matched normalized command '{}' but no original command was found",
                        command
                    );
                }
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
            _yolo_engine: yolo_engine,
        })
    }

    pub fn update_speaker_config(&self, config: SpeakerRuntimeConfig) {
        self._speaker.update_config(config);
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

#[cfg(test)]
mod tests {
    use crate::utils::StringUtils;

    #[test]
    fn default_vosk_grammar_splits_cjk_chars() {
        assert_eq!(
            "快速开火".build_default_vosk_grammar(),
            Some("快 速 开 火".to_string())
        );
    }

    #[test]
    fn default_vosk_grammar_preserves_latin_words() {
        assert_eq!(
            "open fire now".build_default_vosk_grammar(),
            Some("open fire now".to_string())
        );
    }

    #[test]
    fn default_vosk_grammar_supports_mixed_scripts() {
        assert_eq!(
            "alpha 快速 fire".build_default_vosk_grammar(),
            Some("alpha 快 速 fire".to_string())
        );
    }

    #[test]
    fn default_vosk_grammar_preserves_hangul_words() {
        assert_eq!(
            "빠른 발사".build_default_vosk_grammar(),
            Some("빠른 발사".to_string())
        );
    }

    #[test]
    fn matching_normalization_removes_spaces_and_lowercases_latin_text() {
        assert_eq!(
            "Open   Fire".normalize_text_for_matching(),
            "openfire".to_string()
        );
    }
}
