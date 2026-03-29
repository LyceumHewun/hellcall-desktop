mod asset_manager;
mod hellcall;
mod stratagems;
mod utils;

use asset_manager::{vision_model_manager, vosk_model_manager};
use hellcall::{load_config_from_path, save_config_to_path, Config, EngineHandle, HellcallEngine};
use hellcall::core::microphone::open_volume_meter_stream;
use std::collections::HashMap;
use stratagems::StratagemCatalog;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::{Target, TargetKind};

enum AppEngine {
    None,
    Running(HellcallEngine),
    Stopped(EngineHandle),
}

struct UnsafeStreamWrapper(cpal::Stream);
unsafe impl Send for UnsafeStreamWrapper {}
unsafe impl Sync for UnsafeStreamWrapper {}

struct AppState {
    engine: Mutex<AppEngine>,
    mic_test_stream: Mutex<Option<UnsafeStreamWrapper>>,
    cached_vosk_runtime_model_paths: Mutex<HashMap<String, PathBuf>>,
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

#[tauri::command]
fn get_available_vosk_models(
    app_handle: AppHandle,
) -> Result<Vec<vosk_model_manager::AvailableVoskModel>, String> {
    vosk_model_manager::get_available_models(&app_handle)
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
        })
        .invoke_handler(tauri::generate_handler![
            get_available_vosk_models,
            get_available_vision_models,
            download_vosk_model,
            download_vision_model,
            get_audio_devices,
            get_output_audio_devices,
            get_audio_files,
            get_audio_directory,
            start_mic_test,
            stop_mic_test,
            load_config,
            load_stratagems,
            refresh_stratagems,
            save_config,
            start_engine,
            stop_engine
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
