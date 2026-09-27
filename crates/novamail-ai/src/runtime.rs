//! Probe local Ollama + NVIDIA GPU for first-run / settings UX.

use std::path::Path;
use std::process::Command;

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::provider::{AiError, AiResult};

/// Default when a GPU (or richer local install) is available — better reply quality.
pub const DEFAULT_MODEL: &str = "qwen3:4b-instruct";

/// Low-spec / CPU-friendly fallback.
pub const CPU_DEFAULT_MODEL: &str = "qwen2.5:1.5b";

pub fn default_ollama_url() -> String {
    std::env::var("NOVAMAIL_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into())
}

pub fn default_ollama_model() -> String {
    std::env::var("NOVAMAIL_OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into())
}

/// Best-effort NVIDIA GPU detection (nvidia-smi or Linux driver node).
pub fn detect_nvidia_gpu() -> bool {
    if Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return true;
    }
    Path::new("/proc/driver/nvidia/version").exists() || Path::new("/dev/nvidia0").exists()
}

#[derive(Debug, Clone, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Clone, Deserialize)]
struct TagModel {
    name: String,
}

/// List locally installed Ollama models via `/api/tags`.
pub async fn list_ollama_models(base_url: &str) -> AiResult<Vec<String>> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/tags");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .map_err(|e| AiError::Unavailable(e.to_string()))?;
    let response = client.get(&url).send().await.map_err(|e| {
        AiError::Unavailable(format!("Ollama unreachable at {base} ({e})"))
    })?;
    if !response.status().is_success() {
        return Err(AiError::Unavailable(format!(
            "Ollama tags HTTP {}",
            response.status()
        )));
    }
    let parsed = response
        .json::<TagsResponse>()
        .await
        .map_err(|e| AiError::Inference(e.to_string()))?;
    let mut names: Vec<String> = parsed.models.into_iter().map(|m| m.name).collect();
    names.sort();
    names.dedup();
    Ok(names)
}

pub async fn ollama_reachable(base_url: &str) -> bool {
    list_ollama_models(base_url).await.is_ok()
}

/// Whether the UI should offer a free model picker.
/// True when an NVIDIA GPU is present or Ollama already has models installed.
pub fn allow_model_pick(nvidia_gpu: bool, installed_models: &[String]) -> bool {
    nvidia_gpu || !installed_models.is_empty()
}

fn model_matches(installed: &[String], needle: &str) -> bool {
    installed
        .iter()
        .any(|m| m == needle || m.starts_with(&format!("{needle}-")) || m.starts_with(needle))
}

pub fn recommended_model(nvidia_gpu: bool, installed_models: &[String]) -> String {
    if !nvidia_gpu {
        return CPU_DEFAULT_MODEL.to_string();
    }
    if model_matches(installed_models, DEFAULT_MODEL)
        || model_matches(installed_models, "qwen3:4b")
    {
        // Prefer instruct over thinking alias when both exist.
        if model_matches(installed_models, DEFAULT_MODEL) {
            return DEFAULT_MODEL.to_string();
        }
        if let Some(hit) = installed_models
            .iter()
            .find(|m| m.starts_with("qwen3:4b-instruct"))
        {
            return hit.clone();
        }
    }
    if let Some(first) = installed_models.first() {
        return first.clone();
    }
    DEFAULT_MODEL.to_string()
}

/// Suggested catalog shown in setup/settings even if not yet pulled.
pub fn suggested_models() -> &'static [&'static str] {
    &[DEFAULT_MODEL, CPU_DEFAULT_MODEL]
}

