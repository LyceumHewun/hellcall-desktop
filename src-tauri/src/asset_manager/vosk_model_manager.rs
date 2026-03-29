use super::asset_downloader::{self, AssetType};
use crate::utils;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const LEGACY_VOSK_MODELS_DIR: &str = "vosk_models";
const MODEL_DOWNLOAD_EVENT: &str = "model-download-progress";
#[cfg(target_os = "windows")]
const PROGRAM_DATA_VOSK_LINKS_DIR: &str = r"C:\ProgramData\HellcallDesktop\vosk-model-links";

#[derive(Clone, Copy)]
struct AvailableVoskModelDefinition {
    id: &'static str,
    url: &'static str,
}

const AVAILABLE_VOSK_MODELS: &[AvailableVoskModelDefinition] = &[
    AvailableVoskModelDefinition {
        id: "vosk-model-small-cn-0.22",
        url: "https://alphacephei.com/vosk/models/vosk-model-small-cn-0.22.zip",
    },
    AvailableVoskModelDefinition {
        id: "vosk-model-small-en-us-0.15",
        url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip",
    },
];

#[derive(Serialize)]
pub struct AvailableVoskModel {
    id: String,
    url: String,
    is_downloaded: bool,
}

pub fn get_available_models(app_handle: &AppHandle) -> Result<Vec<AvailableVoskModel>, String> {
    let models_dir = resolve_models_dir(app_handle)?;

    Ok(AVAILABLE_VOSK_MODELS
        .iter()
        .map(|model| AvailableVoskModel {
            id: model.id.to_string(),
            url: model.url.to_string(),
            is_downloaded: models_dir.join(model.id).is_dir(),
        })
        .collect())
}

pub async fn download_model(
    app_handle: &AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    let official_model = AVAILABLE_VOSK_MODELS
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| format!("Unknown Vosk model '{}'.", model_id))?;
    let download_url = if url.trim().is_empty() {
        official_model.url
    } else if url == official_model.url {
        official_model.url
    } else {
        return Err(format!(
            "Download URL for '{}' does not match the official model list.",
            model_id
        ));
    };

    let model_path = resolve_models_dir(app_handle)?.join(&model_id);
    asset_downloader::download_asset(
        app_handle,
        &model_id,
        download_url,
        &model_path,
        AssetType::ZipArchive,
        MODEL_DOWNLOAD_EVENT,
    )
    .await
}

pub fn resolve_selected_model_path(
    app_handle: &AppHandle,
    model_id: &str,
) -> Result<PathBuf, String> {
    let selected_model_id = model_id.trim();
    if selected_model_id.is_empty() {
        return Err("No Vosk model selected. Please choose a model and download it first.".into());
    }

    let model_path = resolve_models_dir(app_handle)?.join(selected_model_id);
    if !model_path.is_dir() {
        return Err(format!(
            "Vosk model '{}' is not downloaded. Please download it first.",
            selected_model_id
        ));
    }

    Ok(model_path)
}

pub fn resolve_runtime_model_path(
    app_handle: &AppHandle,
    model_id: &str,
) -> Result<PathBuf, String> {
    let model_path = resolve_selected_model_path(app_handle, model_id)?;
    ensure_ascii_safe_model_path(&model_path, model_id)
}

fn resolve_models_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_local_data_dir = app_handle.path().app_local_data_dir().map_err(|e| {
        utils::format_and_log_error("Failed to resolve app local data directory", e)
    })?;

    fs::create_dir_all(&app_local_data_dir)
        .map_err(|e| utils::format_and_log_error("Failed to create app local data directory", e))?;

    let models_dir = app_local_data_dir.join("models").join("vosk");
    fs::create_dir_all(&models_dir)
        .map_err(|e| utils::format_and_log_error("Failed to create Vosk models directory", e))?;

    migrate_legacy_models_dir(&app_local_data_dir, &models_dir)?;

    Ok(models_dir)
}

