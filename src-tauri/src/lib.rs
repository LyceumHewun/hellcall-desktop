mod ai;
mod asset_manager;
mod hellcall;
mod stratagems;
mod utils;

use ai::types::{AiSessionEvent, AiSessionRecord, AiSessionSummary};
use asset_manager::{
    sherpa_model_manager, sherpa_runtime_manager, vision_model_manager, vosk_model_manager,
};
use hellcall::config::{AiConfig, AiLlmProviderConfig, MicrophoneConfig, SpeakerConfig};
use hellcall::core::keypress::{Input, KeyPresser, LocalKey};
use hellcall::core::speaker::Speaker;
use hellcall::core::microphone::{
    open_input_stream, open_volume_meter_stream, resolve_input_device_name,
    run_processed_audio_pipeline, validate_virtual_output_device_for_mix,
};
use hellcall::{load_config_from_path, save_config_to_path, Config, EngineHandle, HellcallEngine};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};
use stratagems::StratagemCatalog;
use serde_json::{json, Value};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_log::{Target, TargetKind};

const DEFAULT_AI_SESSION_ID: &str = "default-session";
const DEFAULT_AI_SESSION_TITLE: &str = "Current Session";

enum AppEngine {
    None,
    Running(HellcallEngine),
    Stopped(EngineHandle),
}

struct UnsafeStreamWrapper(cpal::Stream);
unsafe impl Send for UnsafeStreamWrapper {}
unsafe impl Sync for UnsafeStreamWrapper {}

#[derive(Clone)]
struct AiRuntimeContext {
    mode: hellcall::config::AppMode,
    selected_device: Option<String>,
    microphone_config: MicrophoneConfig,
    speaker_config: SpeakerConfig,
    ai_config: AiConfig,
    key_map: HashMap<LocalKey, Input>,
    key_presser_config: hellcall::core::keypress::KeyPresserConfig,
    session_id: Option<String>,
}

impl Default for AiRuntimeContext {
    fn default() -> Self {
        Self {
            mode: hellcall::config::AppMode::VoiceCommand,
            selected_device: None,
            microphone_config: MicrophoneConfig::default(),
            speaker_config: SpeakerConfig::default(),
            ai_config: AiConfig::default(),
            key_map: Config::default().key_map,
            key_presser_config: hellcall::core::keypress::KeyPresserConfig::default(),
            session_id: None,
        }
    }
}

struct AiRecordingCapture {
    samples: Arc<Mutex<Vec<i16>>>,
    stream: Option<UnsafeStreamWrapper>,
    worker_handle: Option<JoinHandle<()>>,
}

struct AiHotkeyBridge {
    key_presser: Arc<KeyPresser>,
    _listener_handle: Option<JoinHandle<()>>,
}

struct AiSpeakerBridge {
    speaker: Speaker,
    selected_device: Option<String>,
    monitor_local_playback: bool,
    virtual_mic_device: Option<String>,
    microphone_enable_denoise: bool,
}

#[derive(Clone, serde::Serialize)]
struct AiRecordingStatePayload {
    recording: bool,
}

#[derive(Clone, serde::Serialize)]
struct AiChatStatePayload {
    streaming: bool,
}

#[derive(Clone, serde::Serialize)]
struct AiTranscriptionEventPayload {
    session_id: String,
    transcript: String,
}

#[derive(Clone, serde::Serialize)]
struct AiErrorEventPayload {
    message: String,
}

#[derive(Clone, serde::Serialize)]
struct AiAgentErrorEventPayload {
    message: String,
}

#[derive(Clone, serde::Serialize)]
struct AiWarmupStatePayload {
    stage: String,
}

#[derive(Clone, serde::Serialize)]
struct AiChatDeltaPayload {
    session_id: String,
    delta: String,
}

#[derive(Clone, serde::Serialize)]
struct AiChatFinishedPayload {
    session_id: String,
    message: String,
}

#[derive(Clone, serde::Serialize)]
struct AiToolEventPayload {
    id: String,
    session_id: String,
    phase: String,
    name: String,
    summary: String,
}

struct AppState {
    engine: Mutex<AppEngine>,
    mic_test_stream: Mutex<Option<UnsafeStreamWrapper>>,
    cached_vosk_runtime_model_paths: Mutex<HashMap<String, PathBuf>>,
    ai_session: Mutex<AiSessionRecord>,
    ai_runtime_context: Mutex<AiRuntimeContext>,
    ai_recording: Mutex<Option<AiRecordingCapture>>,
    ai_hotkey_bridge: Mutex<Option<AiHotkeyBridge>>,
    ai_speaker: Mutex<Option<AiSpeakerBridge>>,
    ai_sherpa: Mutex<Option<ai::sherpa::SherpaSpeechRuntime>>,
    ai_enabled: Mutex<bool>,
    ai_warmup_in_progress: Mutex<bool>,
    ai_streaming: Mutex<bool>,
}

