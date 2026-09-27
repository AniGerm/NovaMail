//! Detect / install / start local Ollama for first-run setup.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::provider::{AiError, AiResult};
use crate::runtime::{default_ollama_url, list_ollama_models};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OllamaPresence {
    pub installed: bool,
    pub binary_path: Option<String>,
    pub reachable: bool,
    pub can_install_user: bool,
    pub can_install_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub status: String,
    pub done: bool,
}

fn which_ollama() -> Option<PathBuf> {
    if let Ok(path) = which_command("ollama") {
        return Some(path);
    }
    let candidates = [
        dirs_path("HOME").map(|h| h.join(".local/bin/ollama")),
        dirs_path("HOME").map(|h| h.join(".novamail/ollama/bin/ollama")),
        Some(PathBuf::from("/usr/local/bin/ollama")),
        Some(PathBuf::from("/usr/bin/ollama")),
        Some(PathBuf::from("/opt/homebrew/bin/ollama")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
}

fn dirs_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from)
}

fn which_command(name: &str) -> Result<PathBuf, ()> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(());
    }
    Ok(PathBuf::from(path))
}

fn pkexec_available() -> bool {
    which_command("pkexec").is_ok()
}

fn arch_tag() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("amd64"),
        "aarch64" => Some("arm64"),
        _ => None,
    }
}

pub fn can_install_user() -> bool {
    cfg!(target_os = "linux") && arch_tag().is_some()
}

pub fn can_install_system() -> bool {
    cfg!(target_os = "linux") && pkexec_available()
}

pub async fn probe_ollama(base_url: &str) -> OllamaPresence {
    let binary = which_ollama();
    let reachable = list_ollama_models(base_url).await.is_ok();
    OllamaPresence {
        installed: binary.is_some() || reachable,
        binary_path: binary.map(|p| p.display().to_string()),
        reachable,
        can_install_user: can_install_user(),
        can_install_system: can_install_system(),
    }
}

fn user_install_root() -> PathBuf {
    dirs_path("HOME")
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
}

