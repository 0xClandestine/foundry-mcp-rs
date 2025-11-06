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
