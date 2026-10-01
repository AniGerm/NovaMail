//! Linux .deb updater via GitHub Releases (same flow as Fax Inbox).
//!
//! Checks `AniGerm/NovaMail` latest release for a `.deb`, downloads it into
//! the app data dir, then quits and installs with `pkexec` + relaunch.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use novamail_ipc::{AppError, AppVersionInfo, UpdateStatusEvent};
use serde::Deserialize;
use tauri::{AppHandle, Emitter};

const OWNER: &str = "AniGerm";
const REPO: &str = "NovaMail";
const USER_AGENT: &str = "NovaMail-Updater";
const START_DELAY_MS: u64 = 10_000;
const INTERVAL_SECS: u64 = 4 * 60 * 60;
const INSTALL_DELAY_MS: u64 = 900;

#[derive(Debug, Clone)]
struct DebReleaseInfo {
    version: String,
    deb_url: String,
    deb_name: String,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[allow(dead_code)]
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Default)]
struct PendingUpdate {
    info: Option<DebReleaseInfo>,
    downloaded_path: Option<PathBuf>,
    install_in_flight: bool,
}

static PENDING: Mutex<PendingUpdate> = Mutex::new(PendingUpdate {
    info: None,
    downloaded_path: None,
    install_in_flight: false,
});

fn emit(app: &AppHandle, event: UpdateStatusEvent) {
    let _ = app.emit("update://status", &event);
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Compare dotted versions; true if remote > local.
pub fn is_version_newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.trim()
            .trim_start_matches(['v', 'V'])
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect()
    };
    let a = parse(remote);
    let b = parse(local);
    let len = a.len().max(b.len());
    for i in 0..len {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x > y {
            return true;
        }
        if x < y {
            return false;
        }
    }
    false
}

fn is_appimage() -> bool {
    std::env::var_os("APPIMAGE").is_some()
}

/// Packaged Linux install via .deb / system package (not AppImage / cargo run).
pub fn use_linux_deb_updater() -> bool {
    if !cfg!(target_os = "linux") || is_appimage() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let path = exe.to_string_lossy();
    path.starts_with("/usr/")
        || path.contains("/.local/share/")
        || (!cfg!(debug_assertions) && !path.contains("/target/"))
}

pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub fn app_version_info() -> AppVersionInfo {
    let channel = if is_appimage() {
        "appimage"
    } else if use_linux_deb_updater() {
        "deb"
    } else if cfg!(debug_assertions) {
        "dev"
    } else {
        "other"
    };
    AppVersionInfo {
        version: current_version(),
        packaged: use_linux_deb_updater() || is_appimage(),
        channel: channel.to_string(),
    }
}

fn updates_dir() -> Result<PathBuf, AppError> {
    let base = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("novamail")
        .join("updates");
    fs::create_dir_all(&base).map_err(|e| {
        AppError::new("update_dir", format!("Could not create updates dir: {e}"))
    })?;
    Ok(base)
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

fn runtime_uid() -> u32 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(1000)
}

async fn fetch_latest_deb_release() -> Result<Option<DebReleaseInfo>, AppError> {
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::new("http_client", e.to_string()))?;
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| AppError::new("github_api", e.to_string()))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(AppError::new(
            "github_api",
            format!("GitHub API HTTP {}", response.status()),
        ));
    }
    let release: GithubRelease = response
        .json()
        .await
        .map_err(|e| AppError::new("github_parse", e.to_string()))?;
    let version = release.tag_name.trim_start_matches(['v', 'V']).to_string();

    let mut preferred = None;
    let mut any_deb = None;
    for a in release.assets {
        let lower = a.name.to_lowercase();
        if !lower.ends_with(".deb") {
            continue;
        }
        if preferred.is_none() && (lower.contains("novamail") || lower.contains("nova-mail")) {
            preferred = Some(a);
        } else if any_deb.is_none() {
            any_deb = Some(a);
        }
    }
    let Some(asset) = preferred.or(any_deb) else {
        return Ok(None);
    };
    Ok(Some(DebReleaseInfo {
        version,
        deb_url: asset.browser_download_url,
        deb_name: asset.name,
    }))
}

async fn download_file(url: &str, dest: &Path, app: &AppHandle) -> Result<(), AppError> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| AppError::new("http_client", e.to_string()))?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::new("download", e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::new("download", e.to_string()))?;
    let total = response.content_length().unwrap_or(0);
    let mut file = File::create(dest).map_err(|e| {
        AppError::new(
            "download",
            format!("Could not create {}: {e}", dest.display()),
        )
    })?;
    let mut received: u64 = 0;
    let mut last_pct: u32 = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::new("download", e.to_string()))?
    {
        file.write_all(&chunk)
            .map_err(|e| AppError::new("download", e.to_string()))?;
        received += chunk.len() as u64;
        if total > 0 {
            let pct = ((received * 100) / total) as u32;
            if pct != last_pct {
                last_pct = pct;
                emit(
                    app,
                    UpdateStatusEvent::DownloadProgress {
                        percent: pct.min(100),
                    },
                );
            }
        }
    }
    file.flush()
        .map_err(|e| AppError::new("download", e.to_string()))?;
    Ok(())
}

