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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_context_is_empty() {
        let ctx = ContextConfig::default();
        assert!(ctx.tools.is_empty());
        assert!(ctx.flags.is_empty());
        assert!(ctx.positionals.is_empty());
    }

    #[test]
    fn test_tool_description_with_context() {
        let mut ctx = ContextConfig::default();
        ctx.tools.insert(
            "forge_build".to_string(),
            "📝 Local Context: This builds your smart contracts".to_string(),
        );

        let result = ctx.tool_description("forge_build", "Build the project");
        assert!(result.contains("Build the project"));
        assert!(result.contains("📝 Local Context: This builds your smart contracts"));
        assert!(result.contains("\n\n")); // Should have separator
    }

    #[test]
    fn test_tool_description_without_context() {
        let ctx = ContextConfig::default();
        let result = ctx.tool_description("forge_build", "Build the project");
        assert_eq!(result, "Build the project");
    }

    #[test]
    fn test_flag_description_with_context() {
        let mut ctx = ContextConfig::default();
        ctx.flags.insert(
            "rpc-url".to_string(),
            "Use our company RPC: https://rpc.example.com".to_string(),
        );

        let result = ctx.flag_description("rpc-url", "RPC endpoint URL");
        assert!(result.contains("RPC endpoint URL"));
        assert!(result.contains("Use our company RPC: https://rpc.example.com"));
    }

    #[test]
    fn test_flag_description_without_context() {
        let ctx = ContextConfig::default();
        let result = ctx.flag_description("rpc-url", "RPC endpoint URL");
        assert_eq!(result, "RPC endpoint URL");
    }

    #[test]
    fn test_positional_description_with_context() {
        let mut ctx = ContextConfig::default();
        ctx.positionals.insert(
            "contract".to_string(),
            "Deploy to our testnet first".to_string(),
        );

        let result = ctx.positional_description("contract", "Contract to deploy");
        assert!(result.contains("Contract to deploy"));
        assert!(result.contains("Deploy to our testnet first"));
    }

    #[test]
    fn test_positional_description_without_context() {
        let ctx = ContextConfig::default();
        let result = ctx.positional_description("contract", "Contract to deploy");
        assert_eq!(result, "Contract to deploy");
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        // Change to a temp directory where context.json doesn't exist
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let ctx = ContextConfig::load();

        // Should return default (empty) config
        assert!(ctx.tools.is_empty());
        assert!(ctx.flags.is_empty());
        assert!(ctx.positionals.is_empty());

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_from_file_valid_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_context.json");

        let json_content = r#"{
            "tools": {
                "forge_build": "Custom build context"
            },
            "flags": {
                "rpc-url": "Custom RPC context"
            },
            "positionals": {
                "address": "Custom address context"
            }
        }"#;

        fs::write(&file_path, json_content).unwrap();

        let ctx = ContextConfig::from_file(file_path.to_str().unwrap()).unwrap();
        assert_eq!(
            ctx.tools.get("forge_build").unwrap(),
            "Custom build context"
        );
        assert_eq!(ctx.flags.get("rpc-url").unwrap(), "Custom RPC context");
        assert_eq!(
            ctx.positionals.get("address").unwrap(),
            "Custom address context"
        );
    }

    #[test]
    fn test_from_file_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid.json");

        fs::write(&file_path, "not valid json {{{").unwrap();

        let result = ContextConfig::from_file(file_path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file_missing_file() {
        let result = ContextConfig::from_file("/nonexistent/path/context.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_context_preserves_original_when_no_injection() {
        let ctx = ContextConfig::default();

        let tool_desc = ctx.tool_description("some_tool", "Original tool description");
        let flag_desc = ctx.flag_description("some_flag", "Original flag description");
        let pos_desc = ctx.positional_description("some_arg", "Original arg description");

        // Should return original strings unchanged
        assert_eq!(tool_desc, "Original tool description");
        assert_eq!(flag_desc, "Original flag description");
        assert_eq!(pos_desc, "Original arg description");
    }

    #[test]
    fn test_context_with_empty_string_injection() {
        let mut ctx = ContextConfig::default();
        ctx.tools.insert("tool1".to_string(), "".to_string());

        let result = ctx.tool_description("tool1", "Original");
        // Empty context still adds the separator
        assert_eq!(result, "Original\n\n");
    }

    #[test]
    fn test_context_special_characters_in_descriptions() {
        let mut ctx = ContextConfig::default();
        ctx.tools.insert(
            "tool1".to_string(),
            "Special chars: <>&\"'`$()[]{}\\".to_string(),
        );

        let result = ctx.tool_description("tool1", "Original");
        assert!(result.contains("Special chars: <>&\"'`$()[]{}\\"));
    }

    #[test]
    fn test_context_multiline_injection() {
        let mut ctx = ContextConfig::default();
        ctx.flags
            .insert("config".to_string(), "Line 1\nLine 2\nLine 3".to_string());

        let result = ctx.flag_description("config", "Config file");
        assert!(result.contains("Line 1\nLine 2\nLine 3"));
        assert!(result.starts_with("Config file\n\n"));
    }

    #[test]
    fn test_deserialize_with_extra_fields() {
        let json = r#"{
            "tools": {"t1": "desc1"},
            "flags": {"f1": "desc2"},
            "positionals": {"p1": "desc3"},
            "extra_field": "should_be_ignored"
        }"#;

        let ctx: Result<ContextConfig, _> = serde_json::from_str(json);
        assert!(ctx.is_ok());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.tools.len(), 1);
        assert_eq!(ctx.flags.len(), 1);
        assert_eq!(ctx.positionals.len(), 1);
    }

    #[test]
    fn test_deserialize_with_missing_fields() {
        // All fields are optional with #[serde(default)]
        let json = r#"{}"#;

        let ctx: Result<ContextConfig, _> = serde_json::from_str(json);
        assert!(ctx.is_ok());
        let ctx = ctx.unwrap();
        assert!(ctx.tools.is_empty());
        assert!(ctx.flags.is_empty());
        assert!(ctx.positionals.is_empty());
    }
}
