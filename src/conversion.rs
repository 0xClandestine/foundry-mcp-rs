//! Unified conversion tool for all cast conversion operations
//!
//! This module provides a single MCP tool that wraps all cast conversion CLI subcommands.

use anyhow::{Context, Result};
use rmcp::model::{CallToolResult, Content, Tool};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Command;
use std::sync::Arc;

/// All supported conversion types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversionType {
    // Integer type limits
    MaxInt,
    MinInt,
    MaxUint,

    // Zero constants
    AddressZero,
    HashZero,

    // Text/binary conversions
    FromUtf8,
    ToAscii,
    ToUtf8,
    FromBin,

    // Hex operations
    ConcatHex,
    ToHexdata,
    ToHex,
    ToDec,
    ToBase,

    // Address operations
    ToCheckSumAddress,

    // Bytes operations
    ToBytes32,

    // Integer type conversions
    ToUint256,
    ToInt256,

    // Fixed point conversions
    FromFixedPoint,
    ToFixedPoint,

    // Bit shift operations
    Shl,
    Shr,

    // Unit conversions
    ToUnit,
    ParseUnits,
    FormatUnits,
    ToWei,
    FromWei,

    // RLP encoding
    ToRlp,
    FromRlp,
}

impl ConversionType {
    /// Get the cast subcommand name for this conversion
    fn subcommand(&self) -> &'static str {
        match self {
            Self::MaxInt => "max-int",
            Self::MinInt => "min-int",
            Self::MaxUint => "max-uint",
            Self::AddressZero => "address-zero",
            Self::HashZero => "hash-zero",
            Self::FromUtf8 => "from-utf8",
            Self::ToAscii => "to-ascii",
            Self::ToUtf8 => "to-utf8",
            Self::FromBin => "from-bin",
            Self::ConcatHex => "concat-hex",
            Self::ToHexdata => "to-hexdata",
            Self::ToHex => "to-hex",
            Self::ToDec => "to-dec",
            Self::ToBase => "to-base",
            Self::ToCheckSumAddress => "to-check-sum-address",
            Self::ToBytes32 => "to-bytes32",
            Self::ToUint256 => "to-uint256",
            Self::ToInt256 => "to-int256",
            Self::FromFixedPoint => "from-fixed-point",
            Self::ToFixedPoint => "to-fixed-point",
            Self::Shl => "shl",
            Self::Shr => "shr",
            Self::ToUnit => "to-unit",
            Self::ParseUnits => "parse-units",
            Self::FormatUnits => "format-units",
            Self::ToWei => "to-wei",
            Self::FromWei => "from-wei",
            Self::ToRlp => "to-rlp",
            Self::FromRlp => "from-rlp",
        }
    }

    /// Get a human-readable description of this conversion
    pub fn description(&self) -> &'static str {
        match self {
            Self::MaxInt => "Get the maximum value of a signed integer type",
            Self::MinInt => "Get the minimum value of a signed integer type",
            Self::MaxUint => "Get the maximum value of an unsigned integer type",
            Self::AddressZero => "Get the zero address (0x0000...0000)",
            Self::HashZero => "Get the zero hash (0x0000...0000)",
            Self::FromUtf8 => "Convert UTF-8 text to hex",
            Self::ToAscii => "Convert hex to ASCII string",
            Self::ToUtf8 => "Convert hex to UTF-8 string",
            Self::FromBin => "Convert binary data to hex",
            Self::ConcatHex => "Concatenate multiple hex strings",
            Self::ToHexdata => "Normalize input to lowercase 0x-prefixed hex",
            Self::ToHex => "Convert number to hexadecimal",
            Self::ToDec => "Convert number to decimal",
            Self::ToBase => "Convert number to arbitrary base",
            Self::ToCheckSumAddress => "Convert address to EIP-55 checksummed format",
            Self::ToBytes32 => "Right-pad hex data to 32 bytes",
            Self::ToUint256 => "Convert number to hex-encoded uint256",
            Self::ToInt256 => "Convert number to hex-encoded int256",
            Self::FromFixedPoint => "Convert fixed point number to integer",
            Self::ToFixedPoint => "Convert integer to fixed point number",
            Self::Shl => "Perform left bit shift operation",
            Self::Shr => "Perform right bit shift operation",
            Self::ToUnit => "Convert ETH amount between units (wei, gwei, ether)",
            Self::ParseUnits => "Convert decimal to smallest unit with arbitrary decimals",
            Self::FormatUnits => "Convert smallest unit to decimal with arbitrary decimals",
            Self::ToWei => "Convert ETH amount to wei",
            Self::FromWei => "Convert wei to ETH amount",
            Self::ToRlp => "RLP encode hex data or array",
            Self::FromRlp => "Decode RLP hex-encoded data",
        }
    }
}

