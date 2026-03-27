use crate::utils;
use futures_util::StreamExt;
use serde::Serialize;
use std::path::Path;
use std::{fs, io};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use zip::ZipArchive;

#[derive(Clone, Copy)]
pub enum AssetType {
    ZipArchive,
    RawFile,
}

#[derive(Clone, Serialize)]
struct AssetDownloadProgressPayload {
    id: String,
    progress: u8,
    status: String,
}

pub async fn download_asset(
    app_handle: &AppHandle,
    asset_id: &str,
    url: &str,
    target_path: &Path,
    asset_type: AssetType,
    event_name: &str,
) -> Result<bool, String> {
    let target_existed = target_path.exists();
    let download_path = match asset_type {
        AssetType::ZipArchive => {
            let parent = target_path.parent().ok_or_else(|| {
                "Zip archive target path must have a parent directory.".to_string()
            })?;
            parent.join(format!("{}.zip", asset_id))
        }
        AssetType::RawFile => target_path.to_path_buf(),
    };

    let result = async {
        ensure_parent_dir(target_path)?;

        if asset_exists(target_path, asset_type) {
            emit_download_progress(app_handle, event_name, asset_id, 100, "Complete");
            return Ok(true);
        }

        if matches!(asset_type, AssetType::ZipArchive) && download_path.exists() {
            fs::remove_file(&download_path).map_err(|e| {
                utils::format_and_log_error("Failed to clear previous downloaded archive", e)
            })?;
        }

        emit_download_progress(app_handle, event_name, asset_id, 0, "Downloading...");

        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| utils::format_and_log_error("Failed to download asset", e))?
            .error_for_status()
            .map_err(|e| utils::format_and_log_error("Asset download failed", e))?;

        let total_size = response.content_length();
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&download_path)
            .await
            .map_err(|e| utils::format_and_log_error("Failed to create asset file", e))?;

        let mut downloaded_bytes = 0u64;
        let mut last_progress = 0u8;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| utils::format_and_log_error("Failed while streaming asset", e))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| utils::format_and_log_error("Failed to write asset", e))?;

            downloaded_bytes += chunk.len() as u64;
            if let Some(total_size) = total_size.filter(|size| *size > 0) {
                let progress = ((downloaded_bytes.saturating_mul(100) / total_size).min(100)) as u8;
                if progress != last_progress {
                    last_progress = progress;
                    emit_download_progress(
                        app_handle,
                        event_name,
                        asset_id,
                        progress,
                        "Downloading...",
                    );
                }
            }
        }

        file.flush()
            .await
            .map_err(|e| utils::format_and_log_error("Failed to flush asset to disk", e))?;
        drop(file);

        match asset_type {
            AssetType::RawFile => {
                emit_download_progress(app_handle, event_name, asset_id, 100, "Complete");
            }
            AssetType::ZipArchive => {
                let extract_root = target_path.parent().ok_or_else(|| {
                    "Zip archive target path must have a parent directory.".to_string()
                })?;
                emit_download_progress(app_handle, event_name, asset_id, 100, "Extracting...");

                let zip_path_for_extract = download_path.clone();
                let extract_root_for_extract = extract_root.to_path_buf();
                tokio::task::spawn_blocking(move || {
                    extract_zip_archive(&zip_path_for_extract, &extract_root_for_extract)
                })
                .await
                .map_err(|e| utils::format_and_log_error("Failed to join extraction task", e))??;

                if !target_path.is_dir() {
                    return Err(format!(
                        "Asset '{}' was extracted, but the expected folder was not created.",
                        asset_id
                    ));
                }

                fs::remove_file(&download_path)
                    .map_err(|e| utils::format_and_log_error("Failed to delete archive", e))?;
                emit_download_progress(app_handle, event_name, asset_id, 100, "Complete");
            }
        }

        Ok(true)
    }
    .await;

    if let Err(error) = &result {
        emit_download_progress(
            app_handle,
            event_name,
            asset_id,
            0,
            &format!("Failed: {}", error),
        );

        let _ = fs::remove_file(&download_path);
        if !target_existed {
            match asset_type {
                AssetType::ZipArchive => {
                    let _ = fs::remove_dir_all(target_path);
                }
                AssetType::RawFile => {
                    let _ = fs::remove_file(target_path);
                }
            }
        }
    }

    result
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Asset path must have a parent directory.".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|e| utils::format_and_log_error("Failed to create asset parent directory", e))
}

fn asset_exists(target_path: &Path, asset_type: AssetType) -> bool {
    match asset_type {
        AssetType::ZipArchive => target_path.is_dir(),
        AssetType::RawFile => target_path.is_file(),
    }
}

fn emit_download_progress(
    app_handle: &AppHandle,
    event_name: &str,
    asset_id: &str,
    progress: u8,
    status: &str,
) {
    let _ = app_handle.emit(
        event_name,
        AssetDownloadProgressPayload {
            id: asset_id.to_string(),
            progress,
            status: status.to_string(),
        },
    );
}

fn extract_zip_archive(zip_path: &Path, output_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| utils::format_and_log_error("Failed to open downloaded archive", e))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| utils::format_and_log_error("Failed to read archive", e))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| utils::format_and_log_error("Failed to read archive entry", e))?;
        let relative_path = entry.enclosed_name().ok_or_else(|| {
            utils::format_and_log_error(
                "Refusing to extract archive entry with invalid path",
                entry.name(),
            )
        })?;
        let destination = output_dir.join(relative_path);

        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|e| {
                utils::format_and_log_error("Failed to create extracted directory", e)
            })?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                utils::format_and_log_error("Failed to create extracted parent directory", e)
            })?;
        }

        let mut output_file = fs::File::create(&destination)
            .map_err(|e| utils::format_and_log_error("Failed to create extracted file", e))?;
        io::copy(&mut entry, &mut output_file)
            .map_err(|e| utils::format_and_log_error("Failed to extract file", e))?;
    }

    Ok(())
}