fn quit_and_install_deb(app: &AppHandle, deb_path: &Path) -> Result<(), AppError> {
    let q_deb = shell_single_quote(&deb_path.to_string_lossy());
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
    let xdg = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", runtime_uid()));
    let home = std::env::var("HOME").unwrap_or_default();
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default();
    let q_display = shell_single_quote(&display);
    let q_xdg = shell_single_quote(&xdg);
    let q_home = shell_single_quote(&home);
    let q_user = shell_single_quote(&user);
    let dbus = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .map(|v| format!("export DBUS_SESSION_BUS_ADDRESS={}", shell_single_quote(&v)))
        .unwrap_or_default();
    let wayland = std::env::var("WAYLAND_DISPLAY")
        .map(|v| format!("export WAYLAND_DISPLAY={}", shell_single_quote(&v)))
        .unwrap_or_default();

    let script = format!(
        r#"#!/bin/bash
# Keep running after NovaMail exits
trap '' HUP
exec >>/tmp/novamail-update.log 2>&1
echo "==== $(date -Is) update install start ===="
set -x

DEB={q_deb}
export DISPLAY={q_display}
export XDG_RUNTIME_DIR={q_xdg}
export HOME={q_home}
export USER={q_user}
export LOGNAME={q_user}
{dbus}
{wayland}

# Install (shows password dialog). Do not use set -e — we always try to relaunch.
if command -v apt-get >/dev/null 2>&1; then
  pkexec env DEBIAN_FRONTEND=noninteractive apt-get install -y --reinstall "$DEB" \
    || pkexec dpkg -i "$DEB" \
    || true
else
  pkexec dpkg -i "$DEB" || true
fi

sleep 2

relaunch() {{
  if command -v gio >/dev/null 2>&1; then
    for desk in /usr/share/applications/novamail.desktop \
                /usr/share/applications/NovaMail.desktop \
                /usr/local/share/applications/novamail.desktop; do
      if [ -f "$desk" ]; then
        gio launch "$desk" && return 0
      fi
    done
  fi
  if command -v gtk-launch >/dev/null 2>&1; then
    gtk-launch novamail && return 0
    gtk-launch NovaMail && return 0
  fi
  for bin in /usr/bin/novamail /usr/bin/novamail-desktop; do
    if [ -x "$bin" ]; then
      nohup "$bin" >/tmp/novamail-relaunch.log 2>&1 &
      disown || true
      return 0
    fi
  done
  if command -v novamail >/dev/null 2>&1; then
    nohup novamail >/tmp/novamail-relaunch.log 2>&1 &
    disown || true
    return 0
  fi
  return 1
}}

for _try in 1 2 3 4 5; do
  if relaunch; then
    echo "relaunch ok (try $_try)"
    exit 0
  fi
  sleep 1
done

echo "relaunch failed"
exit 1
"#
    );

    let script_path = temp_dir().join("novamail-install-update.sh");
    fs::write(&script_path, script.as_bytes()).map_err(|e| {
        AppError::new("install", format!("Could not write install script: {e}"))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)
            .map_err(|e| AppError::new("install", e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)
            .map_err(|e| AppError::new("install", e.to_string()))?;
    }

    let child = Command::new("setsid")
        .args(["bash", &script_path.to_string_lossy()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::new("install", format!("setsid failed: {e}")))?;
    drop(child);

    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        handle.exit(0);
    });
    Ok(())
}

pub async fn check_for_updates(
    app: AppHandle,
) -> Result<novamail_ipc::UpdateCheckResult, AppError> {
    emit(&app, UpdateStatusEvent::Checking);
    match fetch_latest_deb_release().await {
        Ok(Some(latest)) => {
            let current = current_version();
            if is_version_newer(&latest.version, &current) {
                if let Ok(mut pending) = PENDING.lock() {
                    pending.info = Some(latest.clone());
                    pending.downloaded_path = None;
                }
                emit(
                    &app,
                    UpdateStatusEvent::UpdateAvailable {
                        version: latest.version.clone(),
                    },
                );
                Ok(novamail_ipc::UpdateCheckResult {
                    ok: true,
                    available_version: Some(latest.version),
                })
            } else {
                if let Ok(mut pending) = PENDING.lock() {
                    pending.info = None;
                    pending.downloaded_path = None;
                }
                emit(
                    &app,
                    UpdateStatusEvent::UpdateNotAvailable {
                        version: current,
                    },
                );
                Ok(novamail_ipc::UpdateCheckResult {
                    ok: true,
                    available_version: None,
                })
            }
        }
        Ok(None) => {
            emit(
                &app,
                UpdateStatusEvent::Error {
                    message: "Kein .deb-Paket im neuesten GitHub-Release gefunden.".into(),
                },
            );
            Ok(novamail_ipc::UpdateCheckResult {
                ok: false,
                available_version: None,
            })
        }
        Err(err) => {
            emit(
                &app,
                UpdateStatusEvent::Error {
                    message: err.message.clone(),
                },
            );
            Err(err)
        }
    }
}