fn resolve_audio_dir(app_handle: &AppHandle) -> Result<std::path::PathBuf, String> {
    let mut candidates = Vec::new();

    match app_handle.path().resolve("audio/", BaseDirectory::Resource) {
        Ok(path) => candidates.push(path),
        Err(error) => {
            log::warn!("Failed to resolve bundled audio path: {}", error);
        }
    }

    let current_dir = std::env::current_dir()
        .map_err(|e| utils::format_and_log_error("Failed to get current directory", e))?;
    candidates.push(current_dir.join("audio"));
    if let Some(parent_dir) = current_dir.parent() {
        candidates.push(parent_dir.join("audio"));
    }

    for candidate in candidates {
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    Err(
        "Audio directory not found. Expected bundled resources or a local ./audio folder."
            .to_string(),
    )
}

fn resolve_cached_vosk_runtime_model_path(
    app_handle: &AppHandle,
    state: &State<'_, AppState>,
    model_id: &str,
) -> Result<PathBuf, String> {
    let cache_key = model_id.trim();

    {
        let cache_guard = state
            .cached_vosk_runtime_model_paths
            .lock()
            .map_err(|e| e.to_string())?;
        if let Some(cached_path) = cache_guard.get(cache_key) {
            log::debug!(
                "Using cached Vosk runtime path for model '{}': {}",
                cache_key,
                cached_path.display()
            );
            return Ok(cached_path.clone());
        }
    }

    let resolved_path = vosk_model_manager::resolve_runtime_model_path(app_handle, cache_key)?;

    let mut cache_guard = state
        .cached_vosk_runtime_model_paths
        .lock()
        .map_err(|e| e.to_string())?;
    cache_guard.insert(cache_key.to_string(), resolved_path.clone());

    Ok(resolved_path)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn default_ai_session_record() -> AiSessionRecord {
    let now = now_ms();
    AiSessionRecord {
        summary: AiSessionSummary {
            id: DEFAULT_AI_SESSION_ID.to_string(),
            title: DEFAULT_AI_SESSION_TITLE.to_string(),
            created_at_ms: now,
            updated_at_ms: now,
            message_count: 0,
        },
        events: Vec::new(),
    }
}

fn current_ai_session_record(state: &State<'_, AppState>) -> Result<AiSessionRecord, String> {
    state
        .ai_session
        .lock()
        .map_err(|e| e.to_string())
        .map(|session| session.clone())
}

fn reset_ai_session_record(state: &State<'_, AppState>) -> Result<AiSessionSummary, String> {
    let mut session = state.ai_session.lock().map_err(|e| e.to_string())?;
    *session = default_ai_session_record();
    Ok(session.summary.clone())
}

fn append_ai_session_event(
    state: &State<'_, AppState>,
    event: AiSessionEvent,
) -> Result<(), String> {
    let mut session = state.ai_session.lock().map_err(|e| e.to_string())?;
    session.events.push(event);
    session.summary.message_count = session.events.len();
    session.summary.updated_at_ms = now_ms();
    Ok(())
}

fn create_session_if_missing(app_handle: &AppHandle, runtime: &mut AiRuntimeContext) -> Result<String, String> {
    let _ = app_handle;
    if runtime.session_id.as_deref() != Some(DEFAULT_AI_SESSION_ID) {
        runtime.session_id = Some(DEFAULT_AI_SESSION_ID.to_string());
    }
    Ok(DEFAULT_AI_SESSION_ID.to_string())
}

fn emit_ai_recording_state(app_handle: &AppHandle, recording: bool) {
    let _ = app_handle.emit(
        "ai-recording-state",
        AiRecordingStatePayload { recording },
    );
}

fn emit_ai_error(app_handle: &AppHandle, message: impl Into<String>) {
    let _ = app_handle.emit(
        "ai-recording-error",
        AiErrorEventPayload {
            message: message.into(),
        },
    );
}

fn emit_ai_agent_error(app_handle: &AppHandle, message: impl Into<String>) {
    let _ = app_handle.emit(
        "ai-agent-error",
        AiAgentErrorEventPayload {
            message: message.into(),
        },
    );
}

fn emit_ai_warmup_state(app_handle: &AppHandle, stage: impl Into<String>) {
    let _ = app_handle.emit(
        "ai-warmup-state",
        AiWarmupStatePayload {
            stage: stage.into(),
        },
    );
}

fn configure_ai_hotkey_bridge(
    app_handle: &AppHandle,
    state: &State<'_, AppState>,
    runtime: &AiRuntimeContext,
) -> Result<(), String> {
    let mut bridge_guard = state.ai_hotkey_bridge.lock().map_err(|e| e.to_string())?;

    let ptt_input = if runtime.mode == hellcall::config::AppMode::AiAgent {
        runtime.key_map.get(&LocalKey::PTT).cloned()
    } else {
        None
    };

    let key_presser = if let Some(bridge) = bridge_guard.as_mut() {
        bridge
            .key_presser
            .update_config(
                runtime.key_presser_config.clone(),
                runtime.key_map.clone(),
                HashMap::new(),
            )
            .map_err(|e| e.to_string())?;
        bridge.key_presser.clear_listen_map();
        Arc::clone(&bridge.key_presser)
    } else {
        let key_presser = Arc::new(
            KeyPresser::new(
                runtime.key_presser_config.clone(),
                runtime.key_map.clone(),
                HashMap::new(),
            )
            .map_err(|e| e.to_string())?,
        );
        let listener_key_presser = Arc::clone(&key_presser);
        let listener_handle = std::thread::spawn(move || {
            if let Err(error) = listener_key_presser.listen() {
                log::error!("AI hotkey listener failed: {}", error);
            }
        });

        bridge_guard.replace(AiHotkeyBridge {
            key_presser: Arc::clone(&key_presser),
            _listener_handle: Some(listener_handle),
        });
        key_presser
    };

    if let Some(ptt_input) = ptt_input {
        let app_handle_clone = app_handle.clone();
        key_presser.listen_key(ptt_input, move |pressed, _push_fn| {
            let app_handle = app_handle_clone.clone();
            if pressed {
                if let Err(error) = start_ai_recording_internal(&app_handle) {
                    emit_ai_error(&app_handle, error);
                }
            } else {
                std::thread::spawn(move || {
                    let runtime = match app_handle.state::<AppState>().ai_runtime_context.lock() {
                        Ok(guard) => guard.clone(),
                        Err(error) => {
                            emit_ai_error(&app_handle, error.to_string());
                            return;
                        }
                    };

                    if let Err(error) = stop_ai_recording_blocking(&app_handle, runtime) {
                        emit_ai_error(&app_handle, error);
                    }
                });
            }
        });
    }

    Ok(())
}

fn sync_ai_speaker_bridge(
    state: &State<'_, AppState>,
    runtime: &AiRuntimeContext,
) -> Result<(), String> {
    let mut speaker_guard = state.ai_speaker.lock().map_err(|e| e.to_string())?;

    if runtime.mode != hellcall::config::AppMode::AiAgent || !runtime.ai_config.speech.tts.enabled {
        *speaker_guard = None;
        return Ok(());
    }

    let recreate_required = speaker_guard.as_ref().is_none_or(|bridge| {
        bridge.selected_device != runtime.selected_device
            || bridge.monitor_local_playback != runtime.speaker_config.monitor_local_playback
            || bridge.virtual_mic_device != runtime.speaker_config.virtual_mic_device
            || bridge.microphone_enable_denoise != runtime.microphone_config.enable_denoise
    });

    if recreate_required {
        let input_device_name =
            resolve_input_device_name(runtime.selected_device.clone()).map_err(|e| e.to_string())?;
        let speaker = Speaker::new(
            runtime.speaker_config.clone().into(),
            &input_device_name,
            runtime.microphone_config.enable_denoise,
        )
        .map_err(|e| utils::format_and_log_error("Failed to create AI speaker bridge", e))?;

        *speaker_guard = Some(AiSpeakerBridge {
            speaker,
            selected_device: runtime.selected_device.clone(),
            monitor_local_playback: runtime.speaker_config.monitor_local_playback,
            virtual_mic_device: runtime.speaker_config.virtual_mic_device.clone(),
            microphone_enable_denoise: runtime.microphone_config.enable_denoise,
        });
    } else if let Some(bridge) = speaker_guard.as_mut() {
        bridge.speaker.update_config(runtime.speaker_config.clone().into());
    }

    Ok(())
}

fn start_ai_recording_internal(app_handle: &AppHandle) -> Result<(), String> {
    use cpal::traits::StreamTrait;

    let state = app_handle.state::<AppState>();
    ensure_ai_enabled(&state)?;
    let mut recording_guard = state.ai_recording.lock().map_err(|e| e.to_string())?;
    if recording_guard.is_some() {
        return Ok(());
    }

    let runtime = state.ai_runtime_context.lock().map_err(|e| e.to_string())?.clone();
    let input_device_name =
        resolve_input_device_name(runtime.selected_device).map_err(|e| e.to_string())?;
    let microphone_input = open_input_stream(&input_device_name)
        .map_err(|e| utils::format_and_log_error("Failed to open AI recording input stream", e))?;
    let sample_rate = microphone_input.sample_rate;
    let rx = microphone_input.rx;
    let stream = microphone_input.stream;
    stream
        .play()
        .map_err(|e| utils::format_and_log_error("Failed to start AI recording stream", e))?;

    let samples = Arc::new(Mutex::new(Vec::new()));
    let samples_ref = Arc::clone(&samples);
    let enable_denoise = runtime.microphone_config.enable_denoise;
    let worker_handle = std::thread::spawn(move || {
        let result = run_processed_audio_pipeline(
            rx,
            sample_rate,
            0.08,
            enable_denoise,
            None,
            |chunk| {
                let mut buffer = samples_ref.lock().map_err(|_| {
                    anyhow::anyhow!("AI recording sample buffer was poisoned")
                })?;
                buffer.extend_from_slice(chunk);
                Ok(())
            },
        );

        if let Err(error) = result {
            log::error!("AI recording pipeline failed: {}", error);
        }
    });

    *recording_guard = Some(AiRecordingCapture {
        samples,
        stream: Some(UnsafeStreamWrapper(stream)),
        worker_handle: Some(worker_handle),
    });

    emit_ai_recording_state(app_handle, true);
    Ok(())
}

fn stop_ai_recording_blocking(
    app_handle: &AppHandle,
    runtime: AiRuntimeContext,
) -> Result<ai::client::AiTranscriptionResult, String> {
    let state = app_handle.state::<AppState>();
    let capture = {
        let mut recording_guard = state.ai_recording.lock().map_err(|e| e.to_string())?;
        recording_guard.take()
    };

    let Some(mut capture) = capture else {
        return Err("AI recording is not active.".to_string());
    };

    capture.stream.take();
    if let Some(worker_handle) = capture.worker_handle.take() {
        let _ = worker_handle.join();
    }

    let samples = capture
        .samples
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    emit_ai_recording_state(app_handle, false);

    if samples.is_empty() {
        return Err("No microphone audio was captured.".to_string());
    }

    let transcript = {
        let mut sherpa_guard = state.ai_sherpa.lock().map_err(|e| e.to_string())?;
        if sherpa_guard.is_none() {
            sherpa_guard.replace(ai::sherpa::SherpaSpeechRuntime::new(app_handle)?);
        }

        let sherpa = sherpa_guard
            .as_mut()
            .ok_or_else(|| "Sherpa speech runtime is unavailable.".to_string())?;
        sherpa.transcribe(app_handle, &runtime.ai_config, &samples, 16_000)?
    };

    if transcript.is_empty() {
        return Err("ASR returned an empty transcript.".to_string());
    }

    let session_id = {
        let mut runtime_guard = state.ai_runtime_context.lock().map_err(|e| e.to_string())?;
        create_session_if_missing(app_handle, &mut runtime_guard)?
    };

    append_ai_session_event(
        &state,
        AiSessionEvent {
            id: format!("event-{}", now_ms()),
            kind: "user_transcript".to_string(),
            text: Some(transcript.clone()),
            created_at_ms: now_ms(),
        },
    )?;

    let result = ai::client::AiTranscriptionResult {
        session_id: session_id.clone(),
        transcript,
    };
    let _ = app_handle.emit(
        "ai-transcription-ready",
        AiTranscriptionEventPayload {
            session_id: result.session_id.clone(),
            transcript: result.transcript.clone(),
        },
    );

    let runtime_for_chat = state
        .ai_runtime_context
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    if let Err(error) =
        launch_ai_chat_stream(app_handle.clone(), session_id.clone(), runtime_for_chat)
    {
        emit_ai_error(app_handle, error);
    }

    Ok(result)
}

fn emit_ai_chat_state(app_handle: &AppHandle, streaming: bool) {
    let _ = app_handle.emit("ai-chat-state", AiChatStatePayload { streaming });
}

fn emit_ai_chat_delta(app_handle: &AppHandle, session_id: &str, delta: &str) {
    let _ = app_handle.emit(
        "ai-chat-delta",
        AiChatDeltaPayload {
            session_id: session_id.to_string(),
            delta: delta.to_string(),
        },
    );
}

fn emit_ai_chat_finished(app_handle: &AppHandle, session_id: &str, message: &str) {
    let _ = app_handle.emit(
        "ai-chat-finished",
        AiChatFinishedPayload {
            session_id: session_id.to_string(),
            message: message.to_string(),
        },
    );
}

fn launch_ai_chat_stream(
    app_handle: AppHandle,
    session_id: String,
    runtime: AiRuntimeContext,
) -> Result<(), String> {
    {
        let state = app_handle.state::<AppState>();
        let mut streaming = state.ai_streaming.lock().map_err(|e| e.to_string())?;
        if *streaming {
            return Err("AI chat is already streaming.".to_string());
        }
        *streaming = true;
    }

    emit_ai_chat_state(&app_handle, true);
    let app_handle_clone = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let result =
            run_ai_chat_pipeline(app_handle_clone.clone(), session_id.clone(), runtime).await;

        if let Err(error) = result {
            emit_ai_error(&app_handle_clone, error);
        } else if let Ok(message) = result {
            emit_ai_chat_finished(&app_handle_clone, &session_id, &message);
        }

        if let Ok(mut streaming) = app_handle_clone.state::<AppState>().ai_streaming.lock() {
            *streaming = false;
        }
        emit_ai_chat_state(&app_handle_clone, false);
    });

    Ok(())
}

