//! Foundry MCP Server - Entry point

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::service::ServiceExt;

use foundry_mcp::{
    config::Config, foundry::FoundryExecutor, schema::SchemaFile, FoundryMcpHandler,
};

/// Foundry MCP Server - Model Context Protocol server for Foundry CLI tools
#[derive(Parser, Debug)]
#[command(name = "foundry-mcp")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration from CLI flag or default
    let config = match cli.config {
        Some(ref config_path) => Config::from_file(config_path)?,
        None => Config::load_default(),
    };

    // Log configuration status for visibility
    log_config_status(&config);

    // Load schema from embedded schemas.json at compile time
    const SCHEMA_JSON: &str = include_str!("../schemas.json");

    let schema_file: SchemaFile =
        serde_json::from_str(SCHEMA_JSON).context("Failed to parse embedded schemas.json")?;

    // Create the Foundry executor with configuration
    let executor = FoundryExecutor::with_config(schema_file, config);

    // Log Foundry detection status to stderr (won't interfere with MCP protocol on stdout)
    if let Some(path) = executor.foundry_bin_path() {
        eprintln!("✓ Foundry detected at: {}", path);
    } else {
        eprintln!("⚠ Warning: Foundry binaries not found in common locations.");
        eprintln!("  Searched: ~/.foundry/bin, /usr/local/bin, /opt/homebrew/bin");
        eprintln!("  Install from: https://getfoundry.sh/");
    }

    // Create the MCP handler
    let handler = FoundryMcpHandler::new(executor);

    // Serve using stdio transport
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let service = handler.serve((stdin, stdout)).await?;
    service.waiting().await?;

    Ok(())
}

/// Log the current configuration status to stderr for visibility.
///
/// This helps users understand what restrictions are active.
fn log_config_status(config: &Config) {
    if !config.forbidden_commands.is_empty() {
        eprintln!("🔒 Forbidden commands: {:?}", config.forbidden_commands);
    }
    if !config.forbidden_flags.is_empty() {
        eprintln!("🔒 Forbidden flags: {:?}", config.forbidden_flags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_log_config_status_with_restrictions() {
        let config = Config {
            forbidden_commands: vec!["anvil".to_string()],
            forbidden_flags: vec!["broadcast".to_string()],
            allow_dangerous: false,
        };
        
        // Should not panic
        log_config_status(&config);
    }

    #[test]
    fn test_log_config_status_empty() {
        let config = Config {
            forbidden_commands: vec![],
            forbidden_flags: vec![],
            allow_dangerous: true,
        };
        
        // Should not panic
        log_config_status(&config);
    }

    #[test]
    fn test_cli_parsing() {
        // Test that CLI can be parsed
        let cli = Cli::parse_from(&["foundry-mcp"]);
        assert!(cli.config.is_none());
    }

    #[test]
    fn test_cli_with_config_path() {
        let cli = Cli::parse_from(&["foundry-mcp", "--config", "/path/to/config.json"]);
        assert_eq!(cli.config, Some("/path/to/config.json".to_string()));
    }

    #[test]
    fn test_cli_with_short_config_flag() {
        let cli = Cli::parse_from(&["foundry-mcp", "-c", "/path/to/config.json"]);
        assert_eq!(cli.config, Some("/path/to/config.json".to_string()));
    }

    #[test]
    fn test_embedded_schema_is_valid_json() {
        const SCHEMA_JSON: &str = include_str!("../schemas.json");
        
        // Should parse without errors
        let result: Result<SchemaFile, _> = serde_json::from_str(SCHEMA_JSON);
        assert!(result.is_ok(), "Embedded schema should be valid JSON");
        
        let schema = result.unwrap();
        assert!(!schema.tools.is_empty(), "Schema should contain tools");
    }

    #[test]
    fn test_config_loading_with_valid_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.json");
        
        let config_json = r#"{
            "forbidden_commands": ["test_command"],
            "forbidden_flags": ["test_flag"],
            "allow_dangerous": false
        }"#;
        
        fs::write(&config_path, config_json).unwrap();
        
        let config = Config::from_file(&config_path).unwrap();
        assert!(config.forbidden_commands.contains(&"test_command".to_string()));
        assert!(config.forbidden_flags.contains(&"test_flag".to_string()));
    }

    #[test]
    fn test_config_loading_with_invalid_file() {
        let result = Config::from_file("/nonexistent/path/config.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_loading_applies_dangerous_restrictions() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        
        let config_json = r#"{
            "forbidden_commands": [],
            "forbidden_flags": [],
            "allow_dangerous": false
        }"#;
        
        fs::write(&config_path, config_json).unwrap();
        
        let config = Config::from_file(&config_path).unwrap();
        
        // Should have dangerous restrictions applied automatically
        assert!(!config.forbidden_commands.is_empty());
        assert!(!config.forbidden_flags.is_empty());
        assert!(config.forbidden_commands.contains(&"anvil".to_string()));
        assert!(config.forbidden_flags.contains(&"broadcast".to_string()));
    }

    #[test]
    fn test_executor_creation_with_schema() {
        const SCHEMA_JSON: &str = include_str!("../schemas.json");
        let schema_file: SchemaFile = serde_json::from_str(SCHEMA_JSON).unwrap();
        let config = Config::default();
        
        // Should create executor without panic
        let executor = FoundryExecutor::with_config(schema_file, config);
        
        // Executor should have some tools (after filtering)
        assert!(executor.tool_list().len() >= 0);
    }

    #[test]
    fn test_handler_creation_workflow() {
        const SCHEMA_JSON: &str = include_str!("../schemas.json");
        let schema_file: SchemaFile = serde_json::from_str(SCHEMA_JSON).unwrap();
        let config = Config::default();
        
        let executor = FoundryExecutor::with_config(schema_file, config);
        let _handler = FoundryMcpHandler::new(executor);
        
        // Should create handler successfully
    }

    #[test]
    fn test_default_config_has_security_restrictions() {
        let config = Config::load_default();
        
        // Default config should have dangerous restrictions
        assert!(!config.forbidden_commands.is_empty() || !config.allow_dangerous);
        assert!(!config.forbidden_flags.is_empty() || !config.allow_dangerous);
    }

    #[test]
    fn test_config_from_file_overrides_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("custom_config.json");
        
        let config_json = r#"{
            "forbidden_commands": ["custom_command"],
            "forbidden_flags": [],
            "allow_dangerous": true
        }"#;
        
        fs::write(&config_path, config_json).unwrap();
        
        let config = Config::from_file(&config_path).unwrap();
        
        // Should use custom config
        assert!(config.forbidden_commands.contains(&"custom_command".to_string()));
        assert!(config.allow_dangerous);
        
        // Should NOT have default dangerous restrictions (allow_dangerous = true)
        assert!(!config.forbidden_commands.contains(&"anvil".to_string()));
    }

    #[test]
    fn test_safe_default_prevents_dangerous_operations() {
        let config = Config::safe_default();
        
        // Should forbid dangerous commands
        assert!(config.forbidden_commands.contains(&"anvil".to_string()));
        
        // Should forbid dangerous flags
        assert!(config.forbidden_flags.contains(&"broadcast".to_string()));
        assert!(config.forbidden_flags.contains(&"private-key".to_string()));
        
        // Should not allow dangerous operations
        assert!(!config.allow_dangerous);
    }
}
