#![allow(unused)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::core::audio::AudioRecognizerConfig;
use super::core::keypress::{Input, KeyPresserConfig, LocalKey};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub recognizer: RecognizerConfig,
    pub key_presser: KeyPresserConfig,
    /// 按键映射
    ///
    /// 示例:
    /// ```toml
    /// [key_map]
    /// UP = "KeyW"
    /// DOWN = "KeyS"
    /// LEFT = "KeyA"
    /// RIGHT = "KeyD"
    /// OPEN = "ControlLeft"
    /// ```
    ///
    /// 更多按键信息请参考: https://docs.rs/rdev/latest/rdev/enum.Key.html
    pub key_map: HashMap<LocalKey, Input>,
    pub trigger: TriggerConfig,
    pub commands: Vec<CommandConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RecognizerConfig {
    /// 音频识别的时间段 (秒)
    pub chunk_time: f32,
    /// 判断语音结束后的静音持续时间 (毫秒)
    pub vad_silence_duration: u64,
    /// 是否开启降噪
    #[serde(default)]
    pub enable_denoise: bool,
    /// 语音识别的模式
    #[serde(default)]
    pub talk_mode: TalkMode,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum TalkMode {
    #[serde(rename = "push_to_talk")]
    PushToTalk,
    #[serde(rename = "voice_activation")]
    #[default]
    VoiceActivation,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TriggerConfig {
    pub hit_word: Option<String>,
    pub hit_word_grammar: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CommandConfig {
    pub command: String,
    pub grammar: Option<String>,
    pub shortcut: Option<Input>,
    pub keys: Vec<LocalKey>,
    pub audio_files: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            recognizer: RecognizerConfig::default(),
            key_presser: KeyPresserConfig::default(),
            key_map: HashMap::from([
                (LocalKey::UP, Input::Key(rdev::Key::KeyW)),
                (LocalKey::DOWN, Input::Key(rdev::Key::KeyS)),
                (LocalKey::LEFT, Input::Key(rdev::Key::KeyA)),
                (LocalKey::RIGHT, Input::Key(rdev::Key::KeyD)),
                (LocalKey::OPEN, Input::Key(rdev::Key::ControlLeft)),
                (LocalKey::THROW, Input::Button(rdev::Button::Left)),
                (LocalKey::RESEND, Input::Key(rdev::Key::BackQuote)),
            ]),
            trigger: TriggerConfig::default(),
            commands: Vec::new(),
        }
    }
}

impl Default for RecognizerConfig {
    fn default() -> Self {
        Self {
            chunk_time: 0.5,
            vad_silence_duration: 200,
            enable_denoise: false,
            talk_mode: TalkMode::VoiceActivation,
        }
    }
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            hit_word: None,
            hit_word_grammar: None,
        }
    }
}

impl Into<AudioRecognizerConfig> for RecognizerConfig {
    fn into(self) -> AudioRecognizerConfig {
        AudioRecognizerConfig {
            chunk_time: self.chunk_time,
            grammar: Vec::new(),
            vad_silence_duration: self.vad_silence_duration,
            enable_denoise: self.enable_denoise,
            is_ptt: self.talk_mode == TalkMode::PushToTalk,
        }
    }
}
