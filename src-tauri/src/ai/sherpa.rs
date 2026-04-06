use crate::asset_manager::{sherpa_model_manager, sherpa_runtime_manager};
use crate::hellcall::config::AiConfig;
use libloading::Library;
use serde::Deserialize;
use std::ffi::{c_char, CStr, CString};
use std::path::Path;

#[repr(C)]
struct SherpaOnnxOfflineRecognizer {
    _private: [u8; 0],
}

#[repr(C)]
struct SherpaOnnxOfflineStream {
    _private: [u8; 0],
}

#[repr(C)]
struct SherpaOnnxOfflineTts {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxFeatureConfig {
    sample_rate: i32,
    feature_dim: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTransducerModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
    joiner: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineParaformerModelConfig {
    model: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineNemoEncDecCtcModelConfig {
    model: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineWhisperModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
    language: *const c_char,
    task: *const c_char,
    tail_paddings: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTdnnModelConfig {
    model: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineSenseVoiceModelConfig {
    model: *const c_char,
    language: *const c_char,
    use_inverse_text_normalization: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineMoonshineModelConfig {
    preprocessor: *const c_char,
    encoder: *const c_char,
    uncached_decoder: *const c_char,
    cached_decoder: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineFireRedAsrModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineDolphinModelConfig {
    model: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineZipformerCtcModelConfig {
    model: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineCanaryModelConfig {
    encoder: *const c_char,
    decoder: *const c_char,
    src_lang: *const c_char,
    tgt_lang: *const c_char,
    use_pnc: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineLMConfig {
    model: *const c_char,
    scale: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxHomophoneReplacerConfig {
    dict_dir: *const c_char,
    lexicon: *const c_char,
    rule_fsts: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineModelConfig {
    transducer: SherpaOnnxOfflineTransducerModelConfig,
    paraformer: SherpaOnnxOfflineParaformerModelConfig,
    nemo_ctc: SherpaOnnxOfflineNemoEncDecCtcModelConfig,
    whisper: SherpaOnnxOfflineWhisperModelConfig,
    tdnn: SherpaOnnxOfflineTdnnModelConfig,
    tokens: *const c_char,
    num_threads: i32,
    debug: i32,
    provider: *const c_char,
    model_type: *const c_char,
    modeling_unit: *const c_char,
    bpe_vocab: *const c_char,
    telespeech_ctc: *const c_char,
    sense_voice: SherpaOnnxOfflineSenseVoiceModelConfig,
    moonshine: SherpaOnnxOfflineMoonshineModelConfig,
    fire_red_asr: SherpaOnnxOfflineFireRedAsrModelConfig,
    dolphin: SherpaOnnxOfflineDolphinModelConfig,
    zipformer_ctc: SherpaOnnxOfflineZipformerCtcModelConfig,
    canary: SherpaOnnxOfflineCanaryModelConfig,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineRecognizerConfig {
    feat: SherpaOnnxFeatureConfig,
    model: SherpaOnnxOfflineModelConfig,
    lm: SherpaOnnxOfflineLMConfig,
    decoding_method: *const c_char,
    max_active_paths: i32,
    hotwords_file: *const c_char,
    hotwords_score: f32,
    rule_fsts: *const c_char,
    rule_fars: *const c_char,
    blank_penalty: f32,
    hr: SherpaOnnxHomophoneReplacerConfig,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTtsVitsModelConfig {
    model: *const c_char,
    lexicon: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    noise_scale: f32,
    noise_scale_w: f32,
    length_scale: f32,
    dict_dir: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTtsMatchaModelConfig {
    acoustic_model: *const c_char,
    vocoder: *const c_char,
    lexicon: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    noise_scale: f32,
    length_scale: f32,
    dict_dir: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTtsKokoroModelConfig {
    model: *const c_char,
    voices: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    length_scale: f32,
    dict_dir: *const c_char,
    lexicon: *const c_char,
    lang: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTtsKittenModelConfig {
    model: *const c_char,
    voices: *const c_char,
    tokens: *const c_char,
    data_dir: *const c_char,
    length_scale: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTtsModelConfig {
    vits: SherpaOnnxOfflineTtsVitsModelConfig,
    num_threads: i32,
    debug: i32,
    provider: *const c_char,
    matcha: SherpaOnnxOfflineTtsMatchaModelConfig,
    kokoro: SherpaOnnxOfflineTtsKokoroModelConfig,
    kitten: SherpaOnnxOfflineTtsKittenModelConfig,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SherpaOnnxOfflineTtsConfig {
    model: SherpaOnnxOfflineTtsModelConfig,
    rule_fsts: *const c_char,
    max_num_sentences: i32,
    rule_fars: *const c_char,
    silence_scale: f32,
}

#[repr(C)]
struct SherpaOnnxGeneratedAudio {
    samples: *const f32,
    n: i32,
    sample_rate: i32,
}

type CreateOfflineRecognizerFn = unsafe extern "C" fn(
    *const SherpaOnnxOfflineRecognizerConfig,
) -> *mut SherpaOnnxOfflineRecognizer;
type DestroyOfflineRecognizerFn = unsafe extern "C" fn(*mut SherpaOnnxOfflineRecognizer);
type CreateOfflineStreamFn =
    unsafe extern "C" fn(*mut SherpaOnnxOfflineRecognizer) -> *mut SherpaOnnxOfflineStream;
type DestroyOfflineStreamFn = unsafe extern "C" fn(*mut SherpaOnnxOfflineStream);
type AcceptWaveformOfflineFn =
    unsafe extern "C" fn(*mut SherpaOnnxOfflineStream, i32, *const f32, i32);
type DecodeOfflineStreamFn =
    unsafe extern "C" fn(*mut SherpaOnnxOfflineRecognizer, *mut SherpaOnnxOfflineStream);
type GetOfflineStreamResultAsJsonFn =
    unsafe extern "C" fn(*mut SherpaOnnxOfflineStream) -> *const c_char;
type DestroyOfflineStreamResultJsonFn = unsafe extern "C" fn(*const c_char);
type CreateOfflineTtsFn =
    unsafe extern "C" fn(*const SherpaOnnxOfflineTtsConfig) -> *mut SherpaOnnxOfflineTts;
type DestroyOfflineTtsFn = unsafe extern "C" fn(*mut SherpaOnnxOfflineTts);
type OfflineTtsGenerateFn = unsafe extern "C" fn(
    *mut SherpaOnnxOfflineTts,
    *const c_char,
    i32,
    f32,
) -> *const SherpaOnnxGeneratedAudio;
type DestroyOfflineTtsGeneratedAudioFn =
    unsafe extern "C" fn(*const SherpaOnnxGeneratedAudio);

#[derive(Deserialize)]
struct OfflineResultJson {
    text: String,
}

struct SherpaOnnxApi {
    _onnxruntime_lib: Library,
    _c_api_lib: Library,
    create_offline_recognizer: CreateOfflineRecognizerFn,
    destroy_offline_recognizer: DestroyOfflineRecognizerFn,
    create_offline_stream: CreateOfflineStreamFn,
    destroy_offline_stream: DestroyOfflineStreamFn,
    accept_waveform_offline: AcceptWaveformOfflineFn,
    decode_offline_stream: DecodeOfflineStreamFn,
    get_offline_stream_result_as_json: GetOfflineStreamResultAsJsonFn,
    destroy_offline_stream_result_json: DestroyOfflineStreamResultJsonFn,
    create_offline_tts: CreateOfflineTtsFn,
    destroy_offline_tts: DestroyOfflineTtsFn,
    offline_tts_generate: OfflineTtsGenerateFn,
    destroy_offline_tts_generated_audio: DestroyOfflineTtsGeneratedAudioFn,
}

pub struct SherpaSpeechRuntime {
    api: SherpaOnnxApi,
    recognizer: Option<LoadedRecognizer>,
    tts: Option<LoadedTts>,
}

struct LoadedRecognizer {
    key: String,
    handle: *mut SherpaOnnxOfflineRecognizer,
}

struct LoadedTts {
    key: String,
    handle: *mut SherpaOnnxOfflineTts,
}

unsafe impl Send for SherpaSpeechRuntime {}

impl SherpaSpeechRuntime {
    pub fn new(app_handle: &tauri::AppHandle) -> Result<Self, String> {
        let runtime_paths = sherpa_runtime_manager::resolve_runtime_paths(app_handle)?;
        let api = SherpaOnnxApi::load(&runtime_paths.c_api_dll, &runtime_paths.onnxruntime_dll)?;

        Ok(Self {
            api,
            recognizer: None,
            tts: None,
        })
    }

    pub fn transcribe(
        &mut self,
        app_handle: &tauri::AppHandle,
        ai_config: &AiConfig,
        samples: &[i16],
        sample_rate: i32,
    ) -> Result<String, String> {
        let recognizer_handle = self.ensure_recognizer(app_handle, ai_config)?.handle;
        let create_offline_stream = self.api.create_offline_stream;
        let accept_waveform_offline = self.api.accept_waveform_offline;
        let decode_offline_stream = self.api.decode_offline_stream;
        let get_result_as_json = self.api.get_offline_stream_result_as_json;
        let destroy_result_json = self.api.destroy_offline_stream_result_json;
        let destroy_offline_stream = self.api.destroy_offline_stream;

        let stream = unsafe { create_offline_stream(recognizer_handle) };
        if stream.is_null() {
            return Err("Failed to create sherpa offline stream.".to_string());
        }

        let pcm = samples
            .iter()
            .map(|sample| *sample as f32 / 32768.0)
            .collect::<Vec<_>>();

        unsafe {
            accept_waveform_offline(
                stream,
                sample_rate,
                pcm.as_ptr(),
                pcm.len() as i32,
            );
            decode_offline_stream(recognizer_handle, stream);
        }

        let result_ptr = unsafe { get_result_as_json(stream) };
        let transcript = if result_ptr.is_null() {
            Err("Sherpa STT returned an empty result.".to_string())
        } else {
            let result_json = unsafe { CStr::from_ptr(result_ptr) }
                .to_str()
                .map_err(|e| e.to_string())?
                .to_string();
            let parsed = serde_json::from_str::<OfflineResultJson>(&result_json)
                .map_err(|e| format!("Failed to parse sherpa STT result: {}", e))?;
            Ok(parsed.text.trim().to_string())
        };

        unsafe {
            if !result_ptr.is_null() {
                destroy_result_json(result_ptr);
            }
            destroy_offline_stream(stream);
        }

        transcript
    }

    pub fn synthesize(
        &mut self,
        app_handle: &tauri::AppHandle,
        ai_config: &AiConfig,
        text: &str,
    ) -> Result<GeneratedAudio, String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("AI TTS input text is empty.".to_string());
        }

        let tts_handle = self.ensure_tts(app_handle, ai_config)?.handle;
        let input = CString::new(trimmed).map_err(|e| e.to_string())?;
        let speed = ai_config.speech.tts.speed.clamp(0.5, 2.0);
        let offline_tts_generate = self.api.offline_tts_generate;
        let generated = unsafe {
            offline_tts_generate(tts_handle, input.as_ptr(), ai_config.speech.tts.speaker_id, speed)
        };

        if generated.is_null() {
            return Err("Sherpa TTS failed to generate audio.".to_string());
        }

        let destroy_generated_audio = self.api.destroy_offline_tts_generated_audio;
        let output = unsafe {
            let audio = &*generated;
            if audio.samples.is_null() || audio.n <= 0 || audio.sample_rate <= 0 {
                destroy_generated_audio(generated);
                return Err("Sherpa TTS returned empty audio.".to_string());
            }

            let samples = std::slice::from_raw_parts(audio.samples, audio.n as usize).to_vec();
            let sample_rate = audio.sample_rate;
            destroy_generated_audio(generated);

            GeneratedAudio {
                samples,
                sample_rate,
            }
        };

        Ok(output)
    }

    pub fn invalidate_models(&mut self) {
        if let Some(recognizer) = self.recognizer.take() {
            unsafe {
                (self.api.destroy_offline_recognizer)(recognizer.handle);
            }
        }
        if let Some(tts) = self.tts.take() {
            unsafe {
                (self.api.destroy_offline_tts)(tts.handle);
            }
        }
    }

    fn ensure_recognizer(
        &mut self,
        app_handle: &tauri::AppHandle,
        ai_config: &AiConfig,
    ) -> Result<&LoadedRecognizer, String> {
        let key = format!(
            "{}|{}|{}",
            ai_config.speech.stt.model_id,
            ai_config.speech.stt.language,
            ai_config.speech.stt.use_itn
        );

        let needs_reload = self
            .recognizer
            .as_ref()
            .is_none_or(|recognizer| recognizer.key != key);

        if needs_reload {
            if let Some(recognizer) = self.recognizer.take() {
                unsafe {
                    (self.api.destroy_offline_recognizer)(recognizer.handle);
                }
            }

            let model_paths =
                sherpa_model_manager::resolve_stt_model_paths(app_handle, &ai_config.speech.stt.model_id)?;
            let recognizer =
                self.create_recognizer(&model_paths.model, &model_paths.tokens, &ai_config.speech.stt.language, ai_config.speech.stt.use_itn)?;
            self.recognizer = Some(LoadedRecognizer {
                key,
                handle: recognizer,
            });
        }

        self.recognizer
            .as_ref()
            .ok_or_else(|| "Sherpa STT recognizer is unavailable.".to_string())
    }

    fn ensure_tts(
        &mut self,
        app_handle: &tauri::AppHandle,
        ai_config: &AiConfig,
    ) -> Result<&LoadedTts, String> {
        let key = ai_config.speech.tts.model_id.clone();
        let needs_reload = self.tts.as_ref().is_none_or(|tts| tts.key != key);

        if needs_reload {
            if let Some(tts) = self.tts.take() {
                unsafe {
                    (self.api.destroy_offline_tts)(tts.handle);
                }
            }

            let model_paths =
                sherpa_model_manager::resolve_tts_model_paths(app_handle, &ai_config.speech.tts.model_id)?;
            let tts = self.create_tts(&model_paths)?;
            self.tts = Some(LoadedTts { key, handle: tts });
        }

        self.tts
            .as_ref()
            .ok_or_else(|| "Sherpa TTS runtime is unavailable.".to_string())
    }

    fn create_recognizer(
        &self,
        model_path: &Path,
        tokens_path: &Path,
        language: &str,
        use_itn: bool,
    ) -> Result<*mut SherpaOnnxOfflineRecognizer, String> {
        let model = CString::new(path_to_string(model_path)?).map_err(|e| e.to_string())?;
        let tokens = CString::new(path_to_string(tokens_path)?).map_err(|e| e.to_string())?;
        let language = CString::new(if language.trim().is_empty() {
            "auto".to_string()
        } else {
            language.trim().to_string()
        })
        .map_err(|e| e.to_string())?;
        let provider = CString::new("cpu").unwrap();
        let decoding_method = CString::new("greedy_search").unwrap();

        let mut config: SherpaOnnxOfflineRecognizerConfig = zeroed();
        config.feat.sample_rate = 16_000;
        config.feat.feature_dim = 80;
        config.model.tokens = tokens.as_ptr();
        config.model.num_threads = 2;
        config.model.debug = 0;
        config.model.provider = provider.as_ptr();
        config.model.sense_voice.model = model.as_ptr();
        config.model.sense_voice.language = language.as_ptr();
        config.model.sense_voice.use_inverse_text_normalization = i32::from(use_itn);
        config.decoding_method = decoding_method.as_ptr();
        config.max_active_paths = 4;
        config.hotwords_score = 1.5;

        let recognizer = unsafe { (self.api.create_offline_recognizer)(&config) };
        if recognizer.is_null() {
            return Err("Failed to initialize sherpa STT recognizer. Please verify the selected model files.".to_string());
        }

        Ok(recognizer)
    }

    fn create_tts(
        &self,
        model_paths: &sherpa_model_manager::SherpaTtsModelPaths,
    ) -> Result<*mut SherpaOnnxOfflineTts, String> {
        let model = CString::new(path_to_string(&model_paths.model)?).map_err(|e| e.to_string())?;
        let lexicon =
            CString::new(path_to_string(&model_paths.lexicon)?).map_err(|e| e.to_string())?;
        let tokens =
            CString::new(path_to_string(&model_paths.tokens)?).map_err(|e| e.to_string())?;
        let dict_dir = CString::new(path_to_string(&model_paths.dict_dir).unwrap_or_default())
            .map_err(|e| e.to_string())?;
        let provider = CString::new("cpu").unwrap();
        let rule_fsts = CString::new(
            model_paths
                .rule_fsts
                .iter()
                .filter_map(|path| path_to_string(path).ok())
                .collect::<Vec<_>>()
                .join(","),
        )
        .map_err(|e| e.to_string())?;

        let mut config: SherpaOnnxOfflineTtsConfig = zeroed();
        config.model.vits.model = model.as_ptr();
        config.model.vits.lexicon = lexicon.as_ptr();
        config.model.vits.tokens = tokens.as_ptr();
        config.model.vits.noise_scale = 0.667;
        config.model.vits.noise_scale_w = 0.8;
        config.model.vits.length_scale = 1.0;
        config.model.vits.dict_dir = dict_dir.as_ptr();
        config.model.num_threads = 2;
        config.model.debug = 0;
        config.model.provider = provider.as_ptr();
        config.rule_fsts = rule_fsts.as_ptr();
        config.max_num_sentences = 1;
        config.silence_scale = 0.2;

        let tts = unsafe { (self.api.create_offline_tts)(&config) };
        if tts.is_null() {
            return Err("Failed to initialize sherpa TTS runtime. Please verify the selected model files.".to_string());
        }

        Ok(tts)
    }
}

impl Drop for SherpaSpeechRuntime {
    fn drop(&mut self) {
        self.invalidate_models();
    }
}

pub struct GeneratedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: i32,
}

impl SherpaOnnxApi {
    fn load(c_api_dll: &Path, onnxruntime_dll: &Path) -> Result<Self, String> {
        let onnxruntime_lib = unsafe { Library::new(onnxruntime_dll) }.map_err(|e| {
            format!(
                "Failed to load sherpa dependency '{}': {}",
                onnxruntime_dll.display(),
                e
            )
        })?;
        let c_api_lib = unsafe { Library::new(c_api_dll) }.map_err(|e| {
            format!(
                "Failed to load sherpa runtime '{}': {}",
                c_api_dll.display(),
                e
            )
        })?;

        unsafe {
            Ok(Self {
                create_offline_recognizer: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxCreateOfflineRecognizer\0",
                )?,
                destroy_offline_recognizer: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxDestroyOfflineRecognizer\0",
                )?,
                create_offline_stream: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxCreateOfflineStream\0",
                )?,
                destroy_offline_stream: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxDestroyOfflineStream\0",
                )?,
                accept_waveform_offline: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxAcceptWaveformOffline\0",
                )?,
                decode_offline_stream: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxDecodeOfflineStream\0",
                )?,
                get_offline_stream_result_as_json: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxGetOfflineStreamResultAsJson\0",
                )?,
                destroy_offline_stream_result_json: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxDestroyOfflineStreamResultJson\0",
                )?,
                create_offline_tts: load_symbol(&c_api_lib, b"SherpaOnnxCreateOfflineTts\0")?,
                destroy_offline_tts: load_symbol(&c_api_lib, b"SherpaOnnxDestroyOfflineTts\0")?,
                offline_tts_generate: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxOfflineTtsGenerate\0",
                )?,
                destroy_offline_tts_generated_audio: load_symbol(
                    &c_api_lib,
                    b"SherpaOnnxDestroyOfflineTtsGeneratedAudio\0",
                )?,
                _onnxruntime_lib: onnxruntime_lib,
                _c_api_lib: c_api_lib,
            })
        }
    }
}

unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    library
        .get::<T>(name)
        .map(|symbol| *symbol)
        .map_err(|e| format!("Failed to load sherpa symbol '{}': {}", symbol_name(name), e))
}

fn symbol_name(symbol: &[u8]) -> String {
    symbol
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .map(char::from)
        .collect()
}

fn path_to_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(|value| value.to_string())
        .ok_or_else(|| format!("Path '{}' is not valid UTF-8.", path.display()))
}

fn zeroed<T>() -> T {
    unsafe { std::mem::zeroed() }
}
