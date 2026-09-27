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

use novamail_ipc::{SpellDictionaryDto, SpellcheckStatus};
use tracing::info;

use crate::{CoreError, CoreResult};

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
