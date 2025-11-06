//! Foundry CLI tool execution and schema conversion

use anyhow::{Context, Result};
use rmcp::model::*;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;

use crate::config::Config;
use crate::context::ContextConfig;
use crate::schema::{SchemaFile, ToolSchema};

type JsonObject = serde_json::Map<String, Value>;

/// Parse tool name parts into command components (handles triple underscore pattern)
fn parse_subcommand_parts(parts: &[&str]) -> (Vec<String>, bool) {
    let mut subcommand_parts = Vec::new();
    let mut found_triple_underscore = false;
    let mut i = 1;
    let mut result = Vec::new();

    while i < parts.len() {
        if parts[i].is_empty() && i + 2 < parts.len() && parts[i + 1].is_empty() {
            i += 2;
            if i < parts.len() && !parts[i].is_empty() {
                if !subcommand_parts.is_empty() {
                    result.push(subcommand_parts.join("-"));
                    subcommand_parts.clear();
                }
                result.push(format!("--{}", parts[i]));
                found_triple_underscore = true;
            }
            i += 1;
        } else if !parts[i].is_empty() {
            subcommand_parts.push(parts[i].to_string());
            i += 1;
        } else {
            i += 1;
        }
    }

    if !subcommand_parts.is_empty() && !found_triple_underscore {
        result.push(subcommand_parts.join("-"));
    }

    (result, found_triple_underscore)
}

/// Foundry tool executor with security configuration support.
///
/// This executor manages Foundry CLI tools, filters out forbidden commands/flags
/// based on configuration, and handles command execution.
pub struct FoundryExecutor {
    tools: HashMap<String, ToolSchema>,
    tool_list: Vec<Tool>,
    foundry_bin_path: Option<String>,
    #[allow(dead_code)]
    config: Config,
    #[allow(dead_code)]
    context: Arc<ContextConfig>,
}

impl FoundryExecutor {
    /// Create a new FoundryExecutor with default configuration.
    pub fn new(schema_file: SchemaFile) -> Self {
        Self::with_config(schema_file, Config::default())
    }

    /// Create a new FoundryExecutor with custom configuration.
    ///
    /// Forbidden commands and their variants are filtered out during initialization.
    pub fn with_config(schema_file: SchemaFile, config: Config) -> Self {
        let context = Arc::new(ContextConfig::load());

        let filtered_tools: Vec<ToolSchema> = schema_file
            .tools
            .into_iter()
            .filter(|tool| Self::is_tool_allowed(tool, &config))
            .collect();

        let tools: HashMap<String, ToolSchema> = filtered_tools
            .iter()
            .map(|tool| (tool.name.clone(), tool.clone()))
            .collect();

        let tool_list: Vec<Tool> = filtered_tools
            .iter()
            .map(|tool| Self::schema_to_tool(tool, &config, &context))
            .collect();

        let foundry_bin_path = Self::detect_foundry_path();

        Self {
            tools,
            tool_list,
            foundry_bin_path,
            config,
            context,
        }
    }

    /// Get the list of available tools (after filtering).
    pub fn tool_list(&self) -> &[Tool] {
        &self.tool_list
    }

    /// Get the detected Foundry binary path, if found.
    pub fn foundry_bin_path(&self) -> &Option<String> {
        &self.foundry_bin_path
    }

    /// Check if a tool is allowed based on configuration.
    ///
    /// Returns `false` if the tool name or its base command is forbidden.
    fn is_tool_allowed(tool: &ToolSchema, config: &Config) -> bool {
        // Check if the full tool name is forbidden
        if config.is_command_forbidden(&tool.name) {
            eprintln!("🚫 Filtering out forbidden command: {}", tool.name);
            return false;
        }

        // Check if the base command is forbidden (e.g., "anvil" in "anvil_fork")
        let parts: Vec<&str> = tool.name.split('_').collect();
        if !parts.is_empty() && config.is_command_forbidden(parts[0]) {
            eprintln!("🚫 Filtering out forbidden command: {}", tool.name);
            return false;
        }

        true
    }

    fn get_command_path(&self, command_name: &str) -> String {
        if let Some(bin_path) = &self.foundry_bin_path {
            format!("{}/{}", bin_path, command_name)
        } else {
            command_name.to_string()
        }
    }

