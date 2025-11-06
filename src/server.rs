//! MCP server handler implementation

use anyhow::Result;
use rmcp::{
    model::*,
    service::{RequestContext, RoleServer},
    ErrorData as McpError, ServerHandler,
};
use std::sync::Arc;

use crate::chainlist::{self, fetch_chainlist};
use crate::foundry::FoundryExecutor;
use crate::tokenlist;

/// MCP server handler
#[derive(Clone)]
pub struct FoundryMcpHandler {
    foundry: Arc<FoundryExecutor>,
}

impl FoundryMcpHandler {
    pub fn new(foundry: FoundryExecutor) -> Self {
        Self {
            foundry: Arc::new(foundry),
        }
    }

    pub fn foundry_bin_path(&self) -> &Option<String> {
        self.foundry.foundry_bin_path()
    }
}

impl ServerHandler for FoundryMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                prompts: None,
                resources: Some(ResourcesCapability {
                    subscribe: None,
                    list_changed: None,
                }),
                tools: Some(ToolsCapability {
                    list_changed: None,
                }),
                logging: None,
                completions: None,
                experimental: None,
            },
            server_info: Implementation {
                name: "foundry-mcp-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Foundry MCP Server".to_string()),
                icons: None,
                website_url: Some("https://github.com/foundry-rs/foundry".to_string()),
            },
            instructions: Some("MCP server providing access to Foundry CLI tools (forge, cast, anvil, chisel), blockchain RPC endpoints via chainlist.org, and token information via the Optimism token list".into()),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = self.foundry.tool_list().to_vec();

        // Add chainlist tools
        tools.extend(chainlist::get_chainlist_tools());

        // Add tokenlist tools
        tools.extend(tokenlist::get_tokenlist_tools());

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let mut chainlist_resource = RawResource::new("chainlist://all", "All Blockchain Networks");
        chainlist_resource.description = Some(
            "Complete list of all blockchain networks and their RPC endpoints from chainlist.org"
                .to_string(),
        );
        chainlist_resource.mime_type = Some("application/json".to_string());

        let mut tokenlist_resource = RawResource::new("tokenlist://all", "All ERC20 Tokens");
        tokenlist_resource.description = Some(
            "Complete list of ERC20 tokens across Ethereum and L2 networks from the Optimism token list"
                .to_string(),
        );
        tokenlist_resource.mime_type = Some("application/json".to_string());

        let resources = vec![
            chainlist_resource.no_annotation(),
            tokenlist_resource.no_annotation(),
        ];

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match request.uri.as_str() {
            "chainlist://all" => match fetch_chainlist().await {
                Ok(chains) => {
                    let json = serde_json::to_string_pretty(&chains)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                    Ok(ReadResourceResult {
                        contents: vec![ResourceContents::TextResourceContents {
                            uri: request.uri,
                            mime_type: Some("application/json".to_string()),
                            text: json,
                            meta: None,
                        }],
                    })
                }
                Err(e) => Err(McpError::internal_error(
                    format!("Failed to fetch chainlist data: {}", e),
                    None,
                )),
            },
            "tokenlist://all" => match tokenlist::fetch_tokenlist().await {
                Ok(tokens) => {
                    let json = serde_json::to_string_pretty(&tokens)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                    Ok(ReadResourceResult {
                        contents: vec![ResourceContents::TextResourceContents {
                            uri: request.uri,
                            mime_type: Some("application/json".to_string()),
                            text: json,
                            meta: None,
                        }],
                    })
                }
                Err(e) => Err(McpError::internal_error(
                    format!("Failed to fetch token list: {}", e),
                    None,
                )),
            },
            _ => Err(McpError::invalid_params(
                format!("Unknown resource URI: {}", request.uri),
                None,
            )),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        // Handle chainlist tools
        let tool_name: &str = &request.name;
        match tool_name {
            "search_rpc_url" => {
                let args = request
                    .arguments
                    .as_ref()
                    .ok_or_else(|| McpError::invalid_params("Missing arguments", None))?;
                return chainlist::handle_search_rpc_url(args).await;
            }
            "search_chains" => {
                let args = request
                    .arguments
                    .as_ref()
                    .ok_or_else(|| McpError::invalid_params("Missing arguments", None))?;
                return chainlist::handle_search_chains(args).await;
            }
            "list_popular_chains" => {
                let empty_map = serde_json::Map::new();
                let args = request.arguments.as_ref().unwrap_or(&empty_map);
                return chainlist::handle_list_popular_chains(args).await;
            }
            // Handle tokenlist tools
            "search_tokens" => {
                let args = request
                    .arguments
                    .as_ref()
                    .ok_or_else(|| McpError::invalid_params("Missing arguments", None))?;
                return tokenlist::handle_search_tokens(args).await;
            }
            "get_token_by_address" => {
                let args = request
                    .arguments
                    .as_ref()
                    .ok_or_else(|| McpError::invalid_params("Missing arguments", None))?;
                return tokenlist::handle_get_token_by_address(args).await;
            }
            "list_chain_tokens" => {
                let args = request
                    .arguments
                    .as_ref()
                    .ok_or_else(|| McpError::invalid_params("Missing arguments", None))?;
                return tokenlist::handle_list_chain_tokens(args).await;
            }
            "list_supported_chains" => {
                let empty_map = serde_json::Map::new();
                let args = request.arguments.as_ref().unwrap_or(&empty_map);
                return tokenlist::handle_list_supported_chains(args).await;
            }
            _ => {}
        }

        // Handle Foundry tools (sync)
        match self.foundry.execute_tool(&request.name, &request.arguments) {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}
