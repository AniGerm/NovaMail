//! Hunspell / Enchant dictionary discovery and user-level install.
//!
//! WebKitGTK spellcheck uses Enchant, which reads:
//! - system packs under `/usr/share/hunspell`
//! - user packs under `~/.config/enchant/hunspell`
//!
//! NovaMail downloads LibreOffice dictionaries into the user folder so
//! languages can be added without root / apt.

use std::fs;
use std::path::{Path, PathBuf};

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use novamail_ipc::{SpellDictionaryDto, SpellSuggestResult, SpellcheckStatus};
use tracing::info;

use crate::{CoreError, CoreResult};

struct DictCache {
    code: String,
    words: HashSet<String>,
    /// lowercase → original casing from dictionary
    lower_index: HashMap<String, String>,
}

static DICT_CACHE: OnceLock<Mutex<Option<DictCache>>> = OnceLock::new();

const LO_RAW: &str =
    "https://raw.githubusercontent.com/LibreOffice/dictionaries/master";

struct CatalogEntry {
    code: &'static str,
    name: &'static str,
    /// Path under the LibreOffice dictionaries repo (without host).
    aff_path: &'static str,
    dic_path: &'static str,
    /// Optional Debian/Ubuntu package hint (for packaging / docs).
    #[allow(dead_code)]
    apt_package: &'static str,
}

const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        code: "de_DE",
        name: "Deutsch (Deutschland)",
        aff_path: "de/de_DE_frami.aff",
        dic_path: "de/de_DE_frami.dic",
        apt_package: "hunspell-de-de",
    },
    CatalogEntry {
        code: "en_US",
        name: "English (US)",
        aff_path: "en/en_US.aff",
        dic_path: "en/en_US.dic",
        apt_package: "hunspell-en-us",
    },
    CatalogEntry {
        code: "en_GB",
        name: "English (UK)",
        aff_path: "en/en_GB.aff",
        dic_path: "en/en_GB.dic",
        apt_package: "hunspell-en-gb",
    },
    CatalogEntry {
        code: "fr_FR",
        name: "Français",
        aff_path: "fr_FR/dictionaries/fr.aff",
        dic_path: "fr_FR/dictionaries/fr.dic",
        apt_package: "hunspell-fr",
    },
    CatalogEntry {
        code: "es_ES",
        name: "Español",
        aff_path: "es/es_ES.aff",
        dic_path: "es/es_ES.dic",
        apt_package: "hunspell-es",
    },
    CatalogEntry {
        code: "it_IT",
        name: "Italiano",
        aff_path: "it_IT/it_IT.aff",
        dic_path: "it_IT/it_IT.dic",
        apt_package: "hunspell-it",
    },
    CatalogEntry {
        code: "nl_NL",
        name: "Nederlands",
        aff_path: "nl_NL/nl_NL.aff",
        dic_path: "nl_NL/nl_NL.dic",
        apt_package: "hunspell-nl",
    },
    CatalogEntry {
        code: "pt_BR",
        name: "Português (Brasil)",
        aff_path: "pt_BR/pt_BR.aff",
        dic_path: "pt_BR/pt_BR.dic",
        apt_package: "hunspell-pt-br",
    },
    CatalogEntry {
        code: "pl_PL",
        name: "Polski",
        aff_path: "pl_PL/pl_PL.aff",
        dic_path: "pl_PL/pl_PL.dic",
        apt_package: "hunspell-pl",
    },
    CatalogEntry {
        code: "sv_SE",
        name: "Svenska",
        aff_path: "sv_SE/dictionaries/sv_SE.aff",
        dic_path: "sv_SE/dictionaries/sv_SE.dic",
        apt_package: "hunspell-sv",
    },
    CatalogEntry {
        code: "da_DK",
        name: "Dansk",
        aff_path: "da_DK/da_DK.aff",
        dic_path: "da_DK/da_DK.dic",
        apt_package: "hunspell-da",
    },
    CatalogEntry {
        code: "cs_CZ",
        name: "Čeština",
        aff_path: "cs_CZ/cs_CZ.aff",
        dic_path: "cs_CZ/cs_CZ.dic",
        apt_package: "hunspell-cs",
    },
    CatalogEntry {
        code: "ru_RU",
        name: "Русский",
        aff_path: "ru_RU/ru_RU.aff",
        dic_path: "ru_RU/ru_RU.dic",
        apt_package: "hunspell-ru",
    },
];

fn user_hunspell_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("enchant")
        .join("hunspell")
}

fn user_enchant_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("enchant")
}

