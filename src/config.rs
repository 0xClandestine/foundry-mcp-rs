//! Configuration management for Foundry MCP Server

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Configuration for the Foundry MCP Server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// List of forbidden commands (e.g., ["anvil", "forge_script"])
    #[serde(default)]
    pub forbidden_commands: Vec<String>,

    /// List of forbidden flags (e.g., ["broadcast", "private-key"])
    #[serde(default)]
    pub forbidden_flags: Vec<String>,

    /// Whether to allow dangerous commands by default
    #[serde(default = "default_allow_dangerous")]
    pub allow_dangerous: bool,
}

fn default_allow_dangerous() -> bool {
    false
}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Self {
            forbidden_commands: vec![],
            forbidden_flags: vec![],
            allow_dangerous: false,
        }
    }
}

impl Config {
    /// Load configuration from a JSON file.
    ///
    /// Automatically applies hardcoded dangerous restrictions if `allow_dangerous` is `false`.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the configuration file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref)
            .with_context(|| format!("Failed to read config file: {}", path_ref.display()))?;

        let mut config: Config = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path_ref.display()))?;

        config.apply_dangerous_restrictions();
        Ok(config)
    }

    /// Load configuration from default location.
    ///
    /// Tries to load from `~/.foundry-mcp-config.json` first, then falls back to
    /// default configuration with hardcoded dangerous restrictions applied.
    ///
    /// # Returns
    ///
    /// A `Config` instance, either loaded from file or default.
    pub fn load_default() -> Self {
        // Try default config file location
        if let Ok(home) = std::env::var("HOME") {
            let default_path = format!("{}/.foundry-mcp-config.json", home);
            if Path::new(&default_path).exists() {
                match Self::from_file(&default_path) {
                    Ok(config) => {
                        eprintln!("✓ Loaded config from: {}", default_path);
                        return config;
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠ Warning: Failed to parse config at {}: {}",
                            default_path, e
                        );
                    }
                }
            }
        }

        // Fall back to default with dangerous restrictions
        eprintln!("ℹ Using default config with hardcoded dangerous restrictions");
        let mut config = Self::default();
        config.apply_dangerous_restrictions();
        config
    }

    /// Apply hardcoded dangerous restrictions if allow_dangerous is false.
    ///
    /// This merges the hardcoded dangerous commands/flags with user-provided ones,
    /// avoiding duplicates.
    fn apply_dangerous_restrictions(&mut self) {
        if self.allow_dangerous {
            return;
        }

        // Merge hardcoded dangerous commands (avoid duplicates)
        let dangerous_commands: Vec<String> = Self::get_default_dangerous_commands()
            .into_iter()
            .filter(|cmd| !self.forbidden_commands.contains(cmd))
            .collect();
        self.forbidden_commands.extend(dangerous_commands);

        // Merge hardcoded dangerous flags (avoid duplicates)
        let dangerous_flags: Vec<String> = Self::get_default_dangerous_flags()
            .into_iter()
            .filter(|flag| !self.forbidden_flags.contains(flag))
            .collect();
        self.forbidden_flags.extend(dangerous_flags);
    }

    /// Check if a command is forbidden
    pub fn is_command_forbidden(&self, command: &str) -> bool {
        self.forbidden_commands.iter().any(|cmd| command == cmd)
    }

    /// Check if any flags are forbidden in the given set.
    ///
    /// Returns the first forbidden flag found, if any.
    pub fn has_forbidden_flags(&self, flags: &HashSet<&str>) -> Option<String> {
        self.forbidden_flags
            .iter()
            .find(|forbidden| flags.contains(forbidden.as_str()))
            .cloned()
    }

    /// Get the list of dangerous commands that should be forbidden by default
    /// when `allow_dangerous` is `false`.
    ///
    /// # Returns
    ///
    /// A vector of command names that are considered dangerous.
    pub fn get_default_dangerous_commands() -> Vec<String> {
        vec![
            "anvil".to_string(),  // Runs a local Ethereum node
            "chisel".to_string(), // Opens an interactive REPL (use chisel_eval instead)
        ]
    }

    /// Get the list of dangerous flags that should be forbidden by default
    /// when `allow_dangerous` is `false`.
    ///
    /// # Returns
    ///
    /// A vector of flag names that are considered dangerous.
    pub fn get_default_dangerous_flags() -> Vec<String> {
        vec![
            "broadcast".to_string(),   // Broadcasting transactions to real networks
            "private-key".to_string(), // Using private keys directly
            "mnemonic".to_string(),    // Using mnemonic phrases directly
            "legacy".to_string(),      // Legacy transaction types
            "unlock".to_string(),      // Unlocking accounts
        ]
    }

    /// Create a safe default configuration with hardcoded dangerous restrictions.
    ///
    /// This is equivalent to calling `Config::default()` followed by
    /// `apply_dangerous_restrictions()`.
    pub fn safe_default() -> Self {
        Self {
            forbidden_commands: Self::get_default_dangerous_commands(),
            forbidden_flags: Self::get_default_dangerous_flags(),
            allow_dangerous: false,
        }
    }

    /// Save configuration to a file in JSON format.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the configuration should be saved
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or file writing fails.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path_ref = path.as_ref();
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize config to JSON")?;

        std::fs::write(path_ref, json)
            .with_context(|| format!("Failed to write config file: {}", path_ref.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.forbidden_commands.is_empty());
        assert!(config.forbidden_flags.is_empty());
        assert!(!config.allow_dangerous);
    }

    #[test]
    fn test_safe_default_config() {
        let config = Config::safe_default();
        assert!(!config.forbidden_commands.is_empty());
        assert!(!config.forbidden_flags.is_empty());
        assert!(!config.allow_dangerous);
    }

    #[test]
    fn test_is_command_forbidden() {
        let config = Config {
            forbidden_commands: vec!["anvil".to_string(), "forge_script".to_string()],
            forbidden_flags: vec![],
            allow_dangerous: false,
        };
        assert!(config.is_command_forbidden("anvil"));
        assert!(config.is_command_forbidden("forge_script"));
        assert!(!config.is_command_forbidden("forge_build"));
    }

    #[test]
    fn test_has_forbidden_flags() {
        let config = Config {
            forbidden_commands: vec![],
            forbidden_flags: vec!["broadcast".to_string(), "private-key".to_string()],
            allow_dangerous: false,
        };

        let mut flags = HashSet::new();
        flags.insert("broadcast");
        flags.insert("verify");

        assert!(config.has_forbidden_flags(&flags).is_some());

        let mut safe_flags = HashSet::new();
        safe_flags.insert("verify");
        safe_flags.insert("json");

        assert!(config.has_forbidden_flags(&safe_flags).is_none());
    }

    #[test]
    fn test_apply_dangerous_restrictions() {
        // Test with allow_dangerous = false (should add hardcoded restrictions)
        let mut config = Config {
            forbidden_commands: vec!["forge_script".to_string()],
            forbidden_flags: vec!["ledger".to_string()],
            allow_dangerous: false,
        };
        config.apply_dangerous_restrictions();

        // Should have custom + hardcoded commands
        assert!(config
            .forbidden_commands
            .contains(&"forge_script".to_string()));
        assert!(config.forbidden_commands.contains(&"anvil".to_string()));

        // Should have custom + hardcoded flags
        assert!(config.forbidden_flags.contains(&"ledger".to_string()));
        assert!(config.forbidden_flags.contains(&"broadcast".to_string()));
        assert!(config.forbidden_flags.contains(&"private-key".to_string()));
    }

    #[test]
    fn test_apply_dangerous_restrictions_with_allow() {
        // Test with allow_dangerous = true (should NOT add hardcoded restrictions)
        let mut config = Config {
            forbidden_commands: vec!["forge_script".to_string()],
            forbidden_flags: vec!["ledger".to_string()],
            allow_dangerous: true,
        };
        config.apply_dangerous_restrictions();

        // Should only have custom commands (no hardcoded)
        assert!(config
            .forbidden_commands
            .contains(&"forge_script".to_string()));
        assert!(!config.forbidden_commands.contains(&"anvil".to_string()));

        // Should only have custom flags (no hardcoded)
        assert!(config.forbidden_flags.contains(&"ledger".to_string()));
        assert!(!config.forbidden_flags.contains(&"broadcast".to_string()));
    }

    #[test]
    fn test_no_duplicate_restrictions() {
        // Test that apply_dangerous_restrictions doesn't create duplicates
        let mut config = Config {
            forbidden_commands: vec!["anvil".to_string()], // Already has hardcoded command
            forbidden_flags: vec!["broadcast".to_string()], // Already has hardcoded flag
            allow_dangerous: false,
        };
        config.apply_dangerous_restrictions();

        // Count occurrences - should be exactly 1 each
        assert_eq!(
            config
                .forbidden_commands
                .iter()
                .filter(|c| *c == "anvil")
                .count(),
            1
        );
        assert_eq!(
            config
                .forbidden_flags
                .iter()
                .filter(|f| *f == "broadcast")
                .count(),
            1
        );
    }
}
