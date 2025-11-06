//! Chainlist.org API integration for blockchain RPC discovery
//!
//! This module provides comprehensive blockchain RPC discovery capabilities via chainlist.org,
//! including chain search, RPC filtering, and network information.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use rmcp::model::{CallToolResult, Content, Tool};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// RPC endpoint information from chainlist.org
/// Can be either a string URL or an object with metadata
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RpcEntry {
    String(String),
    Object {
        url: String,
        #[serde(default)]
        tracking: Option<String>,
        #[serde(rename = "isOpenSource", default)]
        is_open_source: Option<bool>,
    },
}

impl RpcEntry {
    pub fn url(&self) -> &str {
        match self {
            RpcEntry::String(s) => s,
            RpcEntry::Object { url, .. } => url,
        }
    }

    pub fn tracking(&self) -> Option<&String> {
        match self {
            RpcEntry::String(_) => None,
            RpcEntry::Object { tracking, .. } => tracking.as_ref(),
        }
    }

    pub fn is_open_source(&self) -> Option<bool> {
        match self {
            RpcEntry::String(_) => None,
            RpcEntry::Object { is_open_source, .. } => *is_open_source,
        }
    }
}

/// Helper function to deserialize faucets which can be a string or array
fn deserialize_faucets<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match StringOrVec::deserialize(deserializer) {
        Ok(StringOrVec::String(s)) => Ok(vec![s]),
        Ok(StringOrVec::Vec(v)) => Ok(v),
        Err(_) => Ok(Vec::new()),
    }
}

/// Chain information from chainlist.org
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainInfo {
    pub name: String,
    pub chain: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "networkId", default)]
    pub network_id: Option<u64>,
    #[serde(default)]
    pub rpc: Vec<RpcEntry>,
    #[serde(deserialize_with = "deserialize_faucets", default)]
    pub faucets: Vec<String>,
    #[serde(rename = "nativeCurrency", default)]
    pub native_currency: Option<serde_json::Value>,
    #[serde(rename = "infoURL", default)]
    pub info_url: Option<String>,
    #[serde(rename = "shortName")]
    pub short_name: String,
    #[serde(default)]
    pub explorers: Vec<serde_json::Value>,
    // Additional optional fields that might be present
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub testnet: Option<bool>,
    #[serde(default)]
    pub features: Vec<serde_json::Value>,
    // Catch any other fields we don't know about
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Global cache for chainlist data
static CHAINLIST_CACHE: Lazy<Mutex<Option<Vec<ChainInfo>>>> = Lazy::new(|| Mutex::new(None));

/// Fetches and caches chain data from chainlist.org
pub async fn fetch_chainlist() -> Result<Vec<ChainInfo>> {
    // Check cache first
    {
        let cache = CHAINLIST_CACHE.lock().unwrap();
        if let Some(ref cached) = *cache {
            return Ok(cached.clone());
        }
    }

    // Fetch from API
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client.get("https://chainlist.org/rpcs.json").send().await?;

    // Get the response text for better error handling
    let text = response.text().await?;

    // Try to parse the JSON
    let chains: Vec<ChainInfo> = serde_json::from_str(&text).context(
        "Failed to parse chainlist.org response. This might be due to API format changes.",
    )?;

    // Update cache
    {
        let mut cache = CHAINLIST_CACHE.lock().unwrap();
        *cache = Some(chains.clone());
    }

    Ok(chains)
}

