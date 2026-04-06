use super::asset_downloader::{self, AssetType};
use crate::utils;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const SHERPA_RUNTIME_DOWNLOAD_EVENT: &str = "sherpa-runtime-download-progress";
const SHERPA_RUNTIME_TAG: &str = "v1.12.9";
const SHERPA_RUNTIME_ARCHIVE_NAME: &str = "sherpa-onnx-v1.12.9-win-x64-shared";
const SHERPA_RUNTIME_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.12.9/sherpa-onnx-v1.12.9-win-x64-shared.tar.bz2";

#[derive(Debug, Clone)]
pub struct SherpaRuntimePaths {
    pub c_api_dll: PathBuf,
    pub onnxruntime_dll: PathBuf,
}

pub fn resolve_runtime_paths(app_handle: &AppHandle) -> Result<SherpaRuntimePaths, String> {
    let runtime_root = resolve_runtime_root(app_handle)?;
    let extracted_root = runtime_root.join(SHERPA_RUNTIME_ARCHIVE_NAME);
    let lib_dir = extracted_root.join("lib");
    let c_api_dll = lib_dir.join("sherpa-onnx-c-api.dll");
    let onnxruntime_dll = lib_dir.join("onnxruntime.dll");

    if !c_api_dll.is_file() || !onnxruntime_dll.is_file() {
        return Err(format!(
            "Sherpa runtime '{}' is not ready. Please let the app finish downloading it first.",
            SHERPA_RUNTIME_TAG
        ));
    }

    Ok(SherpaRuntimePaths {
        c_api_dll,
        onnxruntime_dll,
    })
}

pub async fn ensure_runtime_ready(app_handle: &AppHandle) -> Result<SherpaRuntimePaths, String> {
    if let Ok(paths) = resolve_runtime_paths(app_handle) {
        return Ok(paths);
    }

    let runtime_root = resolve_runtime_root(app_handle)?;
    let target_path = runtime_root.join(SHERPA_RUNTIME_ARCHIVE_NAME);
    asset_downloader::download_asset(
        app_handle,
        SHERPA_RUNTIME_ARCHIVE_NAME,
        SHERPA_RUNTIME_URL,
        &target_path,
        AssetType::TarBz2Archive,
        SHERPA_RUNTIME_DOWNLOAD_EVENT,
    )
    .await?;

    resolve_runtime_paths(app_handle)
}

fn resolve_runtime_root(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_local_data_dir = app_handle.path().app_local_data_dir().map_err(|e| {
        utils::format_and_log_error("Failed to resolve app local data directory", e)
    })?;

    fs::create_dir_all(&app_local_data_dir)
        .map_err(|e| utils::format_and_log_error("Failed to create app local data directory", e))?;

    let runtime_root = app_local_data_dir.join("models").join("sherpa").join("runtime");
    fs::create_dir_all(&runtime_root)
        .map_err(|e| utils::format_and_log_error("Failed to create sherpa runtime directory", e))?;

    Ok(runtime_root)
}