/// Normalize a pasted Ollama name or Hugging Face URL into an Ollama pull ref.
///
/// Accepts e.g. `qwen3:4b-instruct`, `hf.co/org/model`, or
/// `https://huggingface.co/org/model` / `…/tree/main`.
pub fn normalize_model_ref(input: &str) -> AiResult<String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(AiError::Inference("Model name or link is empty".into()));
    }
    let lower = raw.to_ascii_lowercase();

    if lower.starts_with("http://") || lower.starts_with("https://") {
        let without_scheme = raw
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or(raw);
        let path = without_scheme.trim_start_matches('/');
        if let Some(rest) = path
            .strip_prefix("huggingface.co/")
            .or_else(|| path.strip_prefix("www.huggingface.co/"))
            .or_else(|| path.strip_prefix("hf.co/"))
        {
            let mut parts = rest.split('/').filter(|p| !p.is_empty());
            let owner = parts.next().unwrap_or("");
            let repo = parts
                .next()
                .unwrap_or("")
                .split('?')
                .next()
                .unwrap_or("")
                .trim();
            if owner.is_empty() || repo.is_empty() || repo == "tree" || repo == "blob" {
                return Err(AiError::Inference(
                    "Hugging Face link must look like https://huggingface.co/org/model".into(),
                ));
            }
            // Optional quant / revision after repo (query already stripped).
            let mut ref_name = format!("hf.co/{owner}/{repo}");
            if let Some(extra) = parts.next() {
                if extra != "tree" && extra != "blob" && extra != "resolve" {
                    ref_name.push(':');
                    ref_name.push_str(extra.split('?').next().unwrap_or(extra));
                }
            }
            return Ok(ref_name);
        }
        if let Some(rest) = path
            .strip_prefix("ollama.com/")
            .or_else(|| path.strip_prefix("www.ollama.com/"))
        {
            let name = rest
                .trim_start_matches("library/")
                .split('/')
                .next()
                .unwrap_or("")
                .split('?')
                .next()
                .unwrap_or("")
                .trim();
            if name.is_empty() {
                return Err(AiError::Inference("Could not parse Ollama library link".into()));
            }
            return Ok(name.to_string());
        }
        return Err(AiError::Inference(
            "Paste an Ollama model name, hf.co/… ref, or huggingface.co / ollama.com link".into(),
        ));
    }

    if lower.starts_with("hf.co/") {
        return Ok(raw.to_string());
    }

    // Bare Ollama library tag, e.g. qwen3:4b-instruct
    if raw.chars().any(|c| c.is_whitespace()) {
        return Err(AiError::Inference("Model name must not contain spaces".into()));
    }
    Ok(raw.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullProgress {
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    pub done: bool,
}

/// Pull a model through Ollama (`/api/pull`). `on_progress` receives NDJSON updates.
pub async fn pull_ollama_model<F>(
    base_url: &str,
    model_ref: &str,
    mut on_progress: F,
) -> AiResult<String>
where
    F: FnMut(PullProgress) + Send,
{
    let normalized = normalize_model_ref(model_ref)?;
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/pull");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60 * 60))
        .build()
        .map_err(|e| AiError::Unavailable(e.to_string()))?;

    #[derive(Serialize)]
    struct PullBody<'a> {
        model: &'a str,
        stream: bool,
    }

    let response = client
        .post(&url)
        .json(&PullBody {
            model: &normalized,
            stream: true,
        })
        .send()
        .await
        .map_err(|e| {
            AiError::Unavailable(format!("Ollama pull failed at {base} ({e})"))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AiError::Inference(format!(
            "Ollama pull HTTP {status}: {body}"
        )));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut last_status = String::from("pulling");

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AiError::Inference(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buffer.find('\n') {
            let line = buffer[..idx].trim().to_string();
            buffer.drain(..=idx);
            if line.is_empty() {
                continue;
            }
            #[derive(Deserialize)]
            struct PullLine {
                status: Option<String>,
                digest: Option<String>,
                total: Option<u64>,
                completed: Option<u64>,
                error: Option<String>,
            }
            let parsed: PullLine = serde_json::from_str(&line)
                .map_err(|e| AiError::Inference(format!("bad pull event: {e}; {line}")))?;
            if let Some(err) = parsed.error {
                return Err(AiError::Inference(err));
            }
            if let Some(status) = parsed.status.clone() {
                last_status = status;
            }
            let done = last_status == "success";
            on_progress(PullProgress {
                status: last_status.clone(),
                digest: parsed.digest,
                total: parsed.total,
                completed: parsed.completed,
                done,
            });
            if done {
                return Ok(normalized);
            }
        }
    }

    if last_status == "success" {
        return Ok(normalized);
    }
    // Some Ollama builds end the stream without an explicit success line.
    on_progress(PullProgress {
        status: "success".into(),
        digest: None,
        total: None,
        completed: None,
        done: true,
    });
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_path_recommends_small_model() {
        assert_eq!(recommended_model(false, &[]), CPU_DEFAULT_MODEL);
        assert!(!allow_model_pick(false, &[]));
    }

    #[test]
    fn gpu_path_recommends_qwen3_4b() {
        assert_eq!(recommended_model(true, &[]), DEFAULT_MODEL);
        assert!(allow_model_pick(true, &[]));
        assert_eq!(
            recommended_model(true, &["qwen3:4b-instruct".into(), "mistral:7b".into()]),
            DEFAULT_MODEL
        );
        assert_eq!(
            recommended_model(true, &["mistral:7b".into()]),
            "mistral:7b"
        );
    }

    #[test]
    fn normalizes_hf_and_ollama_refs() {
        assert_eq!(
            normalize_model_ref("qwen3:4b-instruct").unwrap(),
            "qwen3:4b-instruct"
        );
        assert_eq!(
            normalize_model_ref("hf.co/Qwen/Qwen3-4B-Instruct-2507-GGUF").unwrap(),
            "hf.co/Qwen/Qwen3-4B-Instruct-2507-GGUF"
        );
        assert_eq!(
            normalize_model_ref("https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507-GGUF")
                .unwrap(),
            "hf.co/Qwen/Qwen3-4B-Instruct-2507-GGUF"
        );
        assert_eq!(
            normalize_model_ref("https://huggingface.co/Qwen/Qwen3-4B-GGUF/tree/main").unwrap(),
            "hf.co/Qwen/Qwen3-4B-GGUF"
        );
        assert_eq!(
            normalize_model_ref("https://ollama.com/library/qwen3").unwrap(),
            "qwen3"
        );
    }
}