/// Find RPC URLs for a specific chain by ID or name
pub fn find_chain_rpcs<'a>(chains: &'a [ChainInfo], query: &str) -> Option<&'a ChainInfo> {
    // Try parsing as chain ID first
    if let Ok(chain_id) = query.parse::<u64>() {
        if let Some(chain) = chains.iter().find(|c| c.chain_id == chain_id) {
            return Some(chain);
        }
    }

    let query_lower = query.to_lowercase();

    // Try exact match first
    if let Some(chain) = chains.iter().find(|c| {
        c.name.to_lowercase() == query_lower
            || c.short_name.to_lowercase() == query_lower
            || c.chain.to_lowercase() == query_lower
    }) {
        return Some(chain);
    }

    // Try partial match as fallback
    chains.iter().find(|c| {
        c.name.to_lowercase().contains(&query_lower)
            || c.short_name.to_lowercase().contains(&query_lower)
            || c.chain.to_lowercase().contains(&query_lower)
    })
}

/// Search for chains matching a query string
pub fn search_chains<'a>(chains: &'a [ChainInfo], query: &str) -> Vec<&'a ChainInfo> {
    let query_lower = query.to_lowercase();
    chains
        .iter()
        .filter(|c| {
            c.name.to_lowercase().contains(&query_lower)
                || c.short_name.to_lowercase().contains(&query_lower)
                || c.chain.to_lowercase().contains(&query_lower)
                || c.chain_id.to_string().contains(&query_lower)
        })
        .take(50) // Limit results
        .collect()
}

/// Clear the chainlist cache to force a refresh
pub fn clear_cache() {
    let mut cache = CHAINLIST_CACHE.lock().unwrap();
    *cache = None;
}

/// RPC filter options
#[derive(Debug, Clone, Default)]
pub struct RpcFilter {
    pub no_tracking: bool,
    pub prefer_open_source: bool,
    pub websocket_only: bool,
    pub http_only: bool,
}

/// Filter and sort RPC endpoints based on preferences
pub fn filter_and_sort_rpcs(rpcs: &[RpcEntry], filter: &RpcFilter) -> Vec<RpcEntry> {
    let mut filtered: Vec<RpcEntry> = rpcs
        .iter()
        .filter(|rpc| {
            // Filter by tracking
            if filter.no_tracking && rpc.tracking().is_none_or(|t| t != "none") {
                return false;
            }

            // Filter by protocol
            let url = rpc.url();
            if filter.websocket_only && !url.starts_with("wss://") && !url.starts_with("ws://") {
                return false;
            }
            if filter.http_only && !url.starts_with("https://") && !url.starts_with("http://") {
                return false;
            }

            true
        })
        .cloned()
        .collect();

    // Sort by preference
    if filter.prefer_open_source {
        filtered.sort_by_key(|rpc| {
            (
                !rpc.is_open_source().unwrap_or(false),
                rpc.tracking().map_or(0, |t| match t.as_str() {
                    "none" => 0,
                    "limited" => 1,
                    _ => 2,
                }),
            )
        });
    }

    filtered
}

/// Format chain information as a string
pub fn format_chain_info(chain: &ChainInfo, rpcs: &[RpcEntry], limit: Option<usize>) -> String {
    let mut response = format!(
        "Chain: {} ({})\nChain ID: {}\nShort Name: {}\n",
        chain.name, chain.chain, chain.chain_id, chain.short_name
    );

    if let Some(testnet) = chain.testnet {
        response.push_str(&format!("Testnet: {}\n", testnet));
    }

    if let Some(info_url) = &chain.info_url {
        response.push_str(&format!("Info: {}\n", info_url));
    }

    response.push('\n');
    response.push_str("RPC Endpoints:\n");

    if rpcs.is_empty() {
        response.push_str("  No RPC endpoints found matching the criteria.\n");
    } else {
        let display_rpcs = if let Some(lim) = limit {
            &rpcs[..rpcs.len().min(lim)]
        } else {
            rpcs
        };

        for (i, rpc) in display_rpcs.iter().enumerate() {
            response.push_str(&format!("{}. {}\n", i + 1, rpc.url()));

            let mut details = Vec::new();
            if let Some(tracking) = rpc.tracking() {
                details.push(format!("tracking: {}", tracking));
            }
            if let Some(is_open_source) = rpc.is_open_source() {
                if is_open_source {
                    details.push("open-source".to_string());
                }
            }

            if !details.is_empty() {
                response.push_str(&format!("   [{}]\n", details.join(", ")));
            }
        }

        if let Some(lim) = limit {
            if rpcs.len() > lim {
                response.push_str(&format!("\n... and {} more\n", rpcs.len() - lim));
            }
        }
    }

    if !chain.faucets.is_empty() {
        response.push_str("\nFaucets:\n");
        for faucet in &chain.faucets {
            response.push_str(&format!("  - {}\n", faucet));
        }
    }

    if !chain.explorers.is_empty() {
        response.push_str("\nExplorers:\n");
        for explorer in &chain.explorers {
            if let Some(obj) = explorer.as_object() {
                if let (Some(name), Some(url)) = (obj.get("name"), obj.get("url")) {
                    response.push_str(&format!(
                        "  - {}: {}\n",
                        name.as_str().unwrap_or("Unknown"),
                        url.as_str().unwrap_or("")
                    ));
                }
            }
        }
    }

    response
}

