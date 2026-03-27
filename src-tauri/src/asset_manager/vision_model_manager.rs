use super::asset_downloader::{self, AssetType};
use crate::utils;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const VISION_DOWNLOAD_EVENT: &str = "vision-download-progress";
const VISION_MODEL_ID: &str = "helldivers2-console-command-arrow-yolo-v8n";
const VISION_MODEL_URL: &str =
    "https://huggingface.co/hewunlyceum/helldivers2_console_command_arrow/resolve/main/helldivers2_console_command_arrow.onnx";
const VISION_MODEL_FILENAME: &str = "helldivers2_console_command_arrow.onnx";

#[derive(Serialize)]
pub struct AvailableVisionModel {
    id: String,
    url: String,
    is_downloaded: bool,
}

pub fn get_available_models(app_handle: &AppHandle) -> Result<Vec<AvailableVisionModel>, String> {
    let model_path = resolve_model_file_path(app_handle)?;

    Ok(vec![AvailableVisionModel {
        id: VISION_MODEL_ID.to_string(),
        url: VISION_MODEL_URL.to_string(),
        is_downloaded: model_path.is_file(),
    }])
}

pub async fn download_model(
    app_handle: &AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    if model_id != VISION_MODEL_ID {
        return Err(format!("Unknown vision model '{}'.", model_id));
    }

    let download_url = if url.trim().is_empty() {
        VISION_MODEL_URL
    } else if url == VISION_MODEL_URL {
        VISION_MODEL_URL
    } else {
        return Err(format!(
            "Download URL for '{}' does not match the official model list.",
            model_id
        ));
    };

    let model_path = resolve_model_file_path(app_handle)?;
    asset_downloader::download_asset(
        app_handle,
        &model_id,
        download_url,
        &model_path,
        AssetType::RawFile,
        VISION_DOWNLOAD_EVENT,
    )
    .await
}

pub fn resolve_selected_model_path_if_downloaded(
    app_handle: &AppHandle,
    model_id: &str,
) -> Result<Option<PathBuf>, String> {
    let selected_model_id = model_id.trim();
    if selected_model_id.is_empty() {
        return Ok(None);
    }
    if selected_model_id != VISION_MODEL_ID {
        return Err(format!("Unknown vision model '{}'.", selected_model_id));
    }

    let model_path = resolve_model_file_path(app_handle)?;
    if model_path.is_file() {
        Ok(Some(model_path))
    } else {
        Ok(None)
    }
}

fn resolve_model_file_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_local_data_dir = app_handle.path().app_local_data_dir().map_err(|e| {
        utils::format_and_log_error("Failed to resolve app local data directory", e)
    })?;

    fs::create_dir_all(&app_local_data_dir)
        .map_err(|e| utils::format_and_log_error("Failed to create app local data directory", e))?;

    let vision_dir = app_local_data_dir.join("models").join("vision");
    fs::create_dir_all(&vision_dir)
        .map_err(|e| utils::format_and_log_error("Failed to create vision models directory", e))?;

    Ok(vision_dir.join(VISION_MODEL_FILENAME))
}