/// Enchant personal word list path (`~/.config/enchant/<lang>.dic`).
fn personal_dict_path(code: &str) -> PathBuf {
    user_enchant_dir().join(format!("{code}.dic"))
}

fn normalize_lang_code(lang: &str) -> String {
    if lang.contains('_') {
        lang.to_string()
    } else if lang == "de" {
        "de_DE".into()
    } else if lang == "en" {
        "en_US".into()
    } else {
        lang.to_string()
    }
}

/// Ensure Enchant personal dictionaries exist so WebKit "Learn" can write.
pub fn ensure_personal_dictionaries(languages: &[String]) -> CoreResult<()> {
    let dir = user_enchant_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| CoreError::Message(format!("cannot create enchant dir: {e}")))?;
    // Also ensure hunspell dir exists for installed packs.
    fs::create_dir_all(user_hunspell_dir())
        .map_err(|e| CoreError::Message(format!("cannot create hunspell dir: {e}")))?;
    let mut codes = HashSet::new();
    for lang in languages {
        let code = normalize_lang_code(lang);
        codes.insert(code.clone());
        let short = code.split('_').next().unwrap_or(&code).to_string();
        if short != code {
            codes.insert(short);
        }
    }
    for code in codes {
        let path = personal_dict_path(&code);
        if !path.exists() {
            fs::write(&path, b"").map_err(|e| {
                CoreError::Message(format!("cannot create personal dict {}: {e}", path.display()))
            })?;
            info!(path = %path.display(), "created Enchant personal dictionary");
        }
        // Make sure the file is writable (Learn is disabled when PWL is read-only).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&path) {
                let mut perms = meta.permissions();
                let mode = perms.mode();
                if mode & 0o200 == 0 {
                    perms.set_mode(mode | 0o600);
                    let _ = fs::set_permissions(&path, perms);
                }
            }
        }
    }
    Ok(())
}

/// Add a word to the Enchant personal dictionary and in-memory cache.
pub fn spellcheck_learn_word(word: &str, lang: &str) -> CoreResult<()> {
    let trimmed = word.trim();
    if trimmed.is_empty() || trimmed.chars().any(|c| c.is_whitespace()) {
        return Err(CoreError::Message("invalid word for dictionary".into()));
    }
    let code = normalize_lang_code(lang);
    ensure_personal_dictionaries(&[code.clone()])?;
    let path = personal_dict_path(&code);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let already = existing.lines().any(|line| line.trim() == trimmed);
    if !already {
        let mut out = existing;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(trimmed);
        out.push('\n');
        fs::write(&path, out)
            .map_err(|e| CoreError::Message(format!("write personal dict failed: {e}")))?;
    }
    // Keep short-code PWL in sync (some Enchant backends look up `de.dic`).
    let short = code.split('_').next().unwrap_or(&code);
    if short != code {
        let short_path = personal_dict_path(short);
        let short_existing = fs::read_to_string(&short_path).unwrap_or_default();
        if !short_existing.lines().any(|line| line.trim() == trimmed) {
            let mut out = short_existing;
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(trimmed);
            out.push('\n');
            let _ = fs::write(&short_path, out);
        }
    }
    // Invalidate cache so suggest/correct pick up the new word.
    if let Some(lock) = DICT_CACHE.get() {
        if let Ok(mut guard) = lock.lock() {
            *guard = None;
        }
    }
    info!(word = %trimmed, code = %code, "learned spelling word");
    Ok(())
}

fn load_personal_words(code: &str) -> HashSet<String> {
    let mut words = HashSet::new();
    for path in [
        personal_dict_path(code),
        personal_dict_path(code.split('_').next().unwrap_or(code)),
    ] {
        if let Ok(raw) = fs::read_to_string(&path) {
            for line in raw.lines() {
                let w = line.trim();
                if !w.is_empty() && w.len() <= 48 {
                    words.insert(w.to_string());
                }
            }
        }
    }
    words
}

fn system_hunspell_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/hunspell"),
        PathBuf::from("/usr/share/myspell/dicts"),
        PathBuf::from("/usr/share/myspell"),
    ]
}

fn dict_present_in(dir: &Path, code: &str) -> bool {
    dir.join(format!("{code}.dic")).is_file() && dir.join(format!("{code}.aff")).is_file()
}

fn find_install_source(code: &str) -> Option<&'static str> {
    let user = user_hunspell_dir();
    if dict_present_in(&user, code) {
        return Some("user");
    }
    for dir in system_hunspell_dirs() {
        if dict_present_in(&dir, code) {
            return Some("system");
        }
    }
    // Some packs only ship language code without region (e.g. `de.dic`).
    let lang = code.split('_').next().unwrap_or(code);
    if lang != code {
        if dict_present_in(&user, lang) {
            return Some("user");
        }
        for dir in system_hunspell_dirs() {
            if dict_present_in(&dir, lang) {
                return Some("system");
            }
        }
    }
    None
}