fn ensure_ai_enabled(state: &State<'_, AppState>) -> Result<(), String> {
    let enabled = *state.ai_enabled.lock().map_err(|e| e.to_string())?;
    if enabled {
        Ok(())
    } else {
        Err("AI Agent is not started. Start it from the sidebar first.".to_string())
    }
}

fn emit_ai_tool_event(
    app_handle: &AppHandle,
    session_id: &str,
    phase: &str,
    name: &str,
    summary: &str,
) {
    let _ = app_handle.emit(
        "ai-tool-event",
        AiToolEventPayload {
            id: format!("tool-{}", now_ms()),
            session_id: session_id.to_string(),
            phase: phase.to_string(),
            name: name.to_string(),
            summary: summary.to_string(),
        },
    );
}

fn parse_local_key(name: &str) -> Option<LocalKey> {
    match name.trim().to_ascii_uppercase().as_str() {
        "UP" => Some(LocalKey::UP),
        "DOWN" => Some(LocalKey::DOWN),
        "LEFT" => Some(LocalKey::LEFT),
        "RIGHT" => Some(LocalKey::RIGHT),
        "OPEN" => Some(LocalKey::OPEN),
        "THROW" => Some(LocalKey::THROW),
        "RESEND" => Some(LocalKey::RESEND),
        "PTT" => Some(LocalKey::PTT),
        "OCC" => Some(LocalKey::OCC),
        _ => None,
    }
}

