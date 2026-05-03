use crate::utils;
use bzip2::read::BzDecoder;
use futures_util::StreamExt;
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use std::{fs, io};
use tar::Archive;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::OnceCell;
use zip::ZipArchive;

const CLOUDFLARE_TRACE_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
const HUGGING_FACE_URL_PREFIX: &str = "https://huggingface.co/";
const HUGGING_FACE_MIRROR_PREFIX: &str = "https://hf-mirror.com/";
static SHOULD_USE_HF_MIRROR: OnceCell<bool> = OnceCell::const_new();

#[derive(Clone, Copy)]
pub enum AssetType {
    #[allow(dead_code)]
    ZipArchive,
    SevenZipArchive,
    TarBz2Archive,
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
    let download_path = match archive_extension(asset_type) {
        Some(extension) => archive_download_path(target_path, asset_id, extension)?,
        None => target_path.to_path_buf(),
    };

    let result = async {
        ensure_parent_dir(target_path)?;

        if asset_exists(target_path, asset_type) {
            emit_download_progress(app_handle, event_name, asset_id, 100, "Complete");
            return Ok(true);
        }

        if is_archive_asset(asset_type) && download_path.exists() {
            fs::remove_file(&download_path).map_err(|e| {
                utils::format_and_log_error("Failed to clear previous downloaded archive", e)
            })?;
        }

        emit_download_progress(app_handle, event_name, asset_id, 0, "Downloading...");

        let resolved_url = resolve_download_url(url).await;
        let client = reqwest::Client::new();
        let response = client
            .get(&resolved_url)
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
            AssetType::ZipArchive | AssetType::SevenZipArchive | AssetType::TarBz2Archive => {
                let extract_root = archive_extract_root(target_path, asset_type)?;
                emit_download_progress(app_handle, event_name, asset_id, 100, "Extracting...");

                let archive_path_for_extract = download_path.clone();
                let extract_root_for_extract = extract_root.to_path_buf();
                tokio::task::spawn_blocking(move || {
                    extract_archive(
                        asset_type,
                        &archive_path_for_extract,
                        &extract_root_for_extract,
                    )
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
                AssetType::ZipArchive | AssetType::SevenZipArchive | AssetType::TarBz2Archive => {
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
        AssetType::ZipArchive | AssetType::SevenZipArchive | AssetType::TarBz2Archive => {
            target_path.is_dir()
        }
        AssetType::RawFile => target_path.is_file(),
    }
}

pub fn is_recognized_download_url(candidate_url: &str, official_url: &str) -> bool {
    let trimmed_candidate = candidate_url.trim();
    trimmed_candidate == official_url
        || to_huggingface_mirror_url(official_url)
            .as_deref()
            .is_some_and(|mirror_url| mirror_url == trimmed_candidate)
}

pub fn to_huggingface_mirror_url(url: &str) -> Option<String> {
    url.strip_prefix(HUGGING_FACE_URL_PREFIX)
        .map(|suffix| format!("{HUGGING_FACE_MIRROR_PREFIX}{suffix}"))
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

fn archive_extension(asset_type: AssetType) -> Option<&'static str> {
    match asset_type {
        AssetType::ZipArchive => Some("zip"),
        AssetType::SevenZipArchive => Some("7z"),
        AssetType::TarBz2Archive => Some("tar.bz2"),
        AssetType::RawFile => None,
    }
}

fn archive_download_path(
    target_path: &Path,
    asset_id: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    let parent = target_path.parent().ok_or_else(|| {
        format!(
            "Archive target path for '{}' must have a parent directory.",
            asset_id
        )
    })?;
    Ok(parent.join(format!("{}.{}", asset_id, extension)))
}

fn archive_extract_root(target_path: &Path, asset_type: AssetType) -> Result<&Path, String> {
    target_path.parent().ok_or_else(|| match asset_type {
        AssetType::ZipArchive => {
            "Zip archive target path must have a parent directory.".to_string()
        }
        AssetType::SevenZipArchive => {
            "7z archive target path must have a parent directory.".to_string()
        }
        AssetType::TarBz2Archive => {
            "tar.bz2 archive target path must have a parent directory.".to_string()
        }
        AssetType::RawFile => "Raw file target path must have a parent directory.".to_string(),
    })
}

fn is_archive_asset(asset_type: AssetType) -> bool {
    matches!(
        asset_type,
        AssetType::ZipArchive | AssetType::SevenZipArchive | AssetType::TarBz2Archive
    )
}

fn extract_archive(
    asset_type: AssetType,
    archive_path: &Path,
    output_dir: &Path,
) -> Result<(), String> {
    match asset_type {
        AssetType::ZipArchive => extract_zip_archive(archive_path, output_dir),
        AssetType::SevenZipArchive => extract_7z_archive(archive_path, output_dir),
        AssetType::TarBz2Archive => extract_tar_bz2_archive(archive_path, output_dir),
        AssetType::RawFile => Ok(()),
    }
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

fn extract_7z_archive(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    sevenz_rust::decompress_file_with_extract_fn(archive_path, output_dir, |entry, reader, _| {
        let relative_path = normalize_archive_relative_path(Path::new(entry.name()))
            .map_err(sevenz_rust::Error::other)?;
        let normalized_destination = output_dir.join(relative_path);
        extract_7z_entry(reader, &normalized_destination, entry.is_directory())
            .map_err(sevenz_rust::Error::other)?;
        Ok(true)
    })
    .map_err(|e| utils::format_and_log_error("Failed to extract 7z archive", e))
}

fn extract_tar_bz2_archive(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path)
        .map_err(|e| utils::format_and_log_error("Failed to open downloaded archive", e))?;
    let decoder = BzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| utils::format_and_log_error("Failed to read tar.bz2 archive", e))?
    {
        let mut entry = entry
            .map_err(|e| utils::format_and_log_error("Failed to inspect tar.bz2 entry", e))?;
        let relative_path = normalize_archive_relative_path(
            &entry
                .path()
                .map_err(|e| utils::format_and_log_error("Failed to resolve tar.bz2 entry path", e))?,
        )?;
        let destination = output_dir.join(relative_path);

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                utils::format_and_log_error("Failed to create extracted parent directory", e)
            })?;
        }

        entry
            .unpack(&destination)
            .map_err(|e| utils::format_and_log_error("Failed to extract tar.bz2 entry", e))?;
    }