pub fn spellcheck_status() -> CoreResult<SpellcheckStatus> {
    let user_dict_dir = user_hunspell_dir();
    let dictionaries = CATALOG
        .iter()
        .map(|entry| {
            let source = find_install_source(entry.code).unwrap_or("");
            SpellDictionaryDto {
                code: entry.code.to_string(),
                name: entry.name.to_string(),
                installed: !source.is_empty(),
                source: source.to_string(),
            }
        })
        .collect();
    Ok(SpellcheckStatus {
        dictionaries,
        user_dict_dir: user_dict_dir.display().to_string(),
    })
}

pub async fn spellcheck_install(code: &str) -> CoreResult<SpellDictionaryDto> {
    let entry = CATALOG
        .iter()
        .find(|e| e.code == code)
        .ok_or_else(|| CoreError::Message(format!("unknown spellcheck language: {code}")))?;

    if let Some(source) = find_install_source(entry.code) {
        return Ok(SpellDictionaryDto {
            code: entry.code.to_string(),
            name: entry.name.to_string(),
            installed: true,
            source: source.to_string(),
        });
    }

    let dest = user_hunspell_dir();
    fs::create_dir_all(&dest)
        .map_err(|e| CoreError::Message(format!("cannot create dictionary dir: {e}")))?;

    let aff_url = format!("{LO_RAW}/{}", entry.aff_path);
    let dic_url = format!("{LO_RAW}/{}", entry.dic_path);
    let aff_bytes = download(&aff_url).await?;
    let dic_bytes = download(&dic_url).await?;

    let aff_path = dest.join(format!("{}.aff", entry.code));
    let dic_path = dest.join(format!("{}.dic", entry.code));
    fs::write(&aff_path, &aff_bytes)
        .map_err(|e| CoreError::Message(format!("write {}: {e}", aff_path.display())))?;
    fs::write(&dic_path, &dic_bytes)
        .map_err(|e| CoreError::Message(format!("write {}: {e}", dic_path.display())))?;

    info!(code = entry.code, path = %dest.display(), "installed spellcheck dictionary");

    Ok(SpellDictionaryDto {
        code: entry.code.to_string(),
        name: entry.name.to_string(),
        installed: true,
        source: "user".into(),
    })
}

/// Ensure the dictionary for a UI locale (`de` / `en`) is present.
pub async fn spellcheck_ensure_for_locale(locale: &str) -> CoreResult<SpellDictionaryDto> {
    let code = match locale {
        "de" => "de_DE",
        "en" => "en_US",
        other if other.contains('_') => other,
        other => other,
    };
    spellcheck_install(code).await
}

async fn download(url: &str) -> CoreResult<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent("NovaMail/0.1 (spellcheck dictionary install)")
        .build()
        .map_err(|e| CoreError::Message(e.to_string()))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| CoreError::Message(format!("download failed: {e}")))?;
    if !response.status().is_success() {
        return Err(CoreError::Message(format!(
            "download {url} failed: HTTP {}",
            response.status()
        )));
    }
    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| CoreError::Message(format!("download body failed: {e}")))
}

fn resolve_dic_path(code: &str) -> Option<PathBuf> {
    let user = user_hunspell_dir();
    let candidates = [
        user.join(format!("{code}.dic")),
        user.join(format!("{}.dic", code.split('_').next().unwrap_or(code))),
    ];
    for path in candidates {
        if path.is_file() {
            return Some(path);
        }
    }
    for dir in system_hunspell_dirs() {
        let full = dir.join(format!("{code}.dic"));
        if full.is_file() {
            return Some(full);
        }
        let short = dir.join(format!("{}.dic", code.split('_').next().unwrap_or(code)));
        if short.is_file() {
            return Some(short);
        }
    }
    None
}

