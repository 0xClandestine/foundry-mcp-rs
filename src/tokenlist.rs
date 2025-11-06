//! Token list integration for Ethereum and L2 chains
//!
//! This module provides comprehensive token discovery capabilities via the Optimism
//! token list, including token search, address lookup, and multi-chain support.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use rmcp::model::{CallToolResult, Content, Tool};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Token list standard format (EIP-3770)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenList {
    pub name: String,
    pub version: TokenListVersion,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub tokens: Vec<TokenInfo>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub logo_uri: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenListVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Token information from token list
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub chain_id: u64,
    pub address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    #[serde(default)]
    pub logo_uri: Option<String>,
    #[serde(default)]
    pub extensions: Option<HashMap<String, Value>>,
}

/// Supported chain identifiers
const SUPPORTED_CHAINS: &[(&str, u64)] = &[
    ("ethereum", 1),
    ("optimism", 10),
    ("sepolia", 11155111),
    ("base", 8453),
    ("base-sepolia", 84532),
    ("optimism-sepolia", 11155420),
    ("mode", 34443),
    ("lisk", 1135),
    ("lisk-sepolia", 4202),
    ("redstone", 690),
    ("metal-l2", 1750),
    ("metal-l2-sepolia", 1740),
    ("celo", 42220),
    ("celo-sepolia", 44787),
];

/// Get chain ID from chain name
pub fn chain_name_to_id(name: &str) -> Option<u64> {
    SUPPORTED_CHAINS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, id)| *id)
}

/// Get chain name from chain ID
pub fn chain_id_to_name(id: u64) -> Option<&'static str> {
    SUPPORTED_CHAINS
        .iter()
        .find(|(_, chain_id)| *chain_id == id)
        .map(|(name, _)| *name)
}

/// Global cache for tokenlist data
static TOKENLIST_CACHE: Lazy<Mutex<Option<TokenList>>> = Lazy::new(|| Mutex::new(None));

/// Fetches and caches token data from the Optimism token list
pub async fn fetch_tokenlist() -> Result<TokenList> {
    // Check cache first
    {
        let cache = TOKENLIST_CACHE.lock().unwrap();
        if let Some(ref cached) = *cache {
            return Ok(cached.clone());
        }
    }

    // Fetch from GitHub
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("foundry-mcp-rs")
        .build()?;

    let response = client
        .get("https://raw.githubusercontent.com/ethereum-optimism/ethereum-optimism.github.io/master/optimism.tokenlist.json")
        .send()
        .await?;

    // Get the response text for better error handling
    let text = response.text().await?;

    // Try to parse the JSON
    let tokenlist: TokenList = serde_json::from_str(&text)
        .context("Failed to parse token list response. This might be due to API format changes.")?;

    // Update cache
    {
        let mut cache = TOKENLIST_CACHE.lock().unwrap();
        *cache = Some(tokenlist.clone());
    }

    Ok(tokenlist)
}

/// Clear the tokenlist cache to force a refresh
pub fn clear_cache() {
    let mut cache = TOKENLIST_CACHE.lock().unwrap();
    *cache = None;
}

/// Find token by address on a specific chain
pub fn find_token_by_address<'a>(
    tokens: &'a [TokenInfo],
    address: &str,
    chain_id: Option<u64>,
) -> Vec<&'a TokenInfo> {
    let address_lower = address.to_lowercase();
    tokens
        .iter()
        .filter(|t| {
            let matches_address = t.address.to_lowercase() == address_lower;
            let matches_chain = chain_id.is_none_or(|id| t.chain_id == id);
            matches_address && matches_chain
        })
        .collect()
}

/// Search for tokens by name or symbol
pub fn search_tokens<'a>(
    tokens: &'a [TokenInfo],
    query: &str,
    chain_id: Option<u64>,
) -> Vec<&'a TokenInfo> {
    let query_lower = query.to_lowercase();

    // Try exact matches first
    let mut exact_matches: Vec<&TokenInfo> = tokens
        .iter()
        .filter(|t| {
            let matches_query =
                t.symbol.to_lowercase() == query_lower || t.name.to_lowercase() == query_lower;
            let matches_chain = chain_id.is_none_or(|id| t.chain_id == id);
            matches_query && matches_chain
        })
        .collect();

    // If we have exact matches, return those
    if !exact_matches.is_empty() {
        exact_matches.truncate(50);
        return exact_matches;
    }

    // Otherwise do partial matches
    let mut partial_matches: Vec<&TokenInfo> = tokens
        .iter()
        .filter(|t| {
            let matches_query = t.symbol.to_lowercase().contains(&query_lower)
                || t.name.to_lowercase().contains(&query_lower);
            let matches_chain = chain_id.is_none_or(|id| t.chain_id == id);
            matches_query && matches_chain
        })
        .collect();

    partial_matches.truncate(50);
    partial_matches
}

