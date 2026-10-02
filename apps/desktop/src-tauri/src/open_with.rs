//! List and launch Linux “Open with…” handlers via `gio`.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenWithApp {
    /// Desktop file id, e.g. `org.gnome.Evince.desktop`.
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Apps registered for `mime`, with the default first when known.
pub fn list_apps_for_mime(mime: &str) -> Vec<OpenWithApp> {
    let mime = mime.trim();
    if mime.is_empty() {
        return Vec::new();
    }
    let Ok(output) = Command::new("gio").args(["mime", mime]).output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut default_id: Option<String> = None;
    let mut ids: Vec<String> = Vec::new();
    let mut in_registered = false;
    let mut in_recommended = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("Default application for")
            .and_then(|s| s.split(':').nth(1))
            .map(str::trim)
        {
            if !rest.is_empty() {
                default_id = Some(rest.to_string());
            }
            continue;
        }
        if trimmed.starts_with("Registered applications:") {
            in_registered = true;
            in_recommended = false;
            continue;
        }
        if trimmed.starts_with("Recommended applications:") {
            in_registered = false;
            in_recommended = true;
            continue;
        }
        if trimmed.is_empty() || !trimmed.contains(".desktop") {
            if !line.starts_with('\t') && !line.starts_with(' ') {
                in_registered = false;
                in_recommended = false;
            }
            continue;
        }
        if in_registered || in_recommended {
            let id = trimmed.to_string();
            if !ids.iter().any(|x| x == &id) {
                ids.push(id);
            }
        }
    }
    if let Some(def) = default_id.clone() {
        if !ids.iter().any(|x| x == &def) {
            ids.insert(0, def);
        } else {
            ids.retain(|x| Some(x) != default_id.as_ref());
            if let Some(def) = default_id.clone() {
                ids.insert(0, def);
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = BTreeMap::new();
    for id in ids {
        if seen.insert(id.clone(), ()).is_some() {
            continue;
        }
        let name = desktop_display_name(&id).unwrap_or_else(|| id.replace(".desktop", ""));
        let is_default = default_id.as_deref() == Some(id.as_str());
        out.push(OpenWithApp {
            id,
            name,
            is_default,
        });
    }
    out
}

pub fn open_path_with_app(path: &Path, desktop_id: &str) -> Result<(), String> {
    let id = desktop_id.trim();
    if id.is_empty() {
        return Err("missing application id".into());
    }
    let status = Command::new("gio")
        .arg("launch")
        .arg(id)
        .arg(path)
        .status()
        .map_err(|e| format!("gio launch failed: {e}"))?;
    if status.success() {
        return Ok(());
    }
    // Fallback: gtk-launch <id-without-.desktop> path
    let short = id.strip_suffix(".desktop").unwrap_or(id);
    let status = Command::new("gtk-launch")
        .arg(short)
        .arg(path)
        .status()
        .map_err(|e| format!("gtk-launch failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("could not launch {id}"))
    }
}

fn desktop_display_name(desktop_id: &str) -> Option<String> {
    let candidates = [
        dirs::data_dir().map(|d| d.join("applications").join(desktop_id)),
        Some(Path::new("/usr/share/applications").join(desktop_id)),
        Some(Path::new("/usr/local/share/applications").join(desktop_id)),
        Some(Path::new("/var/lib/flatpak/exports/share/applications").join(desktop_id)),
    ];
    for path in candidates.into_iter().flatten() {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut in_desktop = false;
        for line in raw.lines() {
            if line.trim() == "[Desktop Entry]" {
                in_desktop = true;
                continue;
            }
            if line.starts_with('[') {
                in_desktop = false;
                continue;
            }
            if in_desktop {
                if let Some(name) = line.strip_prefix("Name=") {
                    let name = name.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gio_mime_sample() {
        // Smoke: empty mime yields empty list without panicking.
        assert!(list_apps_for_mime("").is_empty());
    }
}
