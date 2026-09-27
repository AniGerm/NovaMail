//! Probe local Ollama + NVIDIA GPU for first-run / settings UX.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::provider::{AiError, AiResult};

/// CPU-friendly default used when no NVIDIA GPU is available.
pub const CPU_DEFAULT_MODEL: &str = "qwen2.5:1.5b";

pub fn default_ollama_url() -> String {
    std::env::var("NOVAMAIL_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into())
}

pub fn default_ollama_model() -> String {
    std::env::var("NOVAMAIL_OLLAMA_MODEL").unwrap_or_else(|_| CPU_DEFAULT_MODEL.into())
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
    Path::new("/proc/driver/nvidia/version").exists()
        || Path::new("/dev/nvidia0").exists()
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

pub fn recommended_model(nvidia_gpu: bool, installed_models: &[String]) -> String {
    if !nvidia_gpu {
        return CPU_DEFAULT_MODEL.to_string();
    }
    if installed_models.iter().any(|m| m == CPU_DEFAULT_MODEL || m.starts_with("qwen2.5:1.5b"))
    {
        return CPU_DEFAULT_MODEL.to_string();
    }
    installed_models
        .first()
        .cloned()
        .unwrap_or_else(|| CPU_DEFAULT_MODEL.to_string())
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
    fn gpu_or_installed_allows_pick() {
        assert!(allow_model_pick(true, &[]));
        assert!(allow_model_pick(false, &["llama3:8b".into()]));
        assert_eq!(
            recommended_model(true, &["mistral:7b".into()]),
            "mistral:7b"
        );
    }
}
