//! List and launch Linux “Open with…” handlers via desktop files / `gio`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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

/// Every visible installed app (not mime-filtered), sorted by name.
pub fn list_all_apps() -> Vec<OpenWithApp> {
    let mut by_id: BTreeMap<String, String> = BTreeMap::new();
    for dir in application_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(id) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
                continue;
            };
            if let Some(name) = parse_desktop_app_name(&path) {
                // Later dirs (user) override system names for the same id.
                by_id.insert(id, name);
            }
        }
    }
    let mut apps: Vec<OpenWithApp> = by_id
        .into_iter()
        .map(|(id, name)| OpenWithApp {
            id,
            name,
            is_default: false,
        })
        .collect();
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    apps
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

/// Show `path` in the file manager (select file when possible).
pub fn reveal_path(path: &Path) -> Result<(), String> {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let uri = path_to_file_uri(&abs);
    // Prefer Freedesktop FileManager1 so the file itself is selected.
    let status = Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--dest=org.freedesktop.FileManager1",
            "--type=method_call",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1.ShowItems",
            &format!("array:string:{uri}"),
            "string:",
        ])
        .status();
    if status.map(|s| s.success()).unwrap_or(false) {
        return Ok(());
    }
    // Fallback: open the parent folder that contains the staged file.
    let folder = abs.parent().unwrap_or(abs.as_path());
    open::that(folder).map_err(|e| format!("cannot open folder: {e}"))
}

fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data) = dirs::data_dir() {
        dirs.push(data.join("applications"));
    }
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs.push(PathBuf::from(
        "/var/lib/flatpak/exports/share/applications",
    ));
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for part in xdg.split(':') {
            if part.is_empty() {
                continue;
            }
            dirs.push(PathBuf::from(part).join("applications"));
        }
    }
    dirs
}

fn parse_desktop_app_name(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut in_desktop = false;
    let mut name: Option<String> = None;
    let mut app_type: Option<String> = None;
    let mut no_display = false;
    let mut hidden = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if trimmed == "[Desktop Entry]" {
                in_desktop = true;
            } else if in_desktop {
                break;
            }
            continue;
        }
        if !in_desktop {
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("Type=") {
            app_type = Some(v.trim().to_string());
        } else if let Some(v) = trimmed.strip_prefix("Name=") {
            if name.is_none() {
                let v = v.trim();
                if !v.is_empty() {
                    name = Some(v.to_string());
                }
            }
        } else if let Some(v) = trimmed.strip_prefix("NoDisplay=") {
            no_display = v.trim().eq_ignore_ascii_case("true");
        } else if let Some(v) = trimmed.strip_prefix("Hidden=") {
            hidden = v.trim().eq_ignore_ascii_case("true");
        }
    }
    if no_display || hidden {
        return None;
    }
    if app_type.as_deref().unwrap_or("Application") != "Application" {
        return None;
    }
    name.filter(|n| !n.is_empty())
}

fn path_to_file_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::RootDir => {}
            Component::Prefix(p) => {
                out.push_str(&percent_encode(&p.as_os_str().to_string_lossy()));
            }
            other => {
                out.push('/');
                out.push_str(&percent_encode(&other.as_os_str().to_string_lossy()));
            }
        }
    }
    if out == "file://" {
        out.push('/');
    }
    out
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_installed_apps() {
        let apps = list_all_apps();
        // CI / desktop images should have at least a few .desktop entries.
        assert!(
            !apps.is_empty(),
            "expected at least one installed application"
        );
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        assert_eq!(names, sorted);
    }

    #[test]
    fn file_uri_encodes_spaces() {
        let uri = path_to_file_uri(Path::new("/tmp/my file.pdf"));
        assert_eq!(uri, "file:///tmp/my%20file.pdf");
    }

    #[test]
    fn skips_hidden_desktop_entries() {
        let dir = tempfile_dir();
        let path = dir.join("hidden-app.desktop");
        std::fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Hidden App\nNoDisplay=true\n",
        )
        .unwrap();
        assert!(parse_desktop_app_name(&path).is_none());
        let path2 = dir.join("visible-app.desktop");
        std::fs::write(
            &path2,
            "[Desktop Entry]\nType=Application\nName=Visible App\n",
        )
        .unwrap();
        assert_eq!(
            parse_desktop_app_name(&path2).as_deref(),
            Some("Visible App")
        );
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "novamail-openwith-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