/// Parameters for conversion operations
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConversionParams {
    /// The type of conversion to perform
    pub conversion_type: String,

    /// Primary input value (for most conversions)
    pub value: Option<String>,

    /// Array of values (for concat-hex)
    pub values: Option<Vec<String>>,

    /// Integer type (for max-int, min-int, max-uint)
    #[serde(rename = "type")]
    pub int_type: Option<String>,

    /// Number of decimals (for fixed-point conversions)
    pub decimals: Option<String>,

    /// Unit for ETH conversions (wei, gwei, ether)
    pub unit: Option<String>,

    /// Target base for base conversions
    pub base: Option<String>,

    /// Input base for base conversions
    pub base_in: Option<String>,

    /// Output base for shift operations
    pub base_out: Option<String>,

    /// Number of bits for shift operations
    pub bits: Option<String>,

    /// Chain ID for EIP-1191 address encoding
    pub chain_id: Option<u64>,

    /// Decode RLP as integer
    pub as_int: Option<bool>,
}

/// Get the unified cast conversion tool definition
pub fn get_conversion_tool() -> Tool {
    let input_schema = json!({
        "type": "object",
        "properties": {
            "conversion_type": {
                "type": "string",
                "description": "The type of conversion to perform",
                "enum": [
                    "max-int", "min-int", "max-uint",
                    "address-zero", "hash-zero",
                    "from-utf8", "to-ascii", "to-utf8", "from-bin",
                    "concat-hex", "to-hexdata", "to-hex", "to-dec", "to-base",
                    "to-check-sum-address", "to-bytes32",
                    "to-uint256", "to-int256",
                    "from-fixed-point", "to-fixed-point",
                    "shl", "shr",
                    "to-unit", "parse-units", "format-units", "to-wei", "from-wei",
                    "to-rlp", "from-rlp"
                ]
            },
            "value": {
                "type": "string",
                "description": "Primary input value (for most conversions)"
            },
            "values": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Array of values (for concat-hex)"
            },
            "type": {
                "type": "string",
                "description": "Integer type (for max-int, min-int, max-uint). Examples: int8, int256, uint256"
            },
            "decimals": {
                "type": "string",
                "description": "Number of decimals (for fixed-point conversions)"
            },
            "unit": {
                "type": "string",
                "description": "Unit for ETH conversions (wei, gwei, ether)"
            },
            "base": {
                "type": "string",
                "description": "Target base for base conversions (2-64)"
            },
            "base_in": {
                "type": "string",
                "description": "Input base for base conversions (2-64)"
            },
            "base_out": {
                "type": "string",
                "description": "Output base for shift operations (2-64)"
            },
            "bits": {
                "type": "string",
                "description": "Number of bits for shift operations"
            },
            "chain_id": {
                "type": "number",
                "description": "Chain ID for EIP-1191 address encoding"
            },
            "as_int": {
                "type": "boolean",
                "description": "Decode RLP as integer (for from-rlp)"
            }
        },
        "required": ["conversion_type"]
    });

    let description = "Unified tool for all cast conversion operations. \
        Supports: number conversions (hex/dec/base), ETH unit conversions (wei/gwei/ether), \
        text encoding (UTF8/ASCII/hex), address formatting (checksum), \
        integer types (uint256/int256), fixed-point arithmetic, bit shifting, \
        RLP encoding/decoding, and more. \
        Specify the conversion_type and provide the required parameters for that conversion.";

    Tool::new(
        "cast_convert".to_string(),
        description.to_string(),
        Arc::new(input_schema.as_object().unwrap().clone()),
    )
}

/// Handle the cast_convert tool call
pub async fn handle_cast_convert(
    arguments: &Option<serde_json::Map<String, Value>>,
    cast_path: &str,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let args = arguments
        .as_ref()
        .ok_or_else(|| rmcp::ErrorData::invalid_params("Missing arguments", None))?;

    let params: ConversionParams = serde_json::from_value(Value::Object(args.clone()))
        .map_err(|e| rmcp::ErrorData::invalid_params(format!("Invalid parameters: {}", e), None))?;

    match execute_conversion(params, cast_path) {
        Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
        Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
    }
}