fn build_skill_tools(agent: &hellcall::config::AiAgentConfig) -> Vec<Value> {
    agent
        .skill_ids
        .iter()
        .filter_map(|skill_id| match skill_id.as_str() {
            "send_key_sequence" => Some(json!({
                "type": "function",
                "function": {
                    "name": "send_key_sequence",
                    "description": "Execute a local logical key sequence immediately.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "keys": {
                                "type": "array",
                                "items": {
                                    "type": "string",
                                    "enum": ["UP","DOWN","LEFT","RIGHT","OPEN","THROW","RESEND","PTT","OCC"]
                                }
                            }
                        },
                        "required": ["keys"]
                    }
                }
            })),
            "execute_stratagem" => Some(json!({
                "type": "function",
                "function": {
                    "name": "execute_stratagem",
                    "description": "Execute a known stratagem by id or name using OPEN + directions + THROW.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string"},
                            "name": {"type": "string"}
                        }
                    }
                }
            })),
            "list_stratagems" => Some(json!({
                "type": "function",
                "function": {
                    "name": "list_stratagems",
                    "description": "List known local stratagems and their direction sequences.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string"}
                        }
                    }
                }
            })),
            "get_key_mappings" => Some(json!({
                "type": "function",
                "function": {
                    "name": "get_key_mappings",
                    "description": "Return the current local logical-to-physical key mappings.",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            })),
            _ => None,
        })
        .collect()
}

fn build_chat_messages(
    app_handle: &AppHandle,
    agent: &hellcall::config::AiAgentConfig,
) -> Result<Vec<Value>, String> {
    let state = app_handle.state::<AppState>();
    let session = current_ai_session_record(&state)?;
    let mut messages = vec![json!({
        "role": "system",
        "content": agent.system_prompt
    })];

    for event in session.events {
        match event.kind.as_str() {
            "user_transcript" => {
                if let Some(text) = event.text.filter(|text| !text.trim().is_empty()) {
                    messages.push(json!({ "role": "user", "content": text }));
                }
            }
            "assistant_final" => {
                if let Some(text) = event.text.filter(|text| !text.trim().is_empty()) {
                    messages.push(json!({ "role": "assistant", "content": text }));
                }
            }
            "assistant_tool_calls" => {
                if let Some(text) = event.text {
                    let tool_calls: Vec<ai::client::ChatToolCall> =
                        serde_json::from_str(&text).map_err(|e| e.to_string())?;
                    messages.push(json!({
                        "role": "assistant",
                        "content": "",
                        "tool_calls": tool_calls,
                    }));
                }
            }
            "tool_result" => {
                if let Some(text) = event.text {
                    let payload: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": payload["tool_call_id"],
                        "name": payload["name"],
                        "content": payload["content"],
                    }));
                }
            }
            _ => {}
        }
    }

    Ok(messages)
}

fn current_ai_agent(runtime: &AiRuntimeContext) -> Result<hellcall::config::AiAgentConfig, String> {
    runtime
        .ai_config
        .agents
        .iter()
        .find(|agent| agent.id == runtime.ai_config.default_agent_id)
        .cloned()
        .or_else(|| runtime.ai_config.agents.first().cloned())
        .ok_or_else(|| "No AI agent is configured.".to_string())
}

fn selected_ai_provider(ai_config: &AiConfig) -> Result<AiLlmProviderConfig, String> {
    ai_config
        .llm
        .providers
        .iter()
        .find(|provider| provider.id == ai_config.llm.selected_provider_id)
        .cloned()
        .or_else(|| ai_config.llm.providers.first().cloned())
        .ok_or_else(|| "No LLM provider is configured.".to_string())
}

