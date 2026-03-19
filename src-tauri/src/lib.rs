use hellcall::{Config, EngineHandle, HellcallEngine};
use std::fs;
use std::sync::Mutex;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::{Target, TargetKind};

enum AppEngine {
    None,
    Running(HellcallEngine),
    Stopped(EngineHandle),
}

struct AppState {
    engine: Mutex<AppEngine>,
}

#[tauri::command]
fn start_engine(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    config: Config,
) -> Result<String, String> {
    let mut engine_guard = state.engine.lock().map_err(|e| e.to_string())?;

    if let AppEngine::Running(_) = *engine_guard {
        return Ok("Already started".into());
    }

    let model_path = app_handle
        .path()
        .resolve("model/", BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve model path: {}", e))?
        .to_string_lossy()
        .replace("\\\\?\\", "") // on Windows can return paths with \\?\ prefix, which causes issues with some libraries
        .to_string();

    let audio_path = app_handle
        .path()
        .resolve("audio/", BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve audio path: {}", e))?
        .to_string_lossy()
        .replace("\\\\?\\", "")
        .to_string();

    let state_taken = std::mem::replace(&mut *engine_guard, AppEngine::None);

    let engine = match state_taken {
        AppEngine::Stopped(handle) => handle
            .restart(config, &model_path, None, Some(audio_path))
            .map_err(|e| e.to_string())?,
        _ => HellcallEngine::start(config, &model_path, None, Some(audio_path))
            .map_err(|e| e.to_string())?,
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
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            start_engine,
            stop_engine
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
