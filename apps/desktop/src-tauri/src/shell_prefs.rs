//! Desktop shell preferences: close-to-tray and Linux autostart.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellPrefs {
    pub close_to_tray: bool,
    pub autostart: bool,
    /// UI locale (`de` / `en`) — used to localize WebKit/GTK context menus.
    #[serde(default = "default_ui_locale")]
    pub ui_locale: String,
}

fn default_ui_locale() -> String {
    "de".into()
}

impl Default for ShellPrefs {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            autostart: false,
            ui_locale: default_ui_locale(),
        }
    }
}

fn prefs_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("novamail")
        .join("shell-prefs.json")
}

fn autostart_desktop_path() -> Option<PathBuf> {
    dirs::config_dir().map(|c| c.join("autostart").join("novamail.desktop"))
}

pub fn load() -> ShellPrefs {
    let path = prefs_path();
    let mut prefs: ShellPrefs = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    // Autostart is source-of-truth on disk for the .desktop file.
    prefs.autostart = autostart_enabled();
    CLOSE_TO_TRAY.store(prefs.close_to_tray, Ordering::Relaxed);
    prefs
}

pub fn save(prefs: &ShellPrefs) -> Result<(), String> {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())?;
    CLOSE_TO_TRAY.store(prefs.close_to_tray, Ordering::Relaxed);
    Ok(())
}

pub fn close_to_tray_enabled() -> bool {
    CLOSE_TO_TRAY.load(Ordering::Relaxed)
}

pub fn set_close_to_tray(enabled: bool) -> Result<ShellPrefs, String> {
    let mut prefs = load();
    prefs.close_to_tray = enabled;
    save(&prefs)?;
    Ok(prefs)
}

pub fn set_autostart(enabled: bool) -> Result<ShellPrefs, String> {
    if enabled {
        enable_autostart()?;
    } else {
        disable_autostart()?;
    }
    let mut prefs = load();
    prefs.autostart = enabled;
    save(&prefs)?;
    Ok(prefs)
}

pub fn set_ui_locale(locale: &str) -> Result<ShellPrefs, String> {
    let mut prefs = load();
    prefs.ui_locale = match locale {
        "en" | "en_US" | "en-US" => "en".into(),
        _ => "de".into(),
    };
    save(&prefs)?;
    Ok(prefs)
}

/// Apply gettext locale for WebKit/GTK menus before the toolkit initializes.
pub fn apply_process_locale() {
    let locale = load().ui_locale;
    let (lang, messages) = if locale.starts_with("en") {
        ("en_US:en", "en_US.UTF-8")
    } else {
        ("de_DE:de", "de_DE.UTF-8")
    };
    // Prefer LANGUAGE for gettext lookups without forcing the whole process locale.
    std::env::set_var("LANGUAGE", lang);
    if std::env::var_os("LC_MESSAGES").is_none() {
        std::env::set_var("LC_MESSAGES", messages);
    }
}

fn autostart_enabled() -> bool {
    autostart_desktop_path()
        .map(|p| p.is_file())
        .unwrap_or(false)
}

fn resolve_exec() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if exe.is_file() {
            return exe.display().to_string();
        }
    }
    for candidate in ["/usr/bin/novamail", "/usr/local/bin/novamail"] {
        if PathBuf::from(candidate).is_file() {
            return candidate.to_string();
        }
    }
    "novamail".into()
}

fn enable_autostart() -> Result<(), String> {
    let Some(path) = autostart_desktop_path() else {
        return Err("could not resolve autostart directory".into());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let exec = resolve_exec();
    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=NovaMail\n\
         Comment=Local-first email client\n\
         Exec={exec}\n\
         Icon=novamail\n\
         Terminal=false\n\
         Categories=Office;Email;\n\
         X-GNOME-Autostart-enabled=true\n"
    );
    fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

fn disable_autostart() -> Result<(), String> {
    if let Some(path) = autostart_desktop_path() {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
