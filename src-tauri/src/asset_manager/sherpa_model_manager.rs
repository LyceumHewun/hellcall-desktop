use super::asset_downloader::{self, AssetType};
use crate::utils;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const STT_MODEL_DOWNLOAD_EVENT: &str = "sherpa-stt-download-progress";
const TTS_MODEL_DOWNLOAD_EVENT: &str = "sherpa-tts-download-progress";
#[cfg(target_os = "windows")]
const PROGRAM_DATA_SPEECH_LINKS_DIR: &str = r"C:\ProgramData\HellcallDesktop\sherpa-model-links";

#[derive(Clone, Copy)]
struct AvailableSherpaModelDefinition {
    id: &'static str,
    url: &'static str,
}

const AVAILABLE_STT_MODELS: &[AvailableSherpaModelDefinition] = &[AvailableSherpaModelDefinition {
    id: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2",
}];

const AVAILABLE_TTS_MODELS: &[AvailableSherpaModelDefinition] = &[AvailableSherpaModelDefinition {
    id: "vits-melo-tts-zh_en",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-melo-tts-zh_en.tar.bz2",
}];

#[derive(Debug, Clone, Serialize)]
pub struct AvailableSherpaModel {
    pub id: String,
    pub url: String,
    pub is_downloaded: bool,
}

