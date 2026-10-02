use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpellDictionaryDto {
    /// IETF-ish tag used by Enchant / WebKit, e.g. `de_DE`.
    pub code: String,
    pub name: String,
    pub installed: bool,
    /// `system`, `user`, or empty when not installed.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpellcheckStatus {
    pub dictionaries: Vec<SpellDictionaryDto>,
    pub user_dict_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpellSuggestResult {
    pub word: String,
    pub correct: bool,
    pub suggestions: Vec<String>,
}