fn execute_tool_call(
    app_handle: &AppHandle,
    state: &State<'_, AppState>,
    session_id: &str,
    tool_call: &ai::client::ChatToolCall,
) -> Result<String, String> {
    let args: Value = serde_json::from_str(&tool_call.function.arguments).map_err(|e| {
        format!(
            "Failed to parse tool arguments for {}: {}",
            tool_call.function.name, e
        )
    })?;

    match tool_call.function.name.as_str() {
        "send_key_sequence" => {
            let keys = args["keys"]
                .as_array()
                .ok_or_else(|| "send_key_sequence requires a keys array.".to_string())?
                .iter()
                .filter_map(|value| value.as_str())
                .filter_map(parse_local_key)
                .collect::<Vec<_>>();

            if keys.is_empty() {
                return Err("No valid local keys were provided.".to_string());
            }

            if let Some(bridge) = state.ai_hotkey_bridge.lock().map_err(|e| e.to_string())?.as_ref()
            {
                emit_ai_tool_event(
                    app_handle,
                    session_id,
                    "call",
                    "send_key_sequence",
                    &format!("Executing {:?}", keys),
                );
                bridge.key_presser.enqueue(&keys, true);
                return Ok(format!("Executed key sequence: {:?}", keys));
            }

            Err("AI hotkey bridge is unavailable.".to_string())
        }
        "execute_stratagem" => {
            let catalog = stratagems::load_catalog(app_handle)?;
            let wanted_id = args.get("id").and_then(Value::as_str).map(|s| s.to_string());
            let wanted_name = args
                .get("name")
                .and_then(Value::as_str)
                .map(|s| s.to_ascii_lowercase());
            let item = catalog.items.into_iter().find(|item| {
                wanted_id.as_ref().is_some_and(|id| &item.id == id)
                    || wanted_name
                        .as_ref()
                        .is_some_and(|name| item.name.to_ascii_lowercase() == *name)
            });

            let Some(item) = item else {
                return Err("Requested stratagem was not found in the local catalog.".to_string());
            };

            let mut keys = vec![LocalKey::OPEN];
            keys.extend(item.command.iter().filter_map(|step| parse_local_key(step)));
            keys.push(LocalKey::THROW);

            if let Some(bridge) = state.ai_hotkey_bridge.lock().map_err(|e| e.to_string())?.as_ref()
            {
                emit_ai_tool_event(
                    app_handle,
                    session_id,
                    "call",
                    "execute_stratagem",
                    &format!("Executing '{}'", item.name),
                );
                bridge.key_presser.enqueue(&keys, true);
                return Ok(format!("Executed stratagem '{}' with sequence {:?}", item.name, keys));
            }

            Err("AI hotkey bridge is unavailable.".to_string())
        }
        "list_stratagems" => {
            emit_ai_tool_event(
                app_handle,
                session_id,
                "call",
                "list_stratagems",
                "Looking up local stratagem catalog",
            );
            let catalog = stratagems::load_catalog(app_handle)?;
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .map(|value| value.to_ascii_lowercase());
            let items = catalog
                .items
                .into_iter()
                .filter(|item| {
                    query
                        .as_ref()
                        .is_none_or(|query| item.name.to_ascii_lowercase().contains(query))
                })
                .take(20)
                .map(|item| {
                    json!({
                        "id": item.id,
                        "name": item.name,
                        "command": item.command,
                        "category": item.category,
                    })
                })
                .collect::<Vec<_>>();
            Ok(Value::Array(items).to_string())
        }
        "get_key_mappings" => {
            emit_ai_tool_event(
                app_handle,
                session_id,
                "call",
                "get_key_mappings",
                "Reading current logical key mappings",
            );
            let runtime = state.ai_runtime_context.lock().map_err(|e| e.to_string())?.clone();
            let mappings = runtime
                .key_map
                .into_iter()
                .map(|(local_key, input)| {
                    let name = match local_key {
                        LocalKey::UP => "UP",
                        LocalKey::DOWN => "DOWN",
                        LocalKey::LEFT => "LEFT",
                        LocalKey::RIGHT => "RIGHT",
                        LocalKey::OPEN => "OPEN",
                        LocalKey::THROW => "THROW",
                        LocalKey::RESEND => "RESEND",
                        LocalKey::PTT => "PTT",
                        LocalKey::OCC => "OCC",
                    };
                    json!({ "local_key": name, "input": format!("{:?}", input) })
                })
                .collect::<Vec<_>>();
            Ok(Value::Array(mappings).to_string())
        }
        _ => Err(format!("Unsupported tool '{}'.", tool_call.function.name)),
    }
}

fn append_tool_events(
    app_handle: &AppHandle,
    session_id: &str,
    tool_calls: &[ai::client::ChatToolCall],
    results: &[(String, String)],
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let _ = session_id;
    append_ai_session_event(
        &state,
        AiSessionEvent {
            id: format!("event-{}", now_ms()),
            kind: "assistant_tool_calls".to_string(),
            text: Some(serde_json::to_string(tool_calls).map_err(|e| e.to_string())?),
            created_at_ms: now_ms(),
        },
    )?;

    for (tool_call_id, result) in results {
        append_ai_session_event(
            &state,
            AiSessionEvent {
                id: format!("event-{}", now_ms()),
                kind: "tool_result".to_string(),
                text: Some(
                    json!({
                        "tool_call_id": tool_call_id,
                        "name": tool_calls
                            .iter()
                            .find(|call| &call.id == tool_call_id)
                            .map(|call| call.function.name.clone())
                            .unwrap_or_default(),
                        "content": result,
                    })
                    .to_string(),
                ),
                created_at_ms: now_ms(),
            },
        )?;
    }

    Ok(())
}