#[derive(Debug, Clone)]
pub struct SherpaSttModelPaths {
    pub model: PathBuf,
    pub tokens: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SherpaTtsModelPaths {
    pub model: PathBuf,
    pub tokens: PathBuf,
    pub lexicon: PathBuf,
    pub dict_dir: PathBuf,
    pub rule_fsts: Vec<PathBuf>,
}

pub fn get_available_stt_models(app_handle: &AppHandle) -> Result<Vec<AvailableSherpaModel>, String> {
    let models_dir = resolve_sherpa_models_dir(app_handle)?.join("stt");

    Ok(AVAILABLE_STT_MODELS
        .iter()
        .map(|model| AvailableSherpaModel {
            id: model.id.to_string(),
            url: model.url.to_string(),
            is_downloaded: models_dir.join(model.id).is_dir(),
        })
        .collect())
}

pub fn get_available_tts_models(app_handle: &AppHandle) -> Result<Vec<AvailableSherpaModel>, String> {
    let models_dir = resolve_sherpa_models_dir(app_handle)?.join("tts");

    Ok(AVAILABLE_TTS_MODELS
        .iter()
        .map(|model| AvailableSherpaModel {
            id: model.id.to_string(),
            url: model.url.to_string(),
            is_downloaded: models_dir.join(model.id).is_dir(),
        })
        .collect())
}

pub async fn download_stt_model(
    app_handle: &AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    download_model(
        app_handle,
        &model_id,
        &url,
        AVAILABLE_STT_MODELS,
        &resolve_sherpa_models_dir(app_handle)?.join("stt"),
        STT_MODEL_DOWNLOAD_EVENT,
    )
    .await
}

pub async fn download_tts_model(
    app_handle: &AppHandle,
    model_id: String,
    url: String,
) -> Result<bool, String> {
    download_model(
        app_handle,
        &model_id,
        &url,
        AVAILABLE_TTS_MODELS,
        &resolve_sherpa_models_dir(app_handle)?.join("tts"),
        TTS_MODEL_DOWNLOAD_EVENT,
    )
    .await
}

pub fn resolve_stt_model_paths(
    app_handle: &AppHandle,
    model_id: &str,
) -> Result<SherpaSttModelPaths, String> {
    let model_dir = ensure_ascii_safe_model_path(
        &resolve_model_dir(app_handle, model_id, &resolve_sherpa_models_dir(app_handle)?.join("stt"))?,
        model_id,
    )?;

    let model = model_dir.join("model.int8.onnx");
    let tokens = model_dir.join("tokens.txt");
    ensure_required_file(&model, "STT model")?;
    ensure_required_file(&tokens, "STT tokens")?;

    Ok(SherpaSttModelPaths { model, tokens })
}

pub fn resolve_tts_model_paths(
    app_handle: &AppHandle,
    model_id: &str,
) -> Result<SherpaTtsModelPaths, String> {
    let model_dir = ensure_ascii_safe_model_path(
        &resolve_model_dir(app_handle, model_id, &resolve_sherpa_models_dir(app_handle)?.join("tts"))?,
        model_id,
    )?;

    let model = model_dir.join("model.onnx");
    let tokens = model_dir.join("tokens.txt");
    let lexicon = model_dir.join("lexicon.txt");
    let dict_dir = model_dir.join("dict");
    let rule_fsts = [
        model_dir.join("number.fst"),
        model_dir.join("phone.fst"),
        model_dir.join("date.fst"),
        model_dir.join("new_heteronym.fst"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect::<Vec<_>>();

    ensure_required_file(&model, "TTS model")?;
    ensure_required_file(&tokens, "TTS tokens")?;
    ensure_required_file(&lexicon, "TTS lexicon")?;

    Ok(SherpaTtsModelPaths {
        model,
        tokens,
        lexicon,
        dict_dir,
        rule_fsts,
    })
}

fn resolve_sherpa_models_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_local_data_dir = app_handle.path().app_local_data_dir().map_err(|e| {
        utils::format_and_log_error("Failed to resolve app local data directory", e)
    })?;

    fs::create_dir_all(&app_local_data_dir)
        .map_err(|e| utils::format_and_log_error("Failed to create app local data directory", e))?;

    let models_dir = app_local_data_dir.join("models").join("sherpa");
    fs::create_dir_all(models_dir.join("stt"))
        .map_err(|e| utils::format_and_log_error("Failed to create sherpa stt models directory", e))?;
    fs::create_dir_all(models_dir.join("tts"))
        .map_err(|e| utils::format_and_log_error("Failed to create sherpa tts models directory", e))?;

    Ok(models_dir)
}

async fn download_model(
    app_handle: &AppHandle,
    model_id: &str,
    url: &str,
    definitions: &[AvailableSherpaModelDefinition],
    models_dir: &Path,
    event_name: &str,
) -> Result<bool, String> {
    let official_model = definitions
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| format!("Unknown sherpa model '{}'.", model_id))?;
    let download_url = if url.trim().is_empty() {
        official_model.url
    } else if asset_downloader::is_recognized_download_url(url, official_model.url) {
        official_model.url
    } else {
        return Err(format!(
            "Download URL for '{}' does not match the official model list.",
            model_id
        ));
    };

    let model_path = models_dir.join(model_id);
    asset_downloader::download_asset(
        app_handle,
        model_id,
        download_url,
        &model_path,
        AssetType::TarBz2Archive,
        event_name,
    )
    .await
}

fn resolve_model_dir(
    app_handle: &AppHandle,
    model_id: &str,
    models_dir: &Path,
) -> Result<PathBuf, String> {
    let selected_model_id = model_id.trim();
    if selected_model_id.is_empty() {
        return Err("No sherpa model selected. Please choose and download one first.".to_string());
    }

    let _ = app_handle;
    let model_path = models_dir.join(selected_model_id);
    if !model_path.is_dir() {
        return Err(format!(
            "Sherpa model '{}' is not downloaded. Please download it first.",
            selected_model_id
        ));
    }

    Ok(model_path)
}

fn ensure_required_file(path: &Path, label: &str) -> Result<(), String> {
    if path.is_file() {
        return Ok(());
    }

    Err(format!(
        "{} is missing at '{}'. Please re-download the selected sherpa model.",
        label,
        path.display()
    ))
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
            "Sherpa model path contains non-ASCII characters, using ProgramData junction '{}' -> '{}'",
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
    PathBuf::from(PROGRAM_DATA_SPEECH_LINKS_DIR).join(sanitize_model_id_for_link_name(model_id))
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
        "default-sherpa-model".to_string()
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
        utils::format_and_log_error("Failed to create ProgramData sherpa link directory", e)
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
        utils::format_and_log_error("Failed to inspect stale ProgramData sherpa junction", e)
    })?;

    if metadata.file_type().is_dir() {
        fs::remove_dir(link_path).map_err(|e| {
            utils::format_and_log_error("Failed to remove stale ProgramData sherpa junction", e)
        })?;
        return Ok(());
    }

    fs::remove_file(link_path).map_err(|e| {
        utils::format_and_log_error("Failed to remove stale ProgramData sherpa link file", e)
    })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn create_windows_junction(link_path: &Path, target_path: &Path) -> Result<(), String> {
    junction::create(target_path, link_path).map_err(|e| {
        utils::format_and_log_error("Failed to create ProgramData junction for sherpa model", e)
    })
}
