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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{FlagSchema, OptionSchema, PositionalSchema};

    fn create_test_schema() -> SchemaFile {
        SchemaFile {
            tools: vec![
                ToolSchema {
                    name: "forge_build".to_string(),
                    description: "Build the project".to_string(),
                    positionals: vec![],
                    options: vec![],
                    flags: vec![],
                },
                ToolSchema {
                    name: "cast_call".to_string(),
                    description: "Call a contract".to_string(),
                    positionals: vec![
                        PositionalSchema {
                            name: "address".to_string(),
                            param_type: "string".to_string(),
                            description: "Contract address".to_string(),
                            required: true,
                            index: Some(0),
                        },
                    ],
                    options: vec![
                        OptionSchema {
                            name: "rpc-url".to_string(),
                            param_type: "string".to_string(),
                            description: "RPC URL".to_string(),
                            required: false,
                            short: None,
                            value_name: None,
                            default: None,
                        },
                        OptionSchema {
                            name: "private-key".to_string(),
                            param_type: "string".to_string(),
                            description: "Private key".to_string(),
                            required: false,
                            short: None,
                            value_name: None,
                            default: None,
                        },
                    ],
                    flags: vec![
                        FlagSchema {
                            name: "json".to_string(),
                            param_type: "boolean".to_string(),
                            description: "Output as JSON".to_string(),
                            required: false,
                            short: None,
                        },
                        FlagSchema {
                            name: "broadcast".to_string(),
                            param_type: "boolean".to_string(),
                            description: "Broadcast transaction".to_string(),
                            required: false,
                            short: None,
                        },
                    ],
                },
                ToolSchema {
                    name: "anvil".to_string(),
                    description: "Start local testnet".to_string(),
                    positionals: vec![],
                    options: vec![],
                    flags: vec![],
                },
                ToolSchema {
                    name: "forge_script".to_string(),
                    description: "Run a script".to_string(),
                    positionals: vec![],
                    options: vec![
                        OptionSchema {
                            name: "broadcast".to_string(),
                            param_type: "boolean".to_string(),
                            description: "Broadcast transactions".to_string(),
                            required: false,
                            short: None,
                            value_name: None,
                            default: None,
                        },
                    ],
                    flags: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_parse_subcommand_parts_basic() {
        let parts = vec!["forge", "build"];
        let (result, has_triple) = parse_subcommand_parts(&parts);
        assert_eq!(result, vec!["build"]);
        assert!(!has_triple);
    }

    #[test]
    fn test_parse_subcommand_parts_with_hyphen() {
        let parts = vec!["forge", "clean", "cache"];
        let (result, has_triple) = parse_subcommand_parts(&parts);
        assert_eq!(result, vec!["clean-cache"]);
        assert!(!has_triple);
    }

    #[test]
    fn test_parse_subcommand_parts_with_triple_underscore() {
        let parts = vec!["cast", "block", "", "", "number"];
        let (result, has_triple) = parse_subcommand_parts(&parts);
        assert_eq!(result, vec!["block", "--number"]);
        assert!(has_triple);
    }

    #[test]
    fn test_parse_subcommand_parts_empty() {
        let parts = vec!["forge"];
        let (result, has_triple) = parse_subcommand_parts(&parts);
        assert!(result.is_empty());
        assert!(!has_triple);
    }

    #[test]
    fn test_default_executor_filters_dangerous_commands() {
        let schema = create_test_schema();
        // Use load_default which applies dangerous restrictions
        let config = Config::load_default();
        let executor = FoundryExecutor::with_config(schema, config);
        
        // anvil should be filtered out by default
        assert!(executor.tools.get("anvil").is_none());
        
        // Safe tools should be present
        assert!(executor.tools.get("forge_build").is_some());
        assert!(executor.tools.get("cast_call").is_some());
    }

    #[test]
    fn test_executor_with_custom_config_filters_commands() {
        let schema = create_test_schema();
        let config = Config {
            forbidden_commands: vec!["forge_build".to_string()],
            forbidden_flags: vec![],
            allow_dangerous: true, // Allow anvil but not forge_build
        };
        
        let executor = FoundryExecutor::with_config(schema, config);
        
        // forge_build should be filtered out
        assert!(executor.tools.get("forge_build").is_none());
        
        // anvil should be present (allow_dangerous = true)
        assert!(executor.tools.get("anvil").is_some());
        
        // Other tools should be present
        assert!(executor.tools.get("cast_call").is_some());
    }

    #[test]
    fn test_executor_filters_forbidden_flags() {
        let schema = create_test_schema();
        let config = Config {
            forbidden_commands: vec![],
            forbidden_flags: vec!["broadcast".to_string(), "private-key".to_string()],
            allow_dangerous: true,
        };
        
        let executor = FoundryExecutor::with_config(schema, config);
        
        // Tool should exist
        let tool_list = executor.tool_list();
        let cast_call = tool_list.iter().find(|t| t.name == "cast_call").unwrap();
        
        // Check that forbidden flags are not in the input schema
        let properties = cast_call.input_schema.get("properties").unwrap().as_object().unwrap();
        
        // Safe flags should be present
        assert!(properties.contains_key("json"));
        assert!(properties.contains_key("rpc-url"));
        
        // Forbidden flags should NOT be present
        assert!(!properties.contains_key("broadcast"));
        assert!(!properties.contains_key("private-key"));
    }

    #[test]
    fn test_is_tool_allowed_filters_base_command() {
        let config = Config {
            forbidden_commands: vec!["anvil".to_string()],
            forbidden_flags: vec![],
            allow_dangerous: true,
        };
        
        let tool = ToolSchema {
            name: "anvil_fork".to_string(),
            description: "Fork with anvil".to_string(),
            positionals: vec![],
            options: vec![],
            flags: vec![],
        };
        
        // Should be filtered because base command "anvil" is forbidden
        assert!(!FoundryExecutor::is_tool_allowed(&tool, &config));
    }

    #[test]
    fn test_is_tool_allowed_exact_match() {
        let config = Config {
            forbidden_commands: vec!["forge_script".to_string()],
            forbidden_flags: vec![],
            allow_dangerous: true,
        };
        
        let tool = ToolSchema {
            name: "forge_script".to_string(),
            description: "Run script".to_string(),
            positionals: vec![],
            options: vec![],
            flags: vec![],
        };
        
        // Should be filtered by exact name match
        assert!(!FoundryExecutor::is_tool_allowed(&tool, &config));
    }

    #[test]
    fn test_map_type_conversions() {
        assert_eq!(FoundryExecutor::map_type("boolean"), "boolean");
        assert_eq!(FoundryExecutor::map_type("number"), "number");
        assert_eq!(FoundryExecutor::map_type("string"), "string");
        assert_eq!(FoundryExecutor::map_type("path"), "string");
        assert_eq!(FoundryExecutor::map_type("array"), "array");
        assert_eq!(FoundryExecutor::map_type("unknown"), "string");
    }

    #[test]
    fn test_value_to_string_conversions() {
        use serde_json::json;
        
        assert_eq!(
            FoundryExecutor::value_to_string(&json!("test")),
            Some("test".to_string())
        );
        assert_eq!(
            FoundryExecutor::value_to_string(&json!(42)),
            Some("42".to_string())
        );
        assert_eq!(
            FoundryExecutor::value_to_string(&json!(3.14)),
            Some("3.14".to_string())
        );
        assert_eq!(
            FoundryExecutor::value_to_string(&json!(100u64)),
            Some("100".to_string())
        );
    }

    #[test]
    fn test_value_to_string_invalid_types() {
        use serde_json::json;
        
        // Objects and arrays should not convert
        assert!(FoundryExecutor::value_to_string(&json!({"key": "value"})).is_none());
        assert!(FoundryExecutor::value_to_string(&json!([1, 2, 3])).is_none());
        assert!(FoundryExecutor::value_to_string(&json!(null)).is_none());
    }

    #[test]
    fn test_schema_to_tool_includes_all_parameters() {
        let schema = ToolSchema {
            name: "test_tool".to_string(),
            description: "Test tool".to_string(),
            positionals: vec![
                PositionalSchema {
                    name: "arg1".to_string(),
                    param_type: "string".to_string(),
                    description: "First arg".to_string(),
                    required: true,
                    index: Some(0),
                },
            ],
            options: vec![
                OptionSchema {
                    name: "option1".to_string(),
                    param_type: "string".to_string(),
                    description: "Option 1".to_string(),
                    required: false,
                    short: None,
                    value_name: None,
                    default: Some(serde_json::json!("default_value")),
                },
            ],
            flags: vec![
                FlagSchema {
                    name: "flag1".to_string(),
                    param_type: "boolean".to_string(),
                    description: "Flag 1".to_string(),
                    required: false,
                    short: None,
                },
            ],
        };
        
        let context = ContextConfig::default();
        let config = Config::default();
        let tool = FoundryExecutor::schema_to_tool(&schema, &config, &context);
        
        assert_eq!(tool.name, "test_tool");
        
        let properties = tool.input_schema.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("arg1"));
        assert!(properties.contains_key("option1"));
        assert!(properties.contains_key("flag1"));
        
        // Check that default value is included
        let option1 = properties.get("option1").unwrap();
        assert_eq!(
            option1.get("default").unwrap().as_str().unwrap(),
            "default_value"
        );
        
        // Check required fields
        let required = tool.input_schema.get("required").unwrap().as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].as_str().unwrap(), "arg1");
    }

    #[test]
    fn test_execute_tool_requires_valid_tool_name() {
        let schema = create_test_schema();
        let executor = FoundryExecutor::new(schema);
        
        let result = executor.execute_tool("nonexistent_tool", &None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_get_command_path_with_bin_path() {
        let schema = SchemaFile { tools: vec![] };
        let mut executor = FoundryExecutor::new(schema);
        
        // Manually set the bin path for testing
        executor.foundry_bin_path = Some("/test/bin".to_string());
        
        let path = executor.get_command_path("forge");
        assert_eq!(path, "/test/bin/forge");
    }

    #[test]
    fn test_get_command_path_without_bin_path() {
        let schema = SchemaFile { tools: vec![] };
        let mut executor = FoundryExecutor::new(schema);
        executor.foundry_bin_path = None;
        
        let path = executor.get_command_path("forge");
        assert_eq!(path, "forge");
    }

    #[test]
    fn test_safe_default_prevents_dangerous_tools() {
        let schema = create_test_schema();
        // Use safe_default which has all dangerous restrictions
        let config = Config::safe_default();
        let executor = FoundryExecutor::with_config(schema, config);
        
        // Verify dangerous commands are filtered
        assert!(executor.tools.get("anvil").is_none());
        
        // Verify dangerous flags are filtered from remaining tools
        if let Some(cast_tool) = executor.tools.get("cast_call") {
            let has_broadcast_flag = cast_tool.flags.iter().any(|f| f.name == "broadcast");
            let has_private_key_option = cast_tool.options.iter().any(|o| o.name == "private-key");
            
            // These should not be present in the schema
            assert!(!has_broadcast_flag, "broadcast flag should be filtered");
            assert!(!has_private_key_option, "private-key option should be filtered");
        }
    }

    #[test]
    fn test_tool_list_only_contains_allowed_tools() {
        let schema = create_test_schema();
        let config = Config {
            forbidden_commands: vec!["cast_call".to_string()],
            forbidden_flags: vec![],
            allow_dangerous: true,
        };
        
        let executor = FoundryExecutor::with_config(schema, config);
        let tool_list = executor.tool_list();
        
        // cast_call should not be in the list
        assert!(!tool_list.iter().any(|t| t.name == "cast_call"));
        
        // Other allowed tools should be present
        assert!(tool_list.iter().any(|t| t.name == "forge_build"));
    }
}
