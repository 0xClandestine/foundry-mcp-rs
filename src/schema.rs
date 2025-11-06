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
