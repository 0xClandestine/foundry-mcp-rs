//! Context injection for enhancing tool descriptions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Repository-local context configuration for enhancing descriptions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextConfig {
    #[serde(default)]
    pub tools: HashMap<String, String>,
    #[serde(default)]
    pub flags: HashMap<String, String>,
    #[serde(default)]
    pub positionals: HashMap<String, String>,
}

impl ContextConfig {
    /// Load context from file, falling back to empty config on error
    pub fn load() -> Self {
        Self::from_file("context.json").unwrap_or_else(|_| {
            eprintln!("ℹ No context.json found, descriptions will use defaults");
            Self::default()
        })
    }

    fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn tool_description(&self, name: &str, original: &str) -> String {
        self.tools
            .get(name)
            .map(|ctx| format!("{}\n\n{}", original, ctx))
            .unwrap_or_else(|| original.to_string())
    }

    pub fn flag_description(&self, name: &str, original: &str) -> String {
        self.flags
            .get(name)
            .map(|ctx| format!("{}\n\n{}", original, ctx))
            .unwrap_or_else(|| original.to_string())
    }

    pub fn positional_description(&self, name: &str, original: &str) -> String {
        self.positionals
            .get(name)
            .map(|ctx| format!("{}\n\n{}", original, ctx))
            .unwrap_or_else(|| original.to_string())
    }
}
