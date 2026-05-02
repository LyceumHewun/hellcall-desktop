#![allow(unused)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml::Value;

use super::core::audio::AudioRecognizerConfig;
use super::core::keypress::{Input, KeyPresserConfig, LocalKey};
use super::core::speaker::SpeakerRuntimeConfig;

fn default_capture_ratio() -> f32 {
    0.8
}

fn default_speaker_volume() -> f32 {
    1.7
}

fn default_speaker_speed() -> f32 {
    1.05
}

fn default_speaker_sleep_until_end() -> bool {
    true
}

fn default_monitor_local_playback() -> bool {
    true
}

fn default_virtual_mic_enabled() -> bool {
    false
}

fn default_virtual_mic_macro_volume() -> f32 {
    1.0
}

fn default_virtual_mic_input_volume() -> f32 {
    1.0
}

fn default_microphone_enable_denoise() -> bool {
    false
}

fn default_ai_llm_provider_id() -> String {
    "siliconflow".to_string()
}

fn default_ai_base_url() -> String {
    "https://api.siliconflow.cn/v1".to_string()
}

fn default_ai_chat_model() -> String {
    "deepseek-ai/DeepSeek-V3.2".to_string()
}

fn default_ai_llm_enabled() -> bool {
    true
}

fn default_ai_reply_enabled() -> bool {
    true
}

fn default_ai_context_event_count() -> usize {
    12
}

const AI_CONTEXT_EVENT_COUNT_OPTIONS: [usize; 5] = [4, 8, 12, 20, 50];

fn default_ai_stt_model_id() -> String {
    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17".to_string()
}

fn default_ai_tts_enabled() -> bool {
    true
}

fn default_ai_tts_model_id() -> String {
    "vits-melo-tts-zh_en".to_string()
}

fn default_ai_stt_language() -> String {
    "auto".to_string()
}

fn default_ai_tts_speed() -> f32 {
    1.0
}

fn default_ai_agent_id() -> String {
    "tactical-assistant".to_string()
}