/// Get all tokens for a specific chain
pub fn get_tokens_by_chain(tokens: &[TokenInfo], chain_id: u64) -> Vec<&TokenInfo> {
    tokens.iter().filter(|t| t.chain_id == chain_id).collect()
}

/// Format token information as a string
pub fn format_token_info(token: &TokenInfo, show_chain: bool) -> String {
    let mut info = format!(
        "• {} ({})\n  Address: {}\n  Decimals: {}\n",
        token.name, token.symbol, token.address, token.decimals
    );

    if show_chain {
        if let Some(chain_name) = chain_id_to_name(token.chain_id) {
            info.push_str(&format!(
                "  Chain: {} (ID: {})\n",
                chain_name, token.chain_id
            ));
        } else {
            info.push_str(&format!("  Chain ID: {}\n", token.chain_id));
        }
    }

    if let Some(logo_uri) = &token.logo_uri {
        info.push_str(&format!("  Logo: {}\n", logo_uri));
    }

    if let Some(extensions) = &token.extensions {
        if let Some(bridge_info) = extensions.get("bridgeInfo") {
            info.push_str("  Bridge Info: Available\n");
            if let Some(obj) = bridge_info.as_object() {
                for (chain, data) in obj {
                    if let Some(token_address) = data.get("tokenAddress") {
                        info.push_str(&format!(
                            "    {} → {}\n",
                            chain,
                            token_address.as_str().unwrap_or("N/A")
                        ));
                    }
                }
            }
        }
    }

    info
}

/// Get tokenlist MCP tools
pub fn get_tokenlist_tools() -> Vec<Tool> {
    vec![
        // search_tokens tool
        Tool::new(
            "search_tokens".to_string(),
            "Search for tokens by name or symbol across all supported chains. Returns token information including addresses, decimals, and bridge info.".to_string(),
            Arc::new({
                let mut props = serde_json::Map::new();
                props.insert("query".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "Token name or symbol to search for (e.g., 'USDC', 'Ethereum')"
                }));
                props.insert("chain".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "Optional: Filter by chain name or ID (e.g., 'ethereum', 'optimism', '10')"
                }));

                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                schema.insert("properties".to_string(), Value::Object(props));
                schema.insert("required".to_string(), Value::Array(vec![Value::String("query".to_string())]));
                schema
            }),
        ),
        // get_token_by_address tool
        Tool::new(
            "get_token_by_address".to_string(),
            "Get token information by contract address. Supports searching across all chains or filtering by specific chain.".to_string(),
            Arc::new({
                let mut props = serde_json::Map::new();
                props.insert("address".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "Token contract address (with or without 0x prefix)"
                }));
                props.insert("chain".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "Optional: Chain name or ID to search on (e.g., 'ethereum', 'optimism', '10')"
                }));

                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                schema.insert("properties".to_string(), Value::Object(props));
                schema.insert("required".to_string(), Value::Array(vec![Value::String("address".to_string())]));
                schema
            }),
        ),
        // list_chain_tokens tool
        Tool::new(
            "list_chain_tokens".to_string(),
            "List all tokens available on a specific blockchain network.".to_string(),
            Arc::new({
                let mut props = serde_json::Map::new();
                props.insert("chain".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "Chain name or ID (e.g., 'ethereum', 'optimism', '10', 'base')"
                }));
                props.insert("limit".to_string(), serde_json::json!({
                    "type": "number",
                    "description": "Maximum number of tokens to return (default: 50)"
                }));

                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                schema.insert("properties".to_string(), Value::Object(props));
                schema.insert("required".to_string(), Value::Array(vec![Value::String("chain".to_string())]));
                schema
            }),
        ),
        // list_supported_chains tool
        Tool::new(
            "list_supported_chains".to_string(),
            "List all blockchain networks supported by the token list with their chain IDs.".to_string(),
            Arc::new({
                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                schema.insert("properties".to_string(), Value::Object(serde_json::Map::new()));
                schema
            }),
        ),
    ]
}

/// Parse chain parameter (name or ID) to chain ID
fn parse_chain_param(chain_str: &str) -> Option<u64> {
    // Try parsing as number first
    if let Ok(id) = chain_str.parse::<u64>() {
        return Some(id);
    }
    // Try as chain name
    chain_name_to_id(chain_str)
}

