//! Plugin runtime scaffold.
//!
//! ADR: plugins will execute as WASM with capability manifests. This crate
//! defines the host-facing types now so the UI and core can integrate early.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("capability denied: {0}")]
    CapabilityDenied(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub permissions: Vec<String>,
    pub description: String,
}

#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: Vec<PluginManifest>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> &[PluginManifest] {
        &self.plugins
    }

    pub fn register(&mut self, manifest: PluginManifest) {
        self.plugins.retain(|p| p.id != manifest.id);
        self.plugins.push(manifest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_plugin() {
        let mut registry = PluginRegistry::new();
        registry.register(PluginManifest {
            id: "demo".into(),
            name: "Demo".into(),
            version: "0.1.0".into(),
            permissions: vec!["mail:read".into()],
            description: "Demo plugin".into(),
        });
        assert_eq!(registry.list().len(), 1);
    }
}