fn migrate_legacy_models_dir(
    app_local_data_dir: &PathBuf,
    models_dir: &PathBuf,
) -> Result<(), String> {
    let legacy_dir = app_local_data_dir.join(LEGACY_VOSK_MODELS_DIR);
    if !legacy_dir.is_dir() {
        return Ok(());
    }

    let entries = fs::read_dir(&legacy_dir).map_err(|e| {
        utils::format_and_log_error("Failed to read legacy Vosk models directory", e)
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            utils::format_and_log_error("Failed to inspect legacy Vosk model entry", e)
        })?;
        let from_path = entry.path();
        let to_path = models_dir.join(entry.file_name());

        if to_path.exists() {
            continue;
        }

        fs::rename(&from_path, &to_path).map_err(|e| {
            utils::format_and_log_error("Failed to migrate legacy Vosk model into models/vosk", e)
        })?;
    }

    let is_empty = fs::read_dir(&legacy_dir)
        .map_err(|e| {
            utils::format_and_log_error("Failed to re-check legacy Vosk models directory", e)
        })?
        .next()
        .is_none();

    if is_empty {
        let _ = fs::remove_dir(&legacy_dir);
    }

    Ok(())
}

fn ensure_ascii_safe_model_path(model_path: &Path, model_id: &str) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        if !path_contains_non_ascii(model_path) {
            return Ok(model_path.to_path_buf());
        }

        let link_path = build_program_data_link_path(model_id);
        ensure_windows_model_junction(&link_path, model_path)?;
        log::warn!(
            "Vosk model path contains non-ASCII characters, using ProgramData junction '{}' -> '{}'",
            link_path.display(),
            model_path.display()
        );
        return Ok(link_path);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = model_id;
        Ok(model_path.to_path_buf())
    }
}

fn path_contains_non_ascii(path: &Path) -> bool {
    path.to_string_lossy().chars().any(|ch| !ch.is_ascii())
}

#[cfg(target_os = "windows")]
fn build_program_data_link_path(model_id: &str) -> PathBuf {
    PathBuf::from(PROGRAM_DATA_VOSK_LINKS_DIR).join(sanitize_model_id_for_link_name(model_id))
}

#[cfg(target_os = "windows")]
fn sanitize_model_id_for_link_name(model_id: &str) -> String {
    let sanitized = model_id
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        "default-vosk-model".to_string()
    } else {
        sanitized
    }
}

#[cfg(target_os = "windows")]
fn ensure_windows_model_junction(link_path: &Path, target_path: &Path) -> Result<(), String> {
    let link_parent = link_path.parent().ok_or_else(|| {
        format!(
            "Failed to resolve ProgramData parent directory for '{}'.",
            link_path.display()
        )
    })?;
    fs::create_dir_all(link_parent).map_err(|e| {
        utils::format_and_log_error("Failed to create ProgramData Vosk link directory", e)
    })?;

    if fs::symlink_metadata(link_path).is_ok() {
        if fs::canonicalize(link_path).ok() == fs::canonicalize(target_path).ok() {
            return Ok(());
        }

        remove_existing_junction_path(link_path)?;
    }

    create_windows_junction(link_path, target_path)
}

#[cfg(target_os = "windows")]
fn remove_existing_junction_path(link_path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(link_path).map_err(|e| {
        utils::format_and_log_error("Failed to inspect stale ProgramData Vosk junction", e)
    })?;

    if metadata.file_type().is_dir() {
        fs::remove_dir(link_path).map_err(|e| {
            utils::format_and_log_error("Failed to remove stale ProgramData Vosk junction", e)
        })?;
        return Ok(());
    }

    fs::remove_file(link_path).map_err(|e| {
        utils::format_and_log_error("Failed to remove stale ProgramData Vosk link file", e)
    })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn create_windows_junction(link_path: &Path, target_path: &Path) -> Result<(), String> {
    let command = format!(
        "mklink /J \"{}\" \"{}\"",
        link_path.display(),
        target_path.display()
    );

    let output = std::process::Command::new("cmd")
        .args(["/C", &command])
        .output()
        .map_err(|e| utils::format_and_log_error("Failed to execute mklink for Vosk model", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "mklink returned a non-zero exit code without output".to_string()
    };

    Err(utils::format_and_log_error(
        "Failed to create ProgramData junction for Vosk model",
        details,
    ))
}