/// Get chainlist MCP tools
pub fn get_chainlist_tools() -> Vec<Tool> {
    vec![
        // search_rpc_url tool
        Tool::new(
            "search_rpc_url".to_string(),
            "Search for RPC endpoints for a specific network. Query by chain ID (e.g., '1' for Ethereum) or name (e.g., 'ethereum', 'polygon', 'arbitrum'). Returns available RPC URLs with tracking and open-source information.".to_string(),
            Arc::new({
                let mut props = serde_json::Map::new();
                props.insert("chain".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "Chain ID or name (e.g., '1', 'ethereum', 'polygon')"
                }));
                props.insert("prefer_open_source".to_string(), serde_json::json!({
                    "type": "boolean",
                    "description": "Prefer open-source RPC endpoints (default: true)"
                }));
                props.insert("no_tracking".to_string(), serde_json::json!({
                    "type": "boolean",
                    "description": "Only return RPC endpoints with no tracking (default: false)"
                }));
                props.insert("websocket_only".to_string(), serde_json::json!({
                    "type": "boolean",
                    "description": "Only return WebSocket RPC endpoints (default: false)"
                }));
                props.insert("http_only".to_string(), serde_json::json!({
                    "type": "boolean",
                    "description": "Only return HTTP/HTTPS RPC endpoints (default: false)"
                }));
                props.insert("limit".to_string(), serde_json::json!({
                    "type": "number",
                    "description": "Maximum number of RPC endpoints to return"
                }));

                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                schema.insert("properties".to_string(), Value::Object(props));
                schema.insert("required".to_string(), Value::Array(vec![Value::String("chain".to_string())]));
                schema
            }),
        ),
        // search_chains tool
        Tool::new(
            "search_chains".to_string(),
            "Search for blockchain networks by name, symbol, or chain ID. Returns a list of matching chains with their basic information.".to_string(),
            Arc::new({
                let mut props = serde_json::Map::new();
                props.insert("query".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "Search query (name, symbol, or chain ID)"
                }));
                props.insert("testnet_only".to_string(), serde_json::json!({
                    "type": "boolean",
                    "description": "Only return testnets (default: false)"
                }));
                props.insert("mainnet_only".to_string(), serde_json::json!({
                    "type": "boolean",
                    "description": "Only return mainnets (default: false)"
                }));

                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                schema.insert("properties".to_string(), Value::Object(props));
                schema.insert("required".to_string(), Value::Array(vec![Value::String("query".to_string())]));
                schema
            }),
        ),
        // list_popular_chains tool
        Tool::new(
            "list_popular_chains".to_string(),
            "List popular blockchain networks with their chain IDs and basic information. Useful for discovering available networks.".to_string(),
            Arc::new({
                let mut schema = serde_json::Map::new();
                schema.insert("type".to_string(), Value::String("object".to_string()));
                schema.insert("properties".to_string(), Value::Object(serde_json::Map::new()));
                schema
            }),
        ),
    ]
}