async fn run_ai_chat_pipeline(
    app_handle: AppHandle,
    session_id: String,
    runtime: AiRuntimeContext,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let agent = current_ai_agent(&runtime)?;
    let provider = selected_ai_provider(&runtime.ai_config)?;
    let mut messages = build_chat_messages(&app_handle, &agent)?;
    let tools = build_skill_tools(&agent);

    let mut iteration = 0usize;
    loop {
        iteration += 1;
        if iteration > 4 {
            return Err("AI tool loop exceeded the safety limit.".to_string());
        }

        let body = ai::client::build_chat_request_body(
            if agent.chat_model.trim().is_empty() {
                &provider.chat_model
            } else {
                &agent.chat_model
            },
            messages.clone(),
            tools.clone(),
            agent.temperature,
            agent.max_tokens,
            true,
        );

        let mut streamed_text = String::new();
        let stream_result = ai::client::stream_chat_completion(&provider, body, |delta| {
            streamed_text.push_str(delta);
            emit_ai_chat_delta(&app_handle, &session_id, delta);
            Ok(())
        })
        .await?;

        if !stream_result.tool_calls.is_empty() {
            let mut tool_results = Vec::new();
            for tool_call in &stream_result.tool_calls {
                let result = match execute_tool_call(&app_handle, &state, &session_id, tool_call) {
                    Ok(result) => result,
                    Err(error) => {
                        emit_ai_tool_event(
                            &app_handle,
                            &session_id,
                            "error",
                            &tool_call.function.name,
                            &error,
                        );
                        return Err(error);
                    }
                };
                emit_ai_tool_event(
                    &app_handle,
                    &session_id,
                    "result",
                    &tool_call.function.name,
                    &result,
                );
                tool_results.push((tool_call.id.clone(), result));
            }

            append_tool_events(&app_handle, &session_id, &stream_result.tool_calls, &tool_results)?;
            messages = build_chat_messages(&app_handle, &agent)?;
            continue;
        }

        let final_text = if stream_result.content.trim().is_empty() {
            streamed_text
        } else {
            stream_result.content
        };

        if !final_text.trim().is_empty() {
            append_ai_session_event(
                &state,
                AiSessionEvent {
                    id: format!("event-{}", now_ms()),
                    kind: "assistant_final".to_string(),
                    text: Some(final_text.clone()),
                    created_at_ms: now_ms(),
                },
            )?;

            if let Err(error) = synthesize_ai_tts_and_play(&app_handle, &runtime, &final_text).await
            {
                emit_ai_error(&app_handle, error);
            }
        }

        return Ok(final_text);
    }
}

async fn synthesize_ai_tts_and_play(
    app_handle: &AppHandle,
    runtime: &AiRuntimeContext,
    text: &str,
) -> Result<(), String> {
    if !runtime.ai_config.speech.tts.enabled || text.trim().is_empty() {
        return Ok(());
    }

    let state = app_handle.state::<AppState>();
    sync_ai_speaker_bridge(&state, runtime)?;

    let generated_audio = {
        let mut sherpa_guard = state.ai_sherpa.lock().map_err(|e| e.to_string())?;
        if sherpa_guard.is_none() {
            sherpa_guard.replace(ai::sherpa::SherpaSpeechRuntime::new(app_handle)?);
        }

        let sherpa = sherpa_guard
            .as_mut()
            .ok_or_else(|| "Sherpa speech runtime is unavailable.".to_string())?;
        sherpa.synthesize(app_handle, &runtime.ai_config, text)?
    };

    let speaker_guard = state.ai_speaker.lock().map_err(|e| e.to_string())?;
    let Some(bridge) = speaker_guard.as_ref() else {
        return Err("AI speaker is unavailable.".to_string());
    };

    bridge
        .speaker
        .play_pcm_f32(1, generated_audio.sample_rate as u32, generated_audio.samples)
        .map_err(|e| utils::format_and_log_error("Failed to play AI TTS audio", e))?;

    Ok(())
}

#[tauri::command]
fn get_available_vosk_models(
    app_handle: AppHandle,
) -> Result<Vec<vosk_model_manager::AvailableVoskModel>, String> {
    vosk_model_manager::get_available_models(&app_handle)
}

#[tauri::command]
fn get_available_sherpa_stt_models(
    app_handle: AppHandle,
) -> Result<Vec<sherpa_model_manager::AvailableSherpaModel>, String> {
    sherpa_model_manager::get_available_stt_models(&app_handle)
}

#[tauri::command]
fn get_available_sherpa_runtime(
    app_handle: AppHandle,
) -> Result<Vec<sherpa_runtime_manager::AvailableSherpaRuntime>, String> {
    sherpa_runtime_manager::get_available_runtime(&app_handle)
}

#[tauri::command]
fn get_available_sherpa_tts_models(
    app_handle: AppHandle,
) -> Result<Vec<sherpa_model_manager::AvailableSherpaModel>, String> {
    sherpa_model_manager::get_available_tts_models(&app_handle)
}

#[tauri::command]
fn get_available_vision_models(
    app_handle: AppHandle,
) -> Result<Vec<vision_model_manager::AvailableVisionModel>, String> {
    vision_model_manager::get_available_models(&app_handle)
}

#[tauri::command]
async fn download_vosk_model(
    app_handle: AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    vosk_model_manager::download_model(&app_handle, model_id, url).await
}

#[tauri::command]
async fn download_sherpa_stt_model(
    app_handle: AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    sherpa_model_manager::download_stt_model(&app_handle, model_id, url).await
}

#[tauri::command]
async fn download_sherpa_runtime(
    app_handle: AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    sherpa_runtime_manager::download_runtime(&app_handle, model_id, url).await
}

#[tauri::command]
async fn download_sherpa_tts_model(
    app_handle: AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    sherpa_model_manager::download_tts_model(&app_handle, model_id, url).await
}

#[tauri::command]
async fn download_vision_model(
    app_handle: AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    vision_model_manager::download_model(&app_handle, model_id, url).await
}

#[tauri::command]
fn start_engine(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    config: Config,
    device_name: Option<String>,
    selected_model_id: String,
    selected_vision_model_id: String,
) -> Result<String, String> {
    let mut engine_guard = state.engine.lock().map_err(|e| e.to_string())?;

    if let AppEngine::Running(_) = *engine_guard {
        return Ok("Already started".into());
    }

    let vosk_model_path =
        resolve_cached_vosk_runtime_model_path(&app_handle, &state, &selected_model_id)?;
    let vosk_model_path = utils::normalize_runtime_path(&vosk_model_path);

    let vision_model_path = vision_model_manager::resolve_selected_model_path_if_downloaded(
        &app_handle,
        &selected_vision_model_id,
    )?;
    let vision_model_path = vision_model_path.map(|path| utils::normalize_runtime_path(&path));

    let audio_path = resolve_audio_dir(&app_handle)?;
    let audio_path = utils::normalize_runtime_path(&audio_path);

    let state_taken = std::mem::replace(&mut *engine_guard, AppEngine::None);

    let engine = match state_taken {
        AppEngine::Stopped(handle) => handle
            .restart(
                config,
                &vosk_model_path,
                device_name.clone(),
                Some(audio_path.clone()),
                vision_model_path.clone(),
            )
            .map_err(|e| utils::format_and_log_error("Failed to restart engine", e))?,
        _ => HellcallEngine::start(
            config,
            &vosk_model_path,
            device_name,
            Some(audio_path),
            vision_model_path,
        )
        .map_err(|e| utils::format_and_log_error("Failed to start engine", e))?,
    };

    *engine_guard = AppEngine::Running(engine);
    Ok("Started".into())
}