fn load_dict(code: &str) -> CoreResult<DictCache> {
    let path = resolve_dic_path(code).ok_or_else(|| {
        CoreError::Message(format!("no dictionary installed for {code}"))
    })?;
    let raw = fs::read_to_string(&path)
        .map_err(|e| CoreError::Message(format!("read {}: {e}", path.display())))?;
    let mut words = HashSet::new();
    let mut lower_index = HashMap::new();
    for (i, line) in raw.lines().enumerate() {
        if i == 0 && line.chars().all(|c| c.is_ascii_digit()) {
            continue; // word count header
        }
        let word = line.split('/').next().unwrap_or("").trim();
        if word.is_empty() || word.len() > 48 {
            continue;
        }
        words.insert(word.to_string());
        lower_index
            .entry(word.to_lowercase())
            .or_insert_with(|| word.to_string());
    }
    for word in load_personal_words(code) {
        words.insert(word.clone());
        lower_index
            .entry(word.to_lowercase())
            .or_insert(word);
    }
    Ok(DictCache {
        code: code.to_string(),
        words,
        lower_index,
    })
}

fn with_dict<R>(code: &str, f: impl FnOnce(&DictCache) -> R) -> CoreResult<R> {
    let lock = DICT_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = lock
        .lock()
        .map_err(|_| CoreError::Message("spellcheck cache lock poisoned".into()))?;
    let needs_load = guard
        .as_ref()
        .map(|c| c.code != code)
        .unwrap_or(true);
    if needs_load {
        *guard = Some(load_dict(code)?);
    }
    let cache = guard.as_ref().expect("dict just loaded");
    Ok(f(cache))
}

fn edit_distance_le2(a: &str, b: &str) -> Option<u8> {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let (al, bl) = (ac.len(), bc.len());
    if al.abs_diff(bl) > 2 {
        return None;
    }
    // Banded DP for distances ≤ 2.
    let mut prev = (0..=bl).collect::<Vec<_>>();
    let mut cur = vec![0; bl + 1];
    for i in 1..=al {
        cur[0] = i;
        let mut row_min = cur[0];
        let j_start = i.saturating_sub(2);
        let j_end = (i + 2).min(bl);
        for j in 1..=bl {
            if j < j_start || j > j_end {
                cur[j] = 99;
                continue;
            }
            let cost = if ac[i - 1] == bc[j - 1] { 0 } else { 1 };
            let mut best = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1
                && j > 1
                && ac[i - 1] == bc[j - 2]
                && ac[i - 2] == bc[j - 1]
            {
                best = best.min(prev[j - 2] + 1); // approximate transpose via prev row
            }
            cur[j] = best;
            row_min = row_min.min(best);
        }
        if row_min > 2 {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let d = prev[bl];
    if d <= 2 {
        Some(d as u8)
    } else {
        None
    }
}

/// Check a word and return inline suggestions for the composer UI.
pub fn spellcheck_suggest(word: &str, lang: &str) -> CoreResult<SpellSuggestResult> {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return Ok(SpellSuggestResult {
            word: trimmed.into(),
            correct: true,
            suggestions: vec![],
            autocorrect: None,
        });
    }
    // Skip all-caps acronyms / numbers.
    if trimmed.chars().all(|c| !c.is_alphabetic())
        || (trimmed.len() <= 4 && trimmed.chars().all(|c| c.is_ascii_uppercase()))
    {
        return Ok(SpellSuggestResult {
            word: trimmed.into(),
            correct: true,
            suggestions: vec![],
            autocorrect: None,
        });
    }
    let code = normalize_lang_code(lang);

    with_dict(&code, |dict| {
        let lower = trimmed.to_lowercase();
        if dict.words.contains(trimmed) || dict.lower_index.contains_key(&lower) {
            return SpellSuggestResult {
                word: trimmed.into(),
                correct: true,
                suggestions: vec![],
                autocorrect: None,
            };
        }
        let prefix: String = lower.chars().take(2).collect();
        let mut scored: Vec<(u8, String)> = Vec::new();
        for (cand_lower, original) in &dict.lower_index {
            if cand_lower.len().abs_diff(lower.len()) > 2 {
                continue;
            }
            if !prefix.is_empty() && !cand_lower.starts_with(&prefix) {
                continue;
            }
            if let Some(d) = edit_distance_le2(&lower, cand_lower) {
                scored.push((d, original.clone()));
            }
        }
        scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.len().cmp(&b.1.len())));
        scored.dedup_by(|a, b| a.1.eq_ignore_ascii_case(&b.1));
        let best_distance = scored.first().map(|(d, _)| *d);
        let suggestions: Vec<String> = scored.into_iter().take(5).map(|(_, w)| w).collect();
        // Autocorrect only for high-confidence single-edit mistakes on longer words.
        let autocorrect = match (best_distance, suggestions.first()) {
            (Some(1), Some(best)) if trimmed.chars().count() >= 4 => Some(best.clone()),
            _ => None,
        };
        SpellSuggestResult {
            word: trimmed.into(),
            correct: false,
            suggestions,
            autocorrect,
        }
    })
}