    Ok(())
}

fn extract_7z_entry(
    reader: &mut dyn io::Read,
    destination: &Path,
    is_directory: bool,
) -> Result<(), String> {
    if is_directory {
        fs::create_dir_all(destination)
            .map_err(|e| utils::format_and_log_error("Failed to create extracted directory", e))?;
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            utils::format_and_log_error("Failed to create extracted parent directory", e)
        })?;
    }

    let mut output_file = fs::File::create(destination)
        .map_err(|e| utils::format_and_log_error("Failed to create extracted file", e))?;
    io::copy(reader, &mut output_file)
        .map_err(|e| utils::format_and_log_error("Failed to extract file", e))?;
    Ok(())
}

fn normalize_archive_relative_path(path: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(format!(
                    "Refusing to extract archive entry with invalid path '{}'.",
                    path.display()
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err("Refusing to extract archive entry with an empty path.".to_string());
    }

    Ok(normalized)
}

async fn resolve_download_url(url: &str) -> String {
    if should_use_hf_mirror(url).await {
        if let Some(mirror_url) = to_huggingface_mirror_url(url) {
            log::info!(
                "Using Hugging Face mirror for asset download: {}",
                mirror_url
            );
            return mirror_url;
        }
    }

    url.to_string()
}

async fn should_use_hf_mirror(url: &str) -> bool {
    if !url.starts_with(HUGGING_FACE_URL_PREFIX) {
        return false;
    }

    *SHOULD_USE_HF_MIRROR
        .get_or_init(|| async { detect_mainland_china_ip().await })
        .await
}

async fn detect_mainland_china_ip() -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            log::warn!("Failed to build IP region detection client: {}", error);
            return false;
        }
    };

    let response = match client.get(CLOUDFLARE_TRACE_URL).send().await {
        Ok(response) => response,
        Err(error) => {
            log::warn!("Failed to detect IP region via Cloudflare trace: {}", error);
            return false;
        }
    };

    let body = match response.error_for_status() {
        Ok(response) => match response.text().await {
            Ok(body) => body,
            Err(error) => {
                log::warn!("Failed to read IP region detection response: {}", error);
                return false;
            }
        },
        Err(error) => {
            log::warn!("IP region detection request failed: {}", error);
            return false;
        }
    };

    let is_mainland_china = body.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key == "loc" {
            Some(value.eq_ignore_ascii_case("CN"))
        } else {
            None
        }
    });

    let use_mirror = is_mainland_china.unwrap_or(false);
    log::info!(
        "Asset download route detection result: use_hf_mirror={}",
        use_mirror
    );
    use_mirror
}
