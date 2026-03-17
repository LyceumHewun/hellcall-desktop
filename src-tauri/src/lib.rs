use hellcall::Config;
use std::fs;
use tauri::{AppHandle, Manager};

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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![load_config, save_config])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
