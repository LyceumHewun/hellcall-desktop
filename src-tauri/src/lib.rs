use hellcall::{Config, EngineHandle, HellcallEngine};
use std::fs;
use std::sync::Mutex;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::{Target, TargetKind};

mod utils;

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
}

#[tauri::command]
fn start_engine(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    config: Config,
    device_name: Option<String>,
) -> Result<String, String> {
    let mut engine_guard = state.engine.lock().map_err(|e| e.to_string())?;

    if let AppEngine::Running(_) = *engine_guard {
        return Ok("Already started".into());
    }

    let model_path = app_handle
        .path()
        .resolve("model/", BaseDirectory::Resource)
        .map_err(|e| utils::format_and_log_error("Failed to resolve model path", e))?
        .to_string_lossy()
        .replace("\\\\?\\", "") // on Windows can return paths with \\?\ prefix, which causes issues with some libraries
        .to_string();

    let audio_path = app_handle
        .path()
        .resolve("audio/", BaseDirectory::Resource)
        .map_err(|e| utils::format_and_log_error("Failed to resolve audio path", e))?
        .to_string_lossy()
        .replace("\\\\?\\", "")
        .to_string();

    let state_taken = std::mem::replace(&mut *engine_guard, AppEngine::None);

    let engine = match state_taken {
        AppEngine::Stopped(handle) => handle
            .restart(config, &model_path, device_name.clone(), Some(audio_path))
            .map_err(|e| utils::format_and_log_error("Failed to restart engine", e))?,
        _ => HellcallEngine::start(config, &model_path, device_name, Some(audio_path))
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
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
        }
        let default_config = Config::default();
        let toml_string = toml::to_string(&default_config).map_err(|e| e.to_string())?;
        fs::write(&config_path, toml_string).map_err(|e| e.to_string())?;
        return Ok(default_config);
    }

    let toml_string = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    let config: Config = toml::from_str(&toml_string).map_err(|e| e.to_string())?;
    Ok(config)
}

#[tauri::command]
fn save_config(app: AppHandle, new_config: Config) -> Result<bool, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let config_path = config_dir.join("config.toml");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }

    let toml_string = toml::to_string(&new_config).map_err(|e| e.to_string())?;
    fs::write(&config_path, toml_string).map_err(|e| e.to_string())?;
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
fn start_mic_test(
    device_name: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use tauri::Emitter;

    let host = cpal::default_host();
    let device = if let Some(name) = device_name {
        host.input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().unwrap_or_default() == name)
            .ok_or_else(|| "Device not found".to_string())?
    } else {
        host.default_input_device()
            .ok_or_else(|| "No default device".to_string())?
    };

    let config = device.default_input_config().map_err(|e| e.to_string())?;

    let sample_format = config.sample_format();
    let config: cpal::StreamConfig = config.into();

    let last_emit = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));

    let err_fn = |err| log::error!("an error occurred on stream: {}", err);

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let last_emit = last_emit.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut le = last_emit.lock().unwrap();
                    if le.elapsed() >= std::time::Duration::from_millis(60) {
                        let sum_squares: f32 = data.iter().map(|&s| s * s).sum();
                        let rms = (sum_squares / data.len() as f32).sqrt();
                        let _ = app_handle.emit("mic_volume", rms);
                        *le = std::time::Instant::now();
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let last_emit = last_emit.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut le = last_emit.lock().unwrap();
                    if le.elapsed() >= std::time::Duration::from_millis(60) {
                        let sum_squares: f32 = data
                            .iter()
                            .map(|&s| {
                                let f = s as f32 / i16::MAX as f32;
                                f * f
                            })
                            .sum();
                        let rms = (sum_squares / data.len() as f32).sqrt();
                        let _ = app_handle.emit("mic_volume", rms);
                        *le = std::time::Instant::now();
                    }
                },
                err_fn,
                None,
            )
        }
        _ => return Err("Unsupported sample format".to_string()),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

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
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            engine: Mutex::new(AppEngine::None),
            mic_test_stream: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_audio_devices,
            start_mic_test,
            stop_mic_test,
            load_config,
            save_config,
            start_engine,
            stop_engine
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