/// Execute a cast conversion
pub fn execute_conversion(params: ConversionParams, cast_path: &str) -> Result<String> {
    let conversion_type: ConversionType =
        serde_json::from_str(&format!("\"{}\"", params.conversion_type))
            .with_context(|| format!("Invalid conversion type: {}", params.conversion_type))?;

    let mut cmd = Command::new(cast_path);
    cmd.arg(conversion_type.subcommand());

    // Add positional arguments based on conversion type
    match conversion_type {
        ConversionType::MaxInt | ConversionType::MinInt | ConversionType::MaxUint => {
            if let Some(t) = params.int_type {
                cmd.arg(t);
            }
        }
        ConversionType::AddressZero | ConversionType::HashZero | ConversionType::FromBin => {
            // No arguments needed
        }
        ConversionType::FromUtf8
        | ConversionType::ToAscii
        | ConversionType::ToUtf8
        | ConversionType::ToHexdata
        | ConversionType::ToBytes32
        | ConversionType::ToUint256
        | ConversionType::ToInt256
        | ConversionType::ToRlp => {
            if let Some(v) = params.value {
                cmd.arg(v);
            }
        }
        ConversionType::ConcatHex => {
            if let Some(vals) = params.values {
                for v in vals {
                    cmd.arg(v);
                }
            }
        }
        ConversionType::ToCheckSumAddress => {
            if let Some(v) = params.value {
                cmd.arg(v);
            }
            if let Some(chain_id) = params.chain_id {
                cmd.arg(chain_id.to_string());
            }
        }
        ConversionType::FromFixedPoint | ConversionType::ToFixedPoint => {
            if let Some(d) = params.decimals {
                cmd.arg(d);
            }
            if let Some(v) = params.value {
                cmd.arg(v);
            }
        }
        ConversionType::Shl | ConversionType::Shr => {
            if let Some(v) = params.value {
                cmd.arg(v);
            }
            if let Some(b) = params.bits {
                cmd.arg(b);
            }
            if let Some(base_in) = params.base_in {
                cmd.arg("--base-in").arg(base_in);
            }
            if let Some(base_out) = params.base_out {
                cmd.arg("--base-out").arg(base_out);
            }
        }
        ConversionType::ToUnit
        | ConversionType::ToWei
        | ConversionType::FromWei
        | ConversionType::ParseUnits
        | ConversionType::FormatUnits => {
            if let Some(v) = params.value {
                cmd.arg(v);
            }
            if let Some(u) = params.unit {
                cmd.arg(u);
            }
        }
        ConversionType::ToHex | ConversionType::ToDec => {
            if let Some(v) = params.value {
                cmd.arg(v);
            }
            if let Some(base_in) = params.base_in {
                cmd.arg("--base-in").arg(base_in);
            }
        }
        ConversionType::ToBase => {
            if let Some(v) = params.value {
                cmd.arg(v);
            }
            if let Some(b) = params.base {
                cmd.arg(b);
            }
            if let Some(base_in) = params.base_in {
                cmd.arg("--base-in").arg(base_in);
            }
        }
        ConversionType::FromRlp => {
            if let Some(v) = params.value {
                cmd.arg(v);
            }
            if params.as_int.unwrap_or(false) {
                cmd.arg("--as-int");
            }
        }
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to execute cast {}", conversion_type.subcommand()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr).trim().to_string();

    if output.status.success() {
        Ok(combined)
    } else {
        anyhow::bail!("Conversion failed: {}", combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion_type_serialization() {
        let types = vec![
            ("max-int", ConversionType::MaxInt),
            ("from-utf8", ConversionType::FromUtf8),
            ("to-hex", ConversionType::ToHex),
            ("shl", ConversionType::Shl),
        ];

        for (name, expected) in types {
            let json = format!("\"{}\"", name);
            let parsed: ConversionType = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", parsed), format!("{:?}", expected));
        }
    }

    #[test]
    fn test_subcommand_names() {
        assert_eq!(ConversionType::MaxInt.subcommand(), "max-int");
        assert_eq!(ConversionType::FromUtf8.subcommand(), "from-utf8");
        assert_eq!(
            ConversionType::ToCheckSumAddress.subcommand(),
            "to-check-sum-address"
        );
        assert_eq!(ConversionType::Shl.subcommand(), "shl");
    }
}