pub async fn download_update(
    app: AppHandle,
) -> Result<novamail_ipc::UpdateActionResult, AppError> {
    let info = {
        let pending = PENDING
            .lock()
            .map_err(|_| AppError::new("lock", "Update state locked"))?;
        pending.info.clone()
    };
    let Some(info) = info else {
        emit(
            &app,
            UpdateStatusEvent::Error {
                message: "Kein Update zum Herunterladen.".into(),
            },
        );
        return Ok(novamail_ipc::UpdateActionResult {
            ok: false,
            reason: Some("no_pending".into()),
        });
    };
    emit(&app, UpdateStatusEvent::DownloadProgress { percent: 0 });
    let dest = updates_dir()?.join(&info.deb_name);
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    match download_file(&info.deb_url, &dest, &app).await {
        Ok(()) => {
            if let Ok(mut pending) = PENDING.lock() {
                pending.downloaded_path = Some(dest);
            }
            emit(
                &app,
                UpdateStatusEvent::UpdateDownloaded {
                    version: info.version,
                },
            );
            Ok(novamail_ipc::UpdateActionResult {
                ok: true,
                reason: None,
            })
        }
        Err(err) => {
            emit(
                &app,
                UpdateStatusEvent::Error {
                    message: err.message.clone(),
                },
            );
            Err(err)
        }
    }
}

pub async fn install_update(app: AppHandle) -> Result<novamail_ipc::UpdateActionResult, AppError> {
    {
        let mut pending = PENDING
            .lock()
            .map_err(|_| AppError::new("lock", "Update state locked"))?;
        if pending.install_in_flight {
            return Ok(novamail_ipc::UpdateActionResult {
                ok: true,
                reason: None,
            });
        }
        if pending.downloaded_path.is_none() {
            emit(
                &app,
                UpdateStatusEvent::Error {
                    message: "Update noch nicht heruntergeladen.".into(),
                },
            );
            return Ok(novamail_ipc::UpdateActionResult {
                ok: false,
                reason: Some("not_downloaded".into()),
            });
        }
        pending.install_in_flight = true;
    }

    let (version, path) = {
        let pending = PENDING.lock().unwrap();
        (
            pending
                .info
                .as_ref()
                .map(|i| i.version.clone())
                .unwrap_or_else(current_version),
            pending.downloaded_path.clone().unwrap(),
        )
    };

    emit(
        &app,
        UpdateStatusEvent::Installing {
            version: version.clone(),
        },
    );

    tokio::time::sleep(Duration::from_millis(INSTALL_DELAY_MS)).await;

    match quit_and_install_deb(&app, &path) {
        Ok(()) => Ok(novamail_ipc::UpdateActionResult {
            ok: true,
            reason: None,
        }),
        Err(err) => {
            if let Ok(mut pending) = PENDING.lock() {
                pending.install_in_flight = false;
            }
            emit(
                &app,
                UpdateStatusEvent::Error {
                    message: err.message.clone(),
                },
            );
            Err(err)
        }
    }
}

/// Start background auto-check (optional; preference lives in the UI).
#[allow(dead_code)]
pub fn spawn_auto_checker(app: AppHandle) {
    if !use_linux_deb_updater() {
        tracing::info!("Updater: not a packaged .deb install — auto-check skipped");
        return;
    }
    tracing::info!("Updater: Linux .deb — auto-check enabled");
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(START_DELAY_MS)).await;
        loop {
            if let Err(err) = check_for_updates(app.clone()).await {
                tracing::warn!(error = %err.message, "auto update check failed");
            }
            tokio::time::sleep(Duration::from_secs(INTERVAL_SECS)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::is_version_newer;

    #[test]
    fn version_compare() {
        assert!(is_version_newer("0.1.1", "0.1.0"));
        assert!(is_version_newer("v0.2.0", "0.1.9"));
        assert!(!is_version_newer("0.1.0", "0.1.0"));
        assert!(!is_version_newer("0.1.0", "0.1.1"));
    }
}