fn default_ai_auto_execute_skills() -> bool {
    true
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
    pub mode: AppMode,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub vision: VisionConfig,
    #[serde(default)]
    pub microphone: MicrophoneConfig,
    #[serde(default)]
    pub speaker: SpeakerConfig,
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

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppMode {
    #[serde(rename = "voice_command")]
    #[default]
    VoiceCommand,
    #[serde(rename = "ai_agent")]
    AiAgent,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiConfig {
    #[serde(default)]
    pub llm: AiLlmConfig,
    #[serde(default)]
    pub speech: AiSpeechConfig,
    #[serde(default = "default_ai_auto_execute_skills")]
    pub auto_execute_skills: bool,
    #[serde(default = "default_ai_agent_id")]
    pub default_agent_id: String,
    #[serde(default = "default_ai_agents")]
    pub agents: Vec<AiAgentConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiLlmConfig {
    #[serde(default = "default_ai_llm_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ai_reply_enabled")]
    pub reply_enabled: bool,
    #[serde(default = "default_ai_context_event_count")]
    pub context_event_count: usize,
    #[serde(default)]
    pub decision: AiLlmStageConfig,
    #[serde(default)]
    pub reply: AiLlmStageConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub enum AiLlmProviderKind {
    #[serde(rename = "siliconflow")]
    SiliconFlow,
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiLlmProviderConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: AiLlmProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub chat_model: String,
    pub is_builtin: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiLlmStageConfig {
    #[serde(default)]
    pub kind: AiLlmProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub chat_model: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiSpeechConfig {
    #[serde(default)]
    pub stt: AiSpeechSttConfig,
    #[serde(default)]
    pub tts: AiSpeechTtsConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiSpeechSttConfig {
    #[serde(default = "default_ai_stt_model_id")]
    pub model_id: String,
    #[serde(default = "default_ai_stt_language")]
    pub language: String,
    #[serde(default)]
    pub use_itn: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiSpeechTtsConfig {
    #[serde(default = "default_ai_tts_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ai_tts_model_id")]
    pub model_id: String,
    #[serde(default)]
    pub speaker_id: i32,
    #[serde(default = "default_ai_tts_speed")]
    pub speed: f32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct AiAgentConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub persona_prompt: String,
    #[serde(default)]
    pub chat_model: String,
    #[serde(default)]
    pub decision_chat_model: String,
    #[serde(default)]
    pub reply_chat_model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub enable_thinking: bool,
    pub skill_ids: Vec<String>,
    pub is_builtin: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct SpeakerConfig {
    #[serde(default = "default_speaker_volume")]
    pub volume: f32,
    #[serde(default = "default_speaker_speed")]
    pub speed: f32,
    #[serde(default = "default_speaker_sleep_until_end")]
    pub sleep_until_end: bool,
    #[serde(default = "default_monitor_local_playback")]
    pub monitor_local_playback: bool,
    #[serde(default = "default_virtual_mic_enabled")]
    pub virtual_mic_enabled: bool,
    #[serde(default)]
    pub virtual_mic_device: Option<String>,
    #[serde(default = "default_virtual_mic_macro_volume")]
    pub virtual_mic_macro_volume: f32,
    #[serde(default = "default_virtual_mic_input_volume")]
    pub virtual_mic_input_volume: f32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct MicrophoneConfig {
    #[serde(default = "default_microphone_enable_denoise")]
    pub enable_denoise: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(default)]
pub struct RecognizerConfig {
    /// 音频识别的时间段 (秒)
    pub chunk_time: f32,
    /// 判断语音结束后的静音持续时间 (毫秒)
    pub vad_silence_duration: u64,
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
            mode: AppMode::default(),
            ai: AiConfig::default(),
            vision: VisionConfig::default(),
            microphone: MicrophoneConfig::default(),
            speaker: SpeakerConfig::default(),
            recognizer: RecognizerConfig::default(),
            key_presser: KeyPresserConfig::default(),
            key_map: HashMap::from([
                (LocalKey::UP, Input::Key(rdev::Key::KeyW)),
                (LocalKey::DOWN, Input::Key(rdev::Key::KeyS)),
                (LocalKey::LEFT, Input::Key(rdev::Key::KeyA)),
                (LocalKey::RIGHT, Input::Key(rdev::Key::KeyD)),
                (LocalKey::OPEN, Input::Key(rdev::Key::ControlLeft)),
                (LocalKey::THROW, Input::Button(rdev::Button::Left)),
            ]),
            trigger: TriggerConfig::default(),
            commands: Vec::new(),
        }
    }
}

fn default_ai_agents() -> Vec<AiAgentConfig> {
    vec![AiAgentConfig::default()]
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            llm: AiLlmConfig::default(),
            speech: AiSpeechConfig::default(),
            auto_execute_skills: default_ai_auto_execute_skills(),
            default_agent_id: default_ai_agent_id(),
            agents: default_ai_agents(),
        }
    }
}

impl Default for AiLlmConfig {
    fn default() -> Self {
        Self {
            enabled: default_ai_llm_enabled(),
            reply_enabled: default_ai_reply_enabled(),
            context_event_count: default_ai_context_event_count(),
            decision: AiLlmStageConfig::default(),
            reply: AiLlmStageConfig::default(),
        }
    }
}

impl Default for AiLlmProviderKind {
    fn default() -> Self {
        Self::SiliconFlow
    }
}

impl AiLlmProviderConfig {
    pub fn builtin_siliconflow() -> Self {
        Self {
            id: default_ai_llm_provider_id(),
            name: "SiliconFlow".to_string(),
            kind: AiLlmProviderKind::SiliconFlow,
            base_url: default_ai_base_url(),
            api_key: String::new(),
            chat_model: default_ai_chat_model(),
            is_builtin: true,
        }
    }
}

impl Default for AiLlmProviderConfig {
    fn default() -> Self {
        Self::builtin_siliconflow()
    }
}

impl AiLlmStageConfig {
    pub fn runtime_provider(&self, id: &str, name: &str) -> AiLlmProviderConfig {
        AiLlmProviderConfig {
            id: id.to_string(),
            name: name.to_string(),
            kind: self.kind.clone(),
            base_url: match self.kind {
                AiLlmProviderKind::SiliconFlow => default_ai_base_url(),
                AiLlmProviderKind::OpenAiCompatible => self.base_url.clone(),
            },
            api_key: self.api_key.clone(),
            chat_model: if self.chat_model.trim().is_empty() {
                default_ai_chat_model()
            } else {
                self.chat_model.clone()
            },
            is_builtin: false,
        }
    }
}

impl Default for AiLlmStageConfig {
    fn default() -> Self {
        Self {
            kind: AiLlmProviderKind::SiliconFlow,
            base_url: default_ai_base_url(),
            api_key: String::new(),
            chat_model: default_ai_chat_model(),
        }
    }
}

impl Default for AiSpeechConfig {
    fn default() -> Self {
        Self {
            stt: AiSpeechSttConfig::default(),
            tts: AiSpeechTtsConfig::default(),
        }
    }
}

impl Default for AiSpeechSttConfig {
    fn default() -> Self {
        Self {
            model_id: default_ai_stt_model_id(),
            language: default_ai_stt_language(),
            use_itn: true,
        }
    }
}

impl Default for AiSpeechTtsConfig {
    fn default() -> Self {
        Self {
            enabled: default_ai_tts_enabled(),
            model_id: default_ai_tts_model_id(),
            speaker_id: 0,
            speed: default_ai_tts_speed(),
        }
    }
}

impl Default for AiAgentConfig {
    fn default() -> Self {
        Self {
            id: default_ai_agent_id(),
            name: "战术副官".to_string(),
            description: "简洁、执行优先的全局作战助手".to_string(),
            persona_prompt: "你是 Hellcall 的战术副官。优先用简洁中文回答；当用户明确要求执行战备、输入方向指令或触发本地动作时，优先调用可用工具完成任务；不要编造工具执行结果。".to_string(),
            chat_model: String::new(),
            decision_chat_model: String::new(),
            reply_chat_model: String::new(),
            temperature: 0.7,
            max_tokens: 2048,
            enable_thinking: false,
            skill_ids: vec![
                "send_key_sequence".to_string(),
                "execute_stratagem".to_string(),
                "list_stratagems".to_string(),
                "get_key_mappings".to_string(),
            ],
            is_builtin: true,
        }
    }
}

impl Default for SpeakerConfig {
    fn default() -> Self {
        Self {
            volume: 1.0,
            speed: 1.0,
            sleep_until_end: true,
            monitor_local_playback: true,
            virtual_mic_enabled: false,
            virtual_mic_device: None,
            virtual_mic_macro_volume: 1.0,
            virtual_mic_input_volume: 1.0,
        }
    }
}

impl Default for MicrophoneConfig {
    fn default() -> Self {
        Self {
            enable_denoise: false,
        }
    }
}

impl From<SpeakerConfig> for SpeakerRuntimeConfig {
    fn from(config: SpeakerConfig) -> Self {
        Self {
            volume: config.volume,
            speed: config.speed,
            sleep_until_end: config.sleep_until_end,
            monitor_local_playback: config.monitor_local_playback,
            virtual_mic_enabled: config.virtual_mic_enabled,
            virtual_mic_device: config.virtual_mic_device,
            virtual_mic_macro_volume: config.virtual_mic_macro_volume,
            virtual_mic_input_volume: config.virtual_mic_input_volume,
        }
    }
}

impl Default for RecognizerConfig {
    fn default() -> Self {
        Self {
            chunk_time: 0.5,
            vad_silence_duration: 200,
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
                match base_table.get_mut(key) {
                    Some(base_value) => merge_toml_values(base_value, old_value),
                    None => {
                        // Keep extra table entries from older configs so map-like sections such as
                        // `key_map` do not lose optional user-defined bindings during upgrades.
                        base_table.insert(key.clone(), old_value.clone());
                    }
                };
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
            migrate_legacy_microphone_config(&mut base_value, &old_value);
            migrate_legacy_ai_config(&mut base_value, &old_value);

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

fn migrate_legacy_microphone_config(base_value: &mut Value, old_value: &Value) {
    let Some(old_denoise) = old_value
        .get("recognizer")
        .and_then(|recognizer| recognizer.get("enable_denoise"))
        .and_then(Value::as_bool)
    else {
        return;
    };

    let Some(base_table) = base_value.as_table_mut() else {
        return;
    };

    let microphone = base_table
        .entry("microphone")
        .or_insert_with(|| Value::Table(toml::map::Map::new()));
    let Some(microphone_table) = microphone.as_table_mut() else {
        return;
    };

    let should_migrate = microphone_table
        .get("enable_denoise")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        == false;

    if should_migrate {
        microphone_table.insert("enable_denoise".to_string(), Value::Boolean(old_denoise));
    }
}

fn sanitize_ai_context_event_count(value: usize) -> usize {
    if AI_CONTEXT_EVENT_COUNT_OPTIONS.contains(&value) {
        value
    } else {
        default_ai_context_event_count()
    }
}

fn llm_provider_to_stage(
    provider: AiLlmProviderConfig,
    chat_model: Option<String>,
) -> AiLlmStageConfig {
    AiLlmStageConfig {
        kind: provider.kind,
        base_url: provider.base_url,
        api_key: provider.api_key,
        chat_model: chat_model
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(provider.chat_model),
    }
}

fn legacy_agent_chat_model(old_ai: &toml::map::Map<String, Value>, key: &str) -> Option<String> {
    let default_agent_id = old_ai
        .get("default_agent_id")
        .and_then(Value::as_str)
        .map(str::to_string);

    old_ai
        .get("agents")
        .and_then(Value::as_array)
        .and_then(|agents| {
            agents.iter().find_map(|agent| {
                let agent_table = agent.as_table()?;
                if let Some(default_agent_id) = &default_agent_id {
                    let agent_id = agent_table.get("id").and_then(Value::as_str);
                    if agent_id != Some(default_agent_id.as_str()) {
                        return None;
                    }
                }
                agent_table
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
        .or_else(|| {
            old_ai
                .get("agents")
                .and_then(Value::as_array)
                .and_then(|agents| {
                    agents.iter().find_map(|agent| {
                        agent
                            .as_table()
                            .and_then(|agent_table| agent_table.get(key))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(str::to_string)
                    })
                })
        })
}

fn provider_from_legacy_llm(
    old_llm: &toml::map::Map<String, Value>,
    provider_id_key: &str,
) -> Option<AiLlmProviderConfig> {
    let providers = old_llm.get("providers").and_then(Value::as_array)?;
    let selected_id = old_llm
        .get(provider_id_key)
        .and_then(Value::as_str)
        .or_else(|| old_llm.get("selected_provider_id").and_then(Value::as_str));

    selected_id
        .and_then(|selected_id| {
            providers.iter().find_map(|provider| {
                let provider_config = provider.clone().try_into::<AiLlmProviderConfig>().ok()?;
                (provider_config.id == selected_id).then_some(provider_config)
            })
        })
        .or_else(|| {
            providers
                .first()
                .and_then(|provider| provider.clone().try_into::<AiLlmProviderConfig>().ok())
        })
}

fn migrate_legacy_ai_config(base_value: &mut Value, old_value: &Value) {
    let Some(old_ai) = old_value.get("ai").and_then(Value::as_table) else {
        return;
    };

    let Some(base_table) = base_value.as_table_mut() else {
        return;
    };

    let ai = base_table
        .entry("ai")
        .or_insert_with(|| Value::Table(toml::map::Map::new()));
    let Some(ai_table) = ai.as_table_mut() else {
        return;
    };

    let legacy_provider = old_ai
        .get("provider")
        .and_then(Value::as_str)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let legacy_base_url = old_ai
        .get("base_url")
        .and_then(Value::as_str)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let legacy_api_key = old_ai
        .get("api_key")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let legacy_chat_model = old_ai
        .get("default_chat_model")
        .and_then(Value::as_str)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let legacy_tts_enabled = old_ai.get("tts_enabled").and_then(Value::as_bool);

    let has_legacy_llm_fields = legacy_provider.is_some()
        || legacy_base_url.is_some()
        || legacy_api_key.is_some()
        || legacy_chat_model.is_some();

    if has_legacy_llm_fields {
        let provider_kind = match legacy_provider {
            Some("siliconflow") | None => AiLlmProviderKind::SiliconFlow,
            _ => AiLlmProviderKind::OpenAiCompatible,
        };

        let provider = AiLlmProviderConfig {
            id: legacy_provider
                .map(sanitize_ai_provider_id)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(default_ai_llm_provider_id),
            name: match provider_kind {
                AiLlmProviderKind::SiliconFlow => "SiliconFlow".to_string(),
                AiLlmProviderKind::OpenAiCompatible => "Custom Provider".to_string(),
            },
            kind: provider_kind,
            base_url: legacy_base_url
                .map(ToString::to_string)
                .unwrap_or_else(default_ai_base_url),
            api_key: legacy_api_key.unwrap_or_default(),
            chat_model: legacy_chat_model
                .map(ToString::to_string)
                .unwrap_or_else(default_ai_chat_model),
            is_builtin: false,
        };
        let decision_stage = llm_provider_to_stage(
            provider.clone(),
            legacy_agent_chat_model(old_ai, "decision_chat_model"),
        );
        let reply_stage = llm_provider_to_stage(
            provider,
            legacy_agent_chat_model(old_ai, "reply_chat_model"),
        );

        ai_table.insert(
            "llm".to_string(),
            Value::try_from(AiLlmConfig {
                enabled: default_ai_llm_enabled(),
                reply_enabled: default_ai_reply_enabled(),
                context_event_count: default_ai_context_event_count(),
                decision: decision_stage,
                reply: reply_stage,
            })
            .unwrap_or_else(|_| Value::Table(toml::map::Map::new())),
        );
    }

    if let Some(llm_table) = old_ai.get("llm").and_then(Value::as_table) {
        let decision_model = legacy_agent_chat_model(old_ai, "decision_chat_model");
        let reply_model = legacy_agent_chat_model(old_ai, "reply_chat_model");

        let decision_stage = provider_from_legacy_llm(llm_table, "decision_provider_id")
            .map(|provider| llm_provider_to_stage(provider, decision_model));
        let reply_stage = provider_from_legacy_llm(llm_table, "reply_provider_id")
            .map(|provider| llm_provider_to_stage(provider, reply_model));

        let llm = ai_table
            .entry("llm")
            .or_insert_with(|| Value::try_from(AiLlmConfig::default()).unwrap());
        if let Some(llm_base_table) = llm.as_table_mut() {
            if let Some(count) = llm_base_table
                .get("context_event_count")
                .and_then(Value::as_integer)
                .and_then(|value| usize::try_from(value).ok())
            {
                llm_base_table.insert(
                    "context_event_count".to_string(),
                    Value::Integer(sanitize_ai_context_event_count(count) as i64),
                );
            }
            if let Some(stage) = decision_stage {
                llm_base_table.insert(
                    "decision".to_string(),
                    Value::try_from(stage).unwrap_or_else(|_| Value::Table(toml::map::Map::new())),
                );
            }
            if let Some(stage) = reply_stage {
                llm_base_table.insert(
                    "reply".to_string(),
                    Value::try_from(stage).unwrap_or_else(|_| Value::Table(toml::map::Map::new())),
                );
            }
        }
    } else if let Some(llm_base_table) = ai_table.get_mut("llm").and_then(Value::as_table_mut) {
        if let Some(count) = llm_base_table
            .get("context_event_count")
            .and_then(Value::as_integer)
            .and_then(|value| usize::try_from(value).ok())
        {
            llm_base_table.insert(
                "context_event_count".to_string(),
                Value::Integer(sanitize_ai_context_event_count(count) as i64),
            );
        }
    }

    if let Some(agents) = ai_table.get_mut("agents").and_then(Value::as_array_mut) {
        for agent in agents {
            let Some(agent_table) = agent.as_table_mut() else {
                continue;
            };
            if !agent_table.contains_key("persona_prompt") {
                if let Some(system_prompt) = agent_table.get("system_prompt").cloned() {
                    agent_table.insert("persona_prompt".to_string(), system_prompt);
                }
            }
            if !agent_table.contains_key("reply_chat_model") {
                if let Some(chat_model) = agent_table.get("chat_model").cloned() {
                    agent_table.insert("reply_chat_model".to_string(), chat_model);
                }
            }
        }
    }

    if let Some(enabled) = legacy_tts_enabled {
        let speech = ai_table
            .entry("speech")
            .or_insert_with(|| Value::try_from(AiSpeechConfig::default()).unwrap());
        let Some(speech_table) = speech.as_table_mut() else {
            return;
        };
        let tts = speech_table
            .entry("tts")
            .or_insert_with(|| Value::try_from(AiSpeechTtsConfig::default()).unwrap());
        if let Some(tts_table) = tts.as_table_mut() {
            tts_table.insert("enabled".to_string(), Value::Boolean(enabled));
        }
    }
}

fn sanitize_ai_provider_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    sanitized.trim_matches('-').to_string()
}
