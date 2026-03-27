#![allow(unused)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml::Value;

use super::core::audio::AudioRecognizerConfig;
use super::core::keypress::{Input, KeyPresserConfig, LocalKey};

fn default_capture_ratio() -> f32 {
    0.8
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VisionConfig {
    #[serde(default)]
    pub enable_occ: bool,
    /// Capture ratio for the center crop (e.g., 0.8 means crop 80% of the shortest edge).
    /// Default should be 0.8 or similar, but we will use serde default.
    #[serde(default = "default_capture_ratio")]
    pub capture_ratio: f32,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            enable_occ: true,
            capture_ratio: 0.8,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub vision: VisionConfig,
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
#[serde(default)]
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
#[serde(default)]
pub struct TriggerConfig {
    pub hit_word: Option<String>,
    pub hit_word_grammar: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
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
            vision: VisionConfig::default(),
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

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            grammar: None,
            shortcut: None,
            keys: Vec::new(),
            audio_files: Vec::new(),
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

/// Deeply merges `old` into `base`, preserving only fields that still exist and match the
/// expected TOML shape in the latest config schema.
fn merge_toml_values(base: &mut Value, old: &Value) {
    match (base, old) {
        (Value::Table(base_table), Value::Table(old_table)) => {
            for (key, old_value) in old_table {
                if let Some(base_value) = base_table.get_mut(key) {
                    merge_toml_values(base_value, old_value);
                }
            }
        }
        (Value::Array(base_array), Value::Array(old_array)) => {
            // Arrays such as [[commands]] are user-authored lists, so replacing the whole array
            // preserves intent better than attempting an item-by-item merge.
            *base_array = old_array.clone();
        }
        (base_value, old_value) if base_value.same_type(old_value) => {
            *base_value = old_value.clone();
        }
        _ => {}
    }
}

pub fn save_config_to_path(config_path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = config_path.parent().filter(|parent| !parent.exists()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let toml_string = toml::to_string(config).map_err(|e| e.to_string())?;
    fs::write(config_path, toml_string).map_err(|e| e.to_string())
}

pub fn load_config_from_path(config_path: &Path) -> Result<Config, String> {
    let default_config = Config::default();

    if let Some(parent) = config_path.parent().filter(|parent| !parent.exists()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if !config_path.exists() {
        save_config_to_path(config_path, &default_config)?;
        return Ok(default_config);
    }

    let file_content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let mut base_value = Value::try_from(&default_config).map_err(|e| e.to_string())?;

    match toml::from_str::<Value>(&file_content) {
        Ok(old_value) => {
            merge_toml_values(&mut base_value, &old_value);

            let final_config = base_value.try_into().unwrap_or_else(|e| {
                log::warn!(
                    "Merged config could not be deserialized, falling back to defaults: {}",
                    e
                );
                default_config.clone()
            });

            save_config_to_path(config_path, &final_config)?;
            Ok(final_config)
        }
        Err(e) => {
            log::error!(
                "Config has invalid TOML syntax, backing up and resetting: {}",
                e
            );
            let bak_path = config_path.with_extension("toml.bak");
            let _ = fs::rename(config_path, &bak_path);

            save_config_to_path(config_path, &default_config)?;
            Ok(default_config)
        }
    }
}