    fn detect_foundry_path() -> Option<String> {
        // Common installation paths for Foundry
        let home = std::env::var("HOME").ok()?;
        let common_paths = vec![
            format!("{}/.foundry/bin", home),
            "/usr/local/bin".to_string(),
            "/opt/homebrew/bin".to_string(),
        ];

        for path in common_paths {
            let forge_path = format!("{}/forge", path);
            if std::path::Path::new(&forge_path).exists() {
                return Some(path);
            }
        }

        // Try to find via which command
        if let Ok(output) = Command::new("which").arg("forge").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    return Some(parent.to_string_lossy().to_string());
                }
            }
        }

        None
    }

    /// Convert a ToolSchema to an MCP Tool, filtering out forbidden flags.
    fn schema_to_tool(tool: &ToolSchema, config: &Config, context: &ContextConfig) -> Tool {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        // Add positional arguments
        for pos in &tool.positionals {
            let description = context.positional_description(&pos.name, &pos.description);
            properties.insert(
                pos.name.clone(),
                serde_json::json!({
                    "type": Self::map_type(&pos.param_type),
                    "description": description,
                }),
            );
            if pos.required {
                required.push(Value::String(pos.name.clone()));
            }
        }

        // Add options (flags with values) - filter out forbidden flags
        for opt in &tool.options {
            if config.forbidden_flags.contains(&opt.name) {
                continue;
            }

            let description = context.flag_description(&opt.name, &opt.description);
            let mut prop = serde_json::json!({
                "type": Self::map_type(&opt.param_type),
                "description": description,
            });
            if let Some(default) = &opt.default {
                prop.as_object_mut()
                    .unwrap()
                    .insert("default".to_string(), default.clone());
            }
            properties.insert(opt.name.clone(), prop);
            if opt.required {
                required.push(Value::String(opt.name.clone()));
            }
        }

        // Add flags (boolean) - filter out forbidden flags
        for flag in &tool.flags {
            if config.forbidden_flags.contains(&flag.name) {
                continue;
            }

            let description = context.flag_description(&flag.name, &flag.description);
            properties.insert(
                flag.name.clone(),
                serde_json::json!({
                    "type": "boolean",
                    "description": description,
                }),
            );
            if flag.required {
                required.push(Value::String(flag.name.clone()));
            }
        }

        let mut input_schema = serde_json::Map::new();
        input_schema.insert("type".to_string(), Value::String("object".to_string()));
        input_schema.insert("properties".to_string(), Value::Object(properties));
        if !required.is_empty() {
            input_schema.insert("required".to_string(), Value::Array(required));
        }

        let tool_description = context.tool_description(&tool.name, &tool.description);

        Tool::new(tool.name.clone(), tool_description, Arc::new(input_schema))
    }

    /// Map Foundry parameter types to JSON schema types.
    fn map_type(param_type: &str) -> &str {
        match param_type {
            "boolean" => "boolean",
            "number" => "number",
            "string" | "path" => "string",
            "array" => "array",
            _ => "string",
        }
    }

    /// Execute a Foundry CLI tool with the given arguments.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name (e.g., "forge_build", "cast_call")
    /// * `arguments` - Optional JSON object containing tool arguments
    ///
    /// # Returns
    ///
    /// Combined stdout and stderr output from the command
    ///
    /// # Errors
    ///
    /// Returns an error if the tool is not found, arguments are invalid,
    /// or command execution fails.
    pub fn execute_tool(&self, name: &str, arguments: &Option<JsonObject>) -> Result<String> {
        let tool = self
            .tools
            .get(name)
            .context(format!("Tool '{}' not found", name))?;
        let parts: Vec<&str> = name.split('_').collect();
        anyhow::ensure!(!parts.is_empty(), "Invalid tool name: {}", name);

        let command_path = self.get_command_path(parts[0]);
        let mut cmd = Command::new(&command_path);

        // Add subcommands/flags from tool name
        let (subcommands, _) = parse_subcommand_parts(&parts);
        eprintln!(
            "[DEBUG] Tool: {} -> Command: {} {}",
            name,
            parts[0],
            subcommands.join(" ")
        );
        for subcommand in subcommands {
            cmd.arg(subcommand);
        }

        // Build command arguments from the schema and provided values
        if let Some(args) = arguments {
            // Add positional arguments first (sorted by index)
            let mut positionals: Vec<_> = tool.positionals.iter().collect();
            positionals.sort_by_key(|p| p.index.unwrap_or(0));

            for pos in positionals {
                if let Some(value) = args.get(&pos.name) {
                    Self::add_positional_argument(&mut cmd, value, &pos.param_type)?;
                } else if pos.required {
                    anyhow::bail!("Required positional argument '{}' not provided", pos.name);
                }
            }

            // Add flags (boolean options)
            for flag in &tool.flags {
                if let Some(value) = args.get(&flag.name) {
                    if let Some(true) = value.as_bool() {
                        cmd.arg(format!("--{}", flag.name));
                    }
                }
            }

            // Add options (flags with values)
            for opt in &tool.options {
                if let Some(value) = args.get(&opt.name) {
                    Self::add_option_argument(&mut cmd, &opt.name, value, &opt.param_type)?;
                } else if opt.required {
                    anyhow::bail!("Required option '{}' not provided", opt.name);
                }
            }
        }

        // Execute the command
        let output = cmd.output().with_context(|| {
            if self.foundry_bin_path.is_some() {
                format!(
                    "Failed to execute '{}' at '{}'. Try running '{} --version'",
                    parts[0], command_path, command_path
                )
            } else {
                format!(
                    "Failed to execute '{}'. Install Foundry from https://getfoundry.sh/",
                    parts[0]
                )
            }
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        if output.status.success() {
            Ok(combined)
        } else {
            anyhow::bail!(combined)
        }
    }

    fn value_to_string(value: &Value) -> Option<String> {
        value
            .as_str()
            .map(String::from)
            .or_else(|| value.as_i64().map(|n| n.to_string()))
            .or_else(|| value.as_u64().map(|n| n.to_string()))
            .or_else(|| value.as_f64().map(|n| n.to_string()))
    }

    fn add_positional_argument(cmd: &mut Command, value: &Value, param_type: &str) -> Result<()> {
        if param_type == "array" {
            if let Some(arr) = value.as_array() {
                for item in arr {
                    if let Some(s) = Self::value_to_string(item) {
                        cmd.arg(s);
                    }
                }
            }
        } else if let Some(s) = Self::value_to_string(value) {
            cmd.arg(s);
        }
        Ok(())
    }

    fn add_option_argument(
        cmd: &mut Command,
        name: &str,
        value: &Value,
        param_type: &str,
    ) -> Result<()> {
        let flag = format!("--{}", name);

        if param_type == "array" {
            if let Some(arr) = value.as_array() {
                for item in arr {
                    if let Some(s) = Self::value_to_string(item) {
                        cmd.arg(&flag).arg(s);
                    }
                }
            }
        } else if let Some(s) = Self::value_to_string(value) {
            cmd.arg(&flag).arg(s);
        }
        Ok(())
    }
}