/// Handle search_tokens tool call
pub async fn handle_search_tokens(
    args: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
        rmcp::ErrorData::invalid_params("Missing or invalid 'query' parameter", None)
    })?;

    let chain_id = args
        .get("chain")
        .and_then(|v| v.as_str())
        .and_then(parse_chain_param);

    // Fetch token data
    let tokenlist = fetch_tokenlist().await.map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Failed to fetch token list: {}", e), None)
    })?;

    // Search tokens
    let results = search_tokens(&tokenlist.tokens, query, chain_id);

    // Build response
    let mut response = if let Some(cid) = chain_id {
        let chain_name = chain_id_to_name(cid).unwrap_or("unknown");
        format!(
            "Found {} tokens matching '{}' on {}\n\n",
            results.len(),
            query,
            chain_name
        )
    } else {
        format!(
            "Found {} tokens matching '{}' across all chains\n\n",
            results.len(),
            query
        )
    };

    if results.is_empty() {
        response
            .push_str("No tokens found. Try a different search term or check the chain filter.\n");
    } else {
        for token in results {
            response.push_str(&format_token_info(token, chain_id.is_none()));
            response.push('\n');
        }
    }

    Ok(CallToolResult::success(vec![Content::text(response)]))
}

/// Handle get_token_by_address tool call
pub async fn handle_get_token_by_address(
    args: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let address = args
        .get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            rmcp::ErrorData::invalid_params("Missing or invalid 'address' parameter", None)
        })?;

    // Normalize address (remove 0x if present, then add it back)
    let normalized_address = if address.starts_with("0x") {
        address.to_string()
    } else {
        format!("0x{}", address)
    };

    let chain_id = args
        .get("chain")
        .and_then(|v| v.as_str())
        .and_then(parse_chain_param);

    // Fetch token data
    let tokenlist = fetch_tokenlist().await.map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Failed to fetch token list: {}", e), None)
    })?;

    // Find token by address
    let results = find_token_by_address(&tokenlist.tokens, &normalized_address, chain_id);

    // Build response
    let mut response = if results.is_empty() {
        format!("No token found with address {}\n", normalized_address)
    } else if results.len() == 1 {
        format!("Token found:\n\n{}", format_token_info(results[0], true))
    } else {
        let mut resp = format!(
            "Found {} tokens with address {} on different chains:\n\n",
            results.len(),
            normalized_address
        );
        for token in &results {
            resp.push_str(&format_token_info(token, true));
            resp.push('\n');
        }
        resp
    };

    if results.is_empty() {
        response
            .push_str("\nTip: Make sure the address is correct and exists in the token list.\n");
    }

    Ok(CallToolResult::success(vec![Content::text(response)]))
}

/// Handle list_chain_tokens tool call
pub async fn handle_list_chain_tokens(
    args: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let chain_str = args.get("chain").and_then(|v| v.as_str()).ok_or_else(|| {
        rmcp::ErrorData::invalid_params("Missing or invalid 'chain' parameter", None)
    })?;

    let chain_id = parse_chain_param(chain_str).ok_or_else(|| {
        rmcp::ErrorData::invalid_params(
            format!(
                "Invalid chain '{}'. Use chain name (e.g., 'ethereum', 'optimism') or chain ID",
                chain_str
            ),
            None,
        )
    })?;

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .min(200) as usize;

    // Fetch token data
    let tokenlist = fetch_tokenlist().await.map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Failed to fetch token list: {}", e), None)
    })?;

    // Get tokens for chain
    let tokens = get_tokens_by_chain(&tokenlist.tokens, chain_id);

    // Build response
    let chain_name = chain_id_to_name(chain_id).unwrap_or("Unknown");
    let mut response = format!(
        "Found {} tokens on {} (Chain ID: {})\n\n",
        tokens.len(),
        chain_name,
        chain_id
    );

    if tokens.is_empty() {
        response.push_str("No tokens found for this chain.\n");
    } else {
        let display_tokens = tokens.iter().take(limit);
        for token in display_tokens {
            response.push_str(&format_token_info(token, false));
            response.push('\n');
        }

        if tokens.len() > limit {
            response.push_str(&format!(
                "\n... and {} more tokens. Increase the limit parameter to see more.\n",
                tokens.len() - limit
            ));
        }
    }

    Ok(CallToolResult::success(vec![Content::text(response)]))
}

/// Handle list_supported_chains tool call
pub async fn handle_list_supported_chains(
    _args: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let mut response = String::from("Supported Chains:\n\n");

    for (name, chain_id) in SUPPORTED_CHAINS {
        response.push_str(&format!("• {} - Chain ID: {}\n", name, chain_id));
    }

    response.push_str("\nUse these chain names or IDs with search_tokens, get_token_by_address, or list_chain_tokens.\n");

    Ok(CallToolResult::success(vec![Content::text(response)]))
}