/// Download Ollama into `~/.local` (no sudo). Linux only.
pub async fn install_ollama_user<F>(mut on_progress: F) -> AiResult<PathBuf>
where
    F: FnMut(InstallProgress) + Send,
{
    if !can_install_user() {
        return Err(AiError::Unavailable(
            "User-local Ollama install is only automated on Linux amd64/arm64".into(),
        ));
    }
    let arch = arch_tag().ok_or_else(|| {
        AiError::Unavailable("Unsupported CPU architecture for Ollama install".into())
    })?;
    let root = user_install_root();
    std::fs::create_dir_all(root.join("bin"))
        .map_err(|e| AiError::Inference(format!("mkdir failed: {e}")))?;

    on_progress(InstallProgress {
        status: "downloading".into(),
        done: false,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60 * 30))
        .build()
        .map_err(|e| AiError::Unavailable(e.to_string()))?;

    // Prefer .tgz (works with stock tar); fall back noted in error if both fail.
    let tgz_url = format!("https://ollama.com/download/ollama-linux-{arch}.tgz");
    let bytes = match client.get(&tgz_url).send().await {
        Ok(resp) if resp.status().is_success() => resp
            .bytes()
            .await
            .map_err(|e| AiError::Inference(e.to_string()))?,
        _ => {
            let zst_url = format!("https://ollama.com/download/ollama-linux-{arch}.tar.zst");
            let resp = client.get(&zst_url).send().await.map_err(|e| {
                AiError::Unavailable(format!("download failed: {e}"))
            })?;
            if !resp.status().is_success() {
                return Err(AiError::Unavailable(format!(
                    "Ollama download HTTP {}",
                    resp.status()
                )));
            }
            let zst = resp
                .bytes()
                .await
                .map_err(|e| AiError::Inference(e.to_string()))?;
            return extract_zst_archive(&zst, &root, &mut on_progress);
        }
    };

    on_progress(InstallProgress {
        status: "extracting".into(),
        done: false,
    });
    extract_tgz_archive(&bytes, &root)?;

    let binary = root.join("bin/ollama");
    if !binary.is_file() {
        return Err(AiError::Inference(
            "Ollama archive extracted, but bin/ollama was not found".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&binary)
            .map_err(|e| AiError::Inference(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&binary, perms)
            .map_err(|e| AiError::Inference(e.to_string()))?;
    }

    on_progress(InstallProgress {
        status: "installed".into(),
        done: false,
    });
    Ok(binary)
}

fn extract_tgz_archive(bytes: &[u8], root: &Path) -> AiResult<()> {
    let tmp = std::env::temp_dir().join(format!("novamail-ollama-{}.tgz", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| AiError::Inference(e.to_string()))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&tmp)
        .arg("-C")
        .arg(root)
        .status()
        .map_err(|e| AiError::Inference(format!("tar failed: {e}")))?;
    let _ = std::fs::remove_file(&tmp);
    if !status.success() {
        return Err(AiError::Inference("tar extraction failed".into()));
    }
    Ok(())
}

fn extract_zst_archive<F>(bytes: &[u8], root: &Path, on_progress: &mut F) -> AiResult<PathBuf>
where
    F: FnMut(InstallProgress) + Send,
{
    on_progress(InstallProgress {
        status: "extracting".into(),
        done: false,
    });
    let tmp = std::env::temp_dir().join(format!("novamail-ollama-{}.tar.zst", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| AiError::Inference(e.to_string()))?;

    // GNU tar 1.31+ supports --zstd; otherwise pipe through zstd/unzstd.
    let status = if Command::new("tar")
        .arg("--help")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("--zstd"))
        .unwrap_or(false)
    {
        Command::new("tar")
            .args(["--zstd", "-xf"])
            .arg(&tmp)
            .arg("-C")
            .arg(root)
            .status()
            .map_err(|e| AiError::Inference(format!("tar failed: {e}")))?
    } else if which_command("zstd").is_ok() || which_command("unzstd").is_ok() {
        let zstd_bin = if which_command("zstd").is_ok() {
            "zstd"
        } else {
            "unzstd"
        };
        let mut decoder = Command::new(zstd_bin)
            .args(if zstd_bin == "zstd" {
                vec!["-d", "-c"]
            } else {
                vec!["-c"]
            })
            .arg(&tmp)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| AiError::Inference(format!("{zstd_bin} failed: {e}")))?;
        let stdout = decoder.stdout.take().ok_or_else(|| {
            AiError::Inference("failed to pipe zstd output".into())
        })?;
        let tar_status = Command::new("tar")
            .args(["-xf", "-", "-C"])
            .arg(root)
            .stdin(stdout)
            .status()
            .map_err(|e| AiError::Inference(format!("tar failed: {e}")))?;
        let _ = decoder.wait();
        tar_status
    } else {
        let _ = std::fs::remove_file(&tmp);
        return Err(AiError::Unavailable(
            "Need tar --zstd or the zstd tool to extract Ollama".into(),
        ));
    };
    let _ = std::fs::remove_file(&tmp);
    if !status.success() {
        return Err(AiError::Inference("archive extraction failed".into()));
    }
    let binary = root.join("bin/ollama");
    if !binary.is_file() {
        return Err(AiError::Inference(
            "Ollama archive extracted, but bin/ollama was not found".into(),
        ));
    }
    Ok(binary)
}

/// System-wide install via official script; OS prompts for admin password (pkexec).
pub fn install_ollama_system<F>(mut on_progress: F) -> AiResult<()>
where
    F: FnMut(InstallProgress) + Send,
{
    if !can_install_system() {
        return Err(AiError::Unavailable(
            "System install needs Linux with pkexec (Polkit)".into(),
        ));
    }
    on_progress(InstallProgress {
        status: "waiting_for_admin".into(),
        done: false,
    });
    let status = Command::new("pkexec")
        .args([
            "bash",
            "-c",
            "curl -fsSL https://ollama.com/install.sh | sh",
        ])
        .status()
        .map_err(|e| AiError::Unavailable(format!("pkexec failed: {e}")))?;
    if !status.success() {
        return Err(AiError::Inference(
            "System Ollama install was cancelled or failed".into(),
        ));
    }
    on_progress(InstallProgress {
        status: "installed".into(),
        done: false,
    });
    Ok(())
}

/// Start `ollama serve` in the background if the API is not already up.
pub async fn ensure_ollama_running(base_url: &str) -> AiResult<String> {
    if list_ollama_models(base_url).await.is_ok() {
        return Ok(base_url.to_string());
    }
    let binary = which_ollama().ok_or_else(|| {
        AiError::Unavailable("Ollama binary not found. Install it first.".into())
    })?;
    Command::new(&binary)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AiError::Unavailable(format!("failed to start ollama serve: {e}")))?;

    let url = if base_url.trim().is_empty() {
        default_ollama_url()
    } else {
        base_url.to_string()
    };
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if list_ollama_models(&url).await.is_ok() {
            return Ok(url);
        }
    }
    Err(AiError::Unavailable(
        "Ollama started but API did not become ready in time".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_tag_known_on_ci() {
        // CI runners are amd64 or arm64; unknown arches return None.
        let _ = arch_tag();
        assert!(can_install_user() || cfg!(not(target_os = "linux")));
    }
}
