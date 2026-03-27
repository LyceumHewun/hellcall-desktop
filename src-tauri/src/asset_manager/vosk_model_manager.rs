use super::asset_downloader::{self, AssetType};
use crate::utils;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const LEGACY_VOSK_MODELS_DIR: &str = "vosk_models";
const MODEL_DOWNLOAD_EVENT: &str = "model-download-progress";

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
