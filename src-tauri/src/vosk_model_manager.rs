use crate::utils;
use futures_util::StreamExt;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::{fs, io};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use zip::ZipArchive;

const VOSK_MODELS_DIR: &str = "vosk_models";

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

#[derive(Clone, Serialize)]
struct ModelDownloadProgressPayload {
    id: String,
    progress: u8,
    status: String,
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
    let models_dir = resolve_models_dir(app_handle)?;
    let model_dir = models_dir.join(&model_id);
    let zip_path = models_dir.join(format!("{}.zip", model_id));
    let model_dir_existed = model_dir.exists();

    let result = async {
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

        if model_dir.is_dir() {
            emit_model_download_progress(app_handle, &model_id, 100, "Complete");
            return Ok(true);
        }

        if zip_path.exists() {
            fs::remove_file(&zip_path).map_err(|e| {
                utils::format_and_log_error("Failed to clear previous Vosk archive", e)
            })?;
        }

        emit_model_download_progress(app_handle, &model_id, 0, "Downloading...");

        let client = reqwest::Client::new();
        let response = client
            .get(download_url)
            .send()
            .await
            .map_err(|e| utils::format_and_log_error("Failed to download Vosk model", e))?
            .error_for_status()
            .map_err(|e| utils::format_and_log_error("Vosk model download failed", e))?;

        let total_size = response.content_length();
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&zip_path)
            .await
            .map_err(|e| utils::format_and_log_error("Failed to create Vosk archive file", e))?;

        let mut downloaded_bytes = 0u64;
        let mut last_progress = 0u8;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| utils::format_and_log_error("Failed while streaming Vosk model", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| utils::format_and_log_error("Failed to write Vosk archive", e))?;

            downloaded_bytes += chunk.len() as u64;
            if let Some(total_size) = total_size.filter(|size| *size > 0) {
                let progress = ((downloaded_bytes.saturating_mul(100) / total_size).min(100)) as u8;
                if progress != last_progress {
                    last_progress = progress;
                    emit_model_download_progress(app_handle, &model_id, progress, "Downloading...");
                }
            }
        }

        file.flush()
            .await
            .map_err(|e| utils::format_and_log_error("Failed to flush Vosk archive to disk", e))?;
        drop(file);

        emit_model_download_progress(app_handle, &model_id, 100, "Extracting...");

        let zip_path_for_extract = zip_path.clone();
        let models_dir_for_extract = models_dir.clone();
        tokio::task::spawn_blocking(move || {
            extract_zip_archive(&zip_path_for_extract, &models_dir_for_extract)
        })
        .await
        .map_err(|e| utils::format_and_log_error("Failed to join Vosk extraction task", e))??;

        if !model_dir.is_dir() {
            return Err(format!(
                "Vosk model '{}' was extracted, but the expected folder was not created.",
                model_id
            ));
        }

        fs::remove_file(&zip_path)
            .map_err(|e| utils::format_and_log_error("Failed to delete Vosk archive", e))?;

        emit_model_download_progress(app_handle, &model_id, 100, "Complete");
        Ok(true)
    }
    .await;

    if let Err(error) = &result {
        let failed_status = format!("Failed: {}", error);
        emit_model_download_progress(app_handle, &model_id, 0, &failed_status);

        let _ = fs::remove_file(&zip_path);
        if !model_dir_existed && model_dir.starts_with(&models_dir) {
            let _ = fs::remove_dir_all(&model_dir);
        }
    }

    result
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

    let models_dir = app_local_data_dir.join(VOSK_MODELS_DIR);
    fs::create_dir_all(&models_dir)
        .map_err(|e| utils::format_and_log_error("Failed to create Vosk models directory", e))?;

    Ok(models_dir)
}

fn emit_model_download_progress(
    app_handle: &AppHandle,
    model_id: &str,
    progress: u8,
    status: &str,
) {
    let _ = app_handle.emit(
        "model-download-progress",
        ModelDownloadProgressPayload {
            id: model_id.to_string(),
            progress,
            status: status.to_string(),
        },
    );
}

fn extract_zip_archive(zip_path: &Path, output_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| utils::format_and_log_error("Failed to open downloaded Vosk archive", e))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| utils::format_and_log_error("Failed to read Vosk archive", e))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| utils::format_and_log_error("Failed to read Vosk archive entry", e))?;
        let relative_path = entry.enclosed_name().ok_or_else(|| {
            utils::format_and_log_error(
                "Refusing to extract Vosk archive entry with invalid path",
                entry.name(),
            )
        })?;
        let destination = output_dir.join(relative_path);

        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|e| {
                utils::format_and_log_error("Failed to create extracted Vosk directory", e)
            })?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                utils::format_and_log_error("Failed to create extracted Vosk parent directory", e)
            })?;
        }

        let mut output_file = fs::File::create(&destination)
            .map_err(|e| utils::format_and_log_error("Failed to create extracted Vosk file", e))?;
        io::copy(&mut entry, &mut output_file)
            .map_err(|e| utils::format_and_log_error("Failed to extract Vosk file", e))?;
    }

    Ok(())
}
