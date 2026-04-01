use crate::hellcall::config::AiConfig;
use futures_util::StreamExt;
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct AiTranscriptionResult {
    pub session_id: String,
    pub transcript: String,
    pub audio_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub function: ChatToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct ChatStreamResult {
    pub content: String,
    pub tool_calls: Vec<ChatToolCall>,
}

#[derive(Debug, Deserialize)]
struct AudioTranscriptionResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChunkChoice {
    delta: ChatDelta,
}

#[derive(Debug, Deserialize)]
struct ChatDelta {
    content: Option<String>,
    tool_calls: Option<Vec<ChatToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ChatToolCallDelta {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    type_name: Option<String>,
    function: Option<ChatToolFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct ChatToolFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PartialToolCall {
    id: String,
    type_name: String,
    function_name: String,
    arguments: String,
}

pub async fn transcribe_audio(ai_config: &AiConfig, audio_path: &Path) -> Result<String, String> {
    let api_key = ai_config.api_key.trim();
    if api_key.is_empty() {
        return Err("AI API key is empty. Please fill it in Global Settings.".to_string());
    }

    let url = format!(
        "{}/audio/transcriptions",
        ai_config.base_url.trim_end_matches('/')
    );

    let file_bytes = fs::read(audio_path).map_err(|e| e.to_string())?;
    let file_part = Part::bytes(file_bytes)
        .file_name("recording.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = Form::new()
        .text("model", ai_config.default_asr_model.clone())
        .part("file", file_part);

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("ASR request failed with {}: {}", status, body));
    }

    let body = response
        .json::<AudioTranscriptionResponse>()
        .await
        .map_err(|e| e.to_string())?;

    Ok(body.text.trim().to_string())
}

pub async fn stream_chat_completion(
    ai_config: &AiConfig,
    body: Value,
    mut on_delta: impl FnMut(&str) -> Result<(), String>,
) -> Result<ChatStreamResult, String> {
    let api_key = ai_config.api_key.trim();
    if api_key.is_empty() {
        return Err("AI API key is empty. Please fill it in Global Settings.".to_string());
    }

    let url = format!(
        "{}/chat/completions",
        ai_config.base_url.trim_end_matches('/')
    );

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Chat request failed with {}: {}", status, body));
    }

    let mut content = String::new();
    let mut pending = String::new();
    let mut partial_tool_calls: BTreeMap<usize, PartialToolCall> = BTreeMap::new();
    let mut bytes_stream = response.bytes_stream();

    while let Some(chunk) = bytes_stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        let chunk_text = String::from_utf8_lossy(&chunk).replace("\r\n", "\n");
        pending.push_str(&chunk_text);

        while let Some(boundary) = pending.find("\n\n") {
            let event = pending[..boundary].to_string();
            pending = pending[boundary + 2..].to_string();
            process_stream_event(
                &event,
                &mut content,
                &mut partial_tool_calls,
                &mut on_delta,
            )?;
        }
    }

    if !pending.trim().is_empty() {
        process_stream_event(
            &pending,
            &mut content,
            &mut partial_tool_calls,
            &mut on_delta,
        )?;
    }

    let tool_calls = partial_tool_calls
        .into_values()
        .map(|tool_call| ChatToolCall {
            id: tool_call.id,
            type_name: if tool_call.type_name.is_empty() {
                "function".to_string()
            } else {
                tool_call.type_name
            },
            function: ChatToolFunction {
                name: tool_call.function_name,
                arguments: tool_call.arguments,
            },
        })
        .collect::<Vec<_>>();

    Ok(ChatStreamResult { content, tool_calls })
}

pub fn build_chat_request_body(
    model: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": stream,
    });

    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
        body["tool_choice"] = Value::String("auto".to_string());
    }

    body
}

fn process_stream_event(
    event: &str,
    content: &mut String,
    partial_tool_calls: &mut BTreeMap<usize, PartialToolCall>,
    on_delta: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let mut data_lines = Vec::new();
    for raw_line in event.lines() {
        let line = raw_line.trim();
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim());
        }
    }

    if data_lines.is_empty() {
        return Ok(());
    }

    let payload = data_lines.join("\n");
    if payload == "[DONE]" {
        return Ok(());
    }

    let chunk = serde_json::from_str::<ChatChunk>(&payload).map_err(|e| e.to_string())?;
    for choice in chunk.choices {
        if let Some(text) = choice.delta.content {
            content.push_str(&text);
            on_delta(&text)?;
        }

        if let Some(tool_call_deltas) = choice.delta.tool_calls {
            for tool_call_delta in tool_call_deltas {
                let entry = partial_tool_calls.entry(tool_call_delta.index).or_default();
                if let Some(id) = tool_call_delta.id {
                    entry.id = id;
                }
                if let Some(type_name) = tool_call_delta.type_name {
                    entry.type_name = type_name;
                }
                if let Some(function) = tool_call_delta.function {
                    if let Some(name) = function.name {
                        entry.function_name.push_str(&name);
                    }
                    if let Some(arguments) = function.arguments {
                        entry.arguments.push_str(&arguments);
                    }
                }
            }
        }
    }

    Ok(())
}