#[tauri::command]
fn stop_engine(state: State<'_, AppState>) -> Result<String, String> {
    let mut engine_guard = state.engine.lock().map_err(|e| e.to_string())?;

    let state_taken = std::mem::replace(&mut *engine_guard, AppEngine::None);

    if let AppEngine::Running(engine) = state_taken {
        let handle = engine.stop();
        *engine_guard = AppEngine::Stopped(handle);
    } else {
        *engine_guard = state_taken;
    }

    Ok("Stopped".into())
}

#[tauri::command]
fn load_config(app: AppHandle) -> Result<Config, String> {
    let config_path = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("config.toml");
    load_config_from_path(&config_path)
}

#[tauri::command]
fn load_stratagems(app: AppHandle) -> Result<StratagemCatalog, String> {
    stratagems::load_catalog(&app)
}

#[tauri::command]
async fn refresh_stratagems(app: AppHandle) -> Result<StratagemCatalog, String> {
    stratagems::refresh_catalog(&app).await
}

#[tauri::command]
fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    new_config: Config,
) -> Result<bool, String> {
    let config_path = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("config.toml");
    save_config_to_path(&config_path, &new_config)?;

    let engine_guard = state.engine.lock().map_err(|e| e.to_string())?;
    if let AppEngine::Running(engine) = &*engine_guard {
        engine.update_speaker_config(new_config.speaker.clone().into());
    }

    Ok(true)
}

#[tauri::command]
fn get_audio_devices() -> Result<Vec<String>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            names.push(name);
        }
    }
    Ok(names)
}

#[tauri::command]
fn get_output_audio_devices() -> Result<Vec<String>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let devices = host.output_devices().map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            names.push(name);
        }
    }
    Ok(names)
}

#[tauri::command]
fn validate_virtual_mic_output_device(
    input_device_name: Option<String>,
    output_device_name: String,
    microphone_config: hellcall::config::MicrophoneConfig,
) -> Result<(), String> {
    validate_virtual_output_device_for_mix(
        input_device_name,
        &output_device_name,
        microphone_config.enable_denoise,
    )
    .map_err(|e| utils::format_and_log_error("Virtual output device is not usable", e))
}

#[tauri::command]
fn get_audio_files(app_handle: AppHandle) -> Result<Vec<String>, String> {
    fn collect_audio_files(
        current_dir: &std::path::Path,
        base_dir: &std::path::Path,
        files: &mut Vec<String>,
    ) -> Result<(), String> {
        let entries = fs::read_dir(current_dir)
            .map_err(|e| utils::format_and_log_error("Failed to read audio directory", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                utils::format_and_log_error("Failed to read audio directory entry", e)
            })?;
            let path = entry.path();
            log::debug!("Found audio path: {}", path.display());

            if path.is_dir() {
                collect_audio_files(&path, base_dir, files)?;
                continue;
            }

            let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
                continue;
            };

            let extension = extension.to_ascii_lowercase();
            if !["wav", "mp3", "ogg", "flac", "m4a"].contains(&extension.as_str()) {
                continue;
            }

            let relative_path = path
                .strip_prefix(base_dir)
                .map_err(|e| utils::format_and_log_error("Failed to resolve audio file path", e))?;
            files.push(relative_path.to_string_lossy().replace('\\', "/"));
        }

        Ok(())
    }

    let audio_path = resolve_audio_dir(&app_handle)?;

    let mut files = Vec::new();
    collect_audio_files(&audio_path, &audio_path, &mut files)?;
    files.sort();
    log::debug!("Collected audio files: {:?}", files);

    Ok(files)
}

#[tauri::command]
fn get_audio_directory(app_handle: AppHandle) -> Result<String, String> {
    let audio_path = resolve_audio_dir(&app_handle)?;
    Ok(utils::normalize_runtime_path(&audio_path))
}

#[tauri::command]
fn start_mic_test(
    device_name: Option<String>,
    microphone_config: hellcall::config::MicrophoneConfig,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri::Emitter;

    let stream = open_volume_meter_stream(
        device_name,
        microphone_config.enable_denoise,
        move |rms| {
        let _ = app_handle.emit("mic_volume", rms);
        },
    )
    .map_err(|e| utils::format_and_log_error("Failed to start mic test", e))?;

    let mut stream_guard = state.mic_test_stream.lock().map_err(|e| e.to_string())?;
    *stream_guard = Some(UnsafeStreamWrapper(stream));

    Ok(())
}

#[tauri::command]
fn stop_mic_test(state: State<'_, AppState>) -> Result<(), String> {
    let mut stream_guard = state.mic_test_stream.lock().map_err(|e| e.to_string())?;
    *stream_guard = None;
    Ok(())
}

#[tauri::command]
fn get_ai_session(
    app_handle: AppHandle,
    session_id: String,
) -> Result<AiSessionRecord, String> {
    let _ = session_id;
    let state = app_handle.state::<AppState>();
    current_ai_session_record(&state)
}

#[tauri::command]
fn delete_ai_session(app_handle: AppHandle, session_id: String) -> Result<bool, String> {
    let _ = session_id;
    let state = app_handle.state::<AppState>();
    reset_ai_session_record(&state)?;
    Ok(true)
}

