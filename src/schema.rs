//! Schema definitions for Foundry CLI tools

use serde::{Deserialize, Serialize};

/// Schema definition for a positional argument
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PositionalSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
    pub required: bool,
    #[serde(default)]
    pub index: Option<usize>,
}

/// Schema definition for an option (flag with value)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OptionSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
    pub required: bool,
    #[serde(default)]
    pub short: Option<String>,
    #[serde(default)]
    pub value_name: Option<String>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

/// Schema definition for a flag (boolean)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FlagSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
    pub required: bool,
    #[serde(default)]
    pub short: Option<String>,
}

/// Schema definition for a tool
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub positionals: Vec<PositionalSchema>,
    #[serde(default)]
    pub options: Vec<OptionSchema>,
    #[serde(default)]
    pub flags: Vec<FlagSchema>,
}

/// Schema container
#[derive(Debug, Deserialize, Serialize)]
pub struct SchemaFile {
    pub tools: Vec<ToolSchema>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    /// Test that positional schema deserializes correctly with all fields
    #[test]
    fn test_positional_schema_deserialization() {
        let json = r#"{
            "name": "address",
            "type": "string",
            "description": "Contract address",
            "required": true,
            "index": 0
        }"#;

        let pos: PositionalSchema = serde_json::from_str(json).unwrap();
        assert_eq!(pos.name, "address");
        assert_eq!(pos.param_type, "string");
        assert_eq!(pos.description, "Contract address");
        assert!(pos.required);
        assert_eq!(pos.index, Some(0));
    }

    /// Test that positional schema works without optional index field
    #[test]
    fn test_positional_schema_without_index() {
        let json = r#"{
            "name": "arg",
            "type": "string",
            "description": "An argument",
            "required": false
        }"#;

        let pos: PositionalSchema = serde_json::from_str(json).unwrap();
        assert_eq!(pos.name, "arg");
        assert!(!pos.required);
        assert_eq!(pos.index, None);
    }

    /// Test that option schema deserializes correctly with all optional fields
    #[test]
    fn test_option_schema_deserialization() {
        let json = r#"{
            "name": "rpc-url",
            "type": "string",
            "description": "RPC endpoint",
            "required": false,
            "short": "r",
            "value_name": "URL",
            "default": "http://localhost:8545"
        }"#;

        let opt: OptionSchema = serde_json::from_str(json).unwrap();
        assert_eq!(opt.name, "rpc-url");
        assert_eq!(opt.param_type, "string");
        assert_eq!(opt.description, "RPC endpoint");
        assert!(!opt.required);
        assert_eq!(opt.short, Some("r".to_string()));
        assert_eq!(opt.value_name, Some("URL".to_string()));
        assert_eq!(
            opt.default,
            Some(serde_json::json!("http://localhost:8545"))
        );
    }

    /// Test that option schema works with only required fields
    #[test]
    fn test_option_schema_with_defaults() {
        let json = r#"{
            "name": "config",
            "type": "path",
            "description": "Config file",
            "required": true
        }"#;

        let opt: OptionSchema = serde_json::from_str(json).unwrap();
        assert_eq!(opt.name, "config");
        assert!(opt.required);
        assert_eq!(opt.short, None);
        assert_eq!(opt.value_name, None);
        assert_eq!(opt.default, None);
    }

    /// Test that flag schema deserializes correctly with short flag
    #[test]
    fn test_flag_schema_deserialization() {
        let json = r#"{
            "name": "verbose",
            "type": "boolean",
            "description": "Verbose output",
            "required": false,
            "short": "v"
        }"#;

        let flag: FlagSchema = serde_json::from_str(json).unwrap();
        assert_eq!(flag.name, "verbose");
        assert_eq!(flag.param_type, "boolean");
        assert_eq!(flag.description, "Verbose output");
        assert!(!flag.required);
        assert_eq!(flag.short, Some("v".to_string()));
    }

    /// Test that flag schema works without optional short flag
    #[test]
    fn test_flag_schema_without_short() {
        let json = r#"{
            "name": "json",
            "type": "boolean",
            "description": "JSON output",
            "required": false
        }"#;

        let flag: FlagSchema = serde_json::from_str(json).unwrap();
        assert_eq!(flag.name, "json");
        assert_eq!(flag.short, None);
    }

    /// Test that complete tool schema with positionals, options, and flags deserializes correctly
    #[test]
    fn test_tool_schema_complete() {
        let json = r#"{
            "name": "forge_build",
            "description": "Build the project",
            "positionals": [
                {
                    "name": "path",
                    "type": "path",
                    "description": "Project path",
                    "required": false,
                    "index": 0
                }
            ],
            "options": [
                {
                    "name": "out",
                    "type": "path",
                    "description": "Output directory",
                    "required": false
                }
            ],
            "flags": [
                {
                    "name": "force",
                    "type": "boolean",
                    "description": "Force rebuild",
                    "required": false
                }
            ]
        }"#;

        let tool: ToolSchema = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "forge_build");
        assert_eq!(tool.description, "Build the project");
        assert_eq!(tool.positionals.len(), 1);
        assert_eq!(tool.options.len(), 1);
        assert_eq!(tool.flags.len(), 1);

        assert_eq!(tool.positionals[0].name, "path");
        assert_eq!(tool.options[0].name, "out");
        assert_eq!(tool.flags[0].name, "force");
    }

    /// Test that minimal tool schema without parameters deserializes correctly
    #[test]
    fn test_tool_schema_minimal() {
        let json = r#"{
            "name": "forge_clean",
            "description": "Clean build artifacts"
        }"#;

        let tool: ToolSchema = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "forge_clean");
        assert_eq!(tool.description, "Clean build artifacts");
        assert!(tool.positionals.is_empty());
        assert!(tool.options.is_empty());
        assert!(tool.flags.is_empty());
    }

    /// Test that schema file with multiple tools deserializes correctly
    #[test]
    fn test_schema_file_deserialization() {
        let json = r#"{
            "tools": [
                {
                    "name": "forge_build",
                    "description": "Build"
                },
                {
                    "name": "forge_test",
                    "description": "Test"
                }
            ]
        }"#;

        let schema_file: SchemaFile = serde_json::from_str(json).unwrap();
        assert_eq!(schema_file.tools.len(), 2);
        assert_eq!(schema_file.tools[0].name, "forge_build");
        assert_eq!(schema_file.tools[1].name, "forge_test");
    }

    /// Test that schema file with empty tools array deserializes correctly
    #[test]
    fn test_schema_file_empty_tools() {
        let json = r#"{"tools": []}"#;

        let schema_file: SchemaFile = serde_json::from_str(json).unwrap();
        assert!(schema_file.tools.is_empty());
    }

    /// Test that option defaults can contain complex nested JSON objects
    #[test]
    fn test_option_default_can_be_complex_json() {
        let json = r#"{
            "name": "config",
            "type": "object",
            "description": "Config",
            "required": false,
            "default": {"key": "value", "nested": {"inner": 42}}
        }"#;

        let opt: OptionSchema = serde_json::from_str(json).unwrap();
        assert!(opt.default.is_some());
        
        let default_val = opt.default.unwrap();
        assert!(default_val.is_object());
        assert_eq!(default_val["key"], "value");
        assert_eq!(default_val["nested"]["inner"], 42);
    }

    /// Test that positional schema can be serialized and deserialized without data loss
    #[test]
    fn test_serialization_roundtrip_positional() {
        let pos = PositionalSchema {
            name: "test".to_string(),
            param_type: "string".to_string(),
            description: "Test param".to_string(),
            required: true,
            index: Some(0),
        };

        let json = serde_json::to_string(&pos).unwrap();
        let deserialized: PositionalSchema = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.name, pos.name);
        assert_eq!(deserialized.param_type, pos.param_type);
        assert_eq!(deserialized.required, pos.required);
        assert_eq!(deserialized.index, pos.index);
    }

    /// Test that tool schema can be serialized and deserialized without data loss
    #[test]
    fn test_serialization_roundtrip_tool() {
        let tool = ToolSchema {
            name: "test_tool".to_string(),
            description: "Test".to_string(),
            positionals: vec![],
            options: vec![],
            flags: vec![],
        };

        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: ToolSchema = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.name, tool.name);
        assert_eq!(deserialized.description, tool.description);
    }

    /// Test that "type" field in JSON correctly maps to "param_type" field in Rust struct
    #[test]
    fn test_type_field_renamed_correctly() {
        let json = r#"{"name": "test", "type": "string", "description": "desc", "required": true}"#;
        let pos: PositionalSchema = serde_json::from_str(json).unwrap();
        
        // "type" in JSON should map to "param_type" in struct
        assert_eq!(pos.param_type, "string");
        
        // When serialized, it should be "type" again
        let serialized = serde_json::to_value(&pos).unwrap();
        assert!(serialized.get("type").is_some());
        assert_eq!(serialized["type"], "string");
    }

    /// Test that all parameter types (string, number, boolean, etc.) deserialize correctly
    #[test]
    fn test_param_types_variety() {
        let types = vec!["string", "number", "boolean", "array", "path", "object"];
        
        for param_type in types {
            let json = format!(
                r#"{{"name": "test", "type": "{}", "description": "desc", "required": false}}"#,
                param_type
            );
            let pos: PositionalSchema = serde_json::from_str(&json).unwrap();
            assert_eq!(pos.param_type, param_type);
        }
    }

    /// Test that invalid/incomplete JSON returns an error instead of panicking
    #[test]
    fn test_invalid_json_fails_gracefully() {
        let invalid_json = r#"{"name": "test", "type": "string"}"#; // missing required field
        let result: Result<ToolSchema, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    /// Test that parameter names with special characters (hyphens, etc.) are preserved
    #[test]
    fn test_schema_with_special_characters_in_names() {
        let json = r#"{
            "name": "rpc-url",
            "type": "string",
            "description": "RPC URL",
            "required": false
        }"#;

        let opt: OptionSchema = serde_json::from_str(json).unwrap();
        assert_eq!(opt.name, "rpc-url");
    }
}