/// Handle search_rpc_url tool call
pub async fn handle_search_rpc_url(
    args: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let chain = args.get("chain").and_then(|v| v.as_str()).ok_or_else(|| {
        rmcp::ErrorData::invalid_params("Missing or invalid 'chain' parameter", None)
    })?;

    let filter = RpcFilter {
        prefer_open_source: args
            .get("prefer_open_source")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        no_tracking: args
            .get("no_tracking")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        websocket_only: args
            .get("websocket_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        http_only: args
            .get("http_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    // Fetch chain data
    let chains = fetch_chainlist().await.map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Failed to fetch chainlist data: {}", e), None)
    })?;

    // Find the requested chain
    let chain_info = find_chain_rpcs(&chains, chain).ok_or_else(|| {
        rmcp::ErrorData::invalid_params(
            format!(
                "Chain '{}' not found. Try using chain ID (e.g., '1' for Ethereum) or common names like 'ethereum', 'polygon', 'arbitrum'",
                chain
            ),
            None,
        )
    })?;

    // Filter and sort RPCs
    let rpcs = filter_and_sort_rpcs(&chain_info.rpc, &filter);

    // Format response
    let response = format_chain_info(chain_info, &rpcs, limit);

    Ok(CallToolResult::success(vec![Content::text(response)]))
}

/// Handle search_chains tool call
pub async fn handle_search_chains(
    args: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
        rmcp::ErrorData::invalid_params("Missing or invalid 'query' parameter", None)
    })?;

    let testnet_only = args
        .get("testnet_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mainnet_only = args
        .get("mainnet_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Fetch chain data
    let chains = fetch_chainlist().await.map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Failed to fetch chainlist data: {}", e), None)
    })?;

    // Search chains
    let mut results = search_chains(&chains, query);

    // Apply filters
    if testnet_only {
        results.retain(|c| c.testnet == Some(true));
    }
    if mainnet_only {
        results.retain(|c| c.testnet != Some(true));
    }

    // Build response
    let mut response = format!("Found {} chains matching '{}'\n\n", results.len(), query);

    for chain in results {
        response.push_str(&format!(
            "• {} ({})\n  Chain ID: {}\n  Short Name: {}\n",
            chain.name, chain.chain, chain.chain_id, chain.short_name
        ));
        if let Some(testnet) = chain.testnet {
            if testnet {
                response.push_str("  Type: Testnet\n");
            }
        }
        response.push_str(&format!("  RPCs: {}\n\n", chain.rpc.len()));
    }

    Ok(CallToolResult::success(vec![Content::text(response)]))
}

/// Handle list_popular_chains tool call
pub async fn handle_list_popular_chains(
    _args: &serde_json::Map<String, Value>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    // Fetch chain data
    let chains = fetch_chainlist().await.map_err(|e| {
        rmcp::ErrorData::internal_error(format!("Failed to fetch chainlist data: {}", e), None)
    })?;

    // Popular chain IDs
    let popular_ids = vec![
        1, 10, 137, 42161, 8453, 43114, 56, 250, 100, 324, 1101, 59144, 534352,
    ];

    let mut response = String::from("Popular Blockchain Networks:\n\n");

    for id in popular_ids {
        if let Some(chain) = chains.iter().find(|c| c.chain_id == id) {
            response.push_str(&format!(
                "• {} ({})\n  Chain ID: {}\n  Short Name: {}\n  RPCs: {}\n\n",
                chain.name,
                chain.chain,
                chain.chain_id,
                chain.short_name,
                chain.rpc.len()
            ));
        }
    }

    response.push_str("Use 'search_chains' to find more networks or 'search_rpc_url' to get RPC endpoints for a specific chain.\n");

    Ok(CallToolResult::success(vec![Content::text(response)]))
}