#[tauri::command]
fn sync_ai_runtime_config(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    config: Config,
    device_name: Option<String>,
    session_id: Option<String>,
) -> Result<(), String> {
    let runtime_snapshot = {
        let mut runtime = state.ai_runtime_context.lock().map_err(|e| e.to_string())?;
        runtime.mode = config.mode.clone();
        runtime.selected_device = device_name.clone();
        runtime.microphone_config = config.microphone.clone();
        runtime.speaker_config = config.speaker.clone();
        runtime.ai_config = config.ai.clone();
        runtime.key_map = config.key_map.clone();
        runtime.key_presser_config = config.key_presser.clone();
        runtime.session_id = session_id;
        runtime.clone()
    };

    configure_ai_hotkey_bridge(&app_handle, &state, &runtime_snapshot)?;
    sync_ai_speaker_bridge(&state, &runtime_snapshot)
}

#[tauri::command]
fn start_ai_recording(app_handle: AppHandle) -> Result<(), String> {
    start_ai_recording_internal(&app_handle)
}

#[tauri::command]
fn stop_ai_recording(app_handle: AppHandle) -> Result<ai::client::AiTranscriptionResult, String> {
    let runtime = app_handle
        .state::<AppState>()
        .ai_runtime_context
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    stop_ai_recording_blocking(&app_handle, runtime)
}

#[tauri::command]
fn start_ai_agent(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let runtime = state.ai_runtime_context.lock().map_err(|e| e.to_string())?.clone();
    if runtime.mode != hellcall::config::AppMode::AiAgent {
        return Err("Switch to AI mode before starting AI Agent.".to_string());
    }

    configure_ai_hotkey_bridge(&app_handle, &state, &runtime)?;
    sync_ai_speaker_bridge(&state, &runtime)?;

    {
        let mut warmup_guard = state.ai_warmup_in_progress.lock().map_err(|e| e.to_string())?;
        if *warmup_guard {
            return Err("AI Agent is already warming up.".to_string());
        }
        *warmup_guard = true;
    }

    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let warmup_result = (|| -> Result<(), String> {
            emit_ai_warmup_state(&app_handle_clone, "LOADING_RUNTIME");
            let state = app_handle_clone.state::<AppState>();
            let mut sherpa_guard = state.ai_sherpa.lock().map_err(|e| e.to_string())?;
            if sherpa_guard.is_none() {
                sherpa_guard.replace(ai::sherpa::SherpaSpeechRuntime::new(&app_handle_clone)?);
            }
            if let Some(sherpa) = sherpa_guard.as_mut() {
                emit_ai_warmup_state(&app_handle_clone, "LOADING_STT");
                sherpa.prewarm_recognizer(&app_handle_clone, &runtime.ai_config)?;
                if runtime.ai_config.speech.tts.enabled {
                    emit_ai_warmup_state(&app_handle_clone, "LOADING_TTS");
                    sherpa.prewarm_tts(&app_handle_clone, &runtime.ai_config)?;
                }
            }
            Ok(())
        })();

        let state = app_handle_clone.state::<AppState>();
        if let Ok(mut warmup_guard) = state.ai_warmup_in_progress.lock() {
            *warmup_guard = false;
        }

        match warmup_result {
            Ok(()) => {
                if let Ok(mut enabled) = state.ai_enabled.lock() {
                    *enabled = true;
                }
                emit_ai_warmup_state(&app_handle_clone, "READY");
            }
            Err(error) => {
                if let Ok(mut sherpa_guard) = state.ai_sherpa.lock() {
                    if let Some(runtime) = sherpa_guard.as_mut() {
                        runtime.invalidate_models();
                    }
                    *sherpa_guard = None;
                }
                if let Ok(mut enabled) = state.ai_enabled.lock() {
                    *enabled = false;
                }
                emit_ai_warmup_state(&app_handle_clone, "OFFLINE");
                emit_ai_agent_error(&app_handle_clone, error);
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn stop_ai_agent(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut enabled = state.ai_enabled.lock().map_err(|e| e.to_string())?;
        *enabled = false;
    }

    {
        let mut recording_guard = state.ai_recording.lock().map_err(|e| e.to_string())?;
        *recording_guard = None;
    }

    {
        let mut sherpa_guard = state.ai_sherpa.lock().map_err(|e| e.to_string())?;
        if let Some(runtime) = sherpa_guard.as_mut() {
            runtime.invalidate_models();
        }
        *sherpa_guard = None;
    }

    emit_ai_warmup_state(&app_handle, "OFFLINE");
    Ok(())
}

#[tauri::command]
fn start_ai_chat_stream(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let runtime = state.ai_runtime_context.lock().map_err(|e| e.to_string())?.clone();
    launch_ai_chat_stream(app_handle, session_id, runtime)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .targets(vec![
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Webview),
                ])
                .format(|out, message, _| out.finish(*message))
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            engine: Mutex::new(AppEngine::None),
            mic_test_stream: Mutex::new(None),
            cached_vosk_runtime_model_paths: Mutex::new(HashMap::new()),
            ai_session: Mutex::new(default_ai_session_record()),
            ai_runtime_context: Mutex::new(AiRuntimeContext::default()),
            ai_recording: Mutex::new(None),
            ai_hotkey_bridge: Mutex::new(None),
            ai_speaker: Mutex::new(None),
            ai_sherpa: Mutex::new(None),
            ai_enabled: Mutex::new(false),
            ai_warmup_in_progress: Mutex::new(false),
            ai_streaming: Mutex::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_available_vosk_models,
            get_available_sherpa_runtime,
            get_available_sherpa_stt_models,
            get_available_sherpa_tts_models,
            get_available_vision_models,
            download_vosk_model,
            download_sherpa_runtime,
            download_sherpa_stt_model,
            download_sherpa_tts_model,
            download_vision_model,
            get_audio_devices,
            get_output_audio_devices,
            validate_virtual_mic_output_device,
            get_audio_files,
            get_audio_directory,
            start_mic_test,
            stop_mic_test,
            load_config,
            load_stratagems,
            refresh_stratagems,
            save_config,
            get_ai_session,
            delete_ai_session,
            sync_ai_runtime_config,
            start_ai_agent,
            stop_ai_agent,
            start_ai_recording,
            stop_ai_recording,
            start_ai_chat_stream,
            start_engine,
            stop_engine
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
