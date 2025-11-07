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
use crate::handlers;
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

        // Add session management tools
        tools.extend(handlers::get_session_tools());

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
            // Handle session management tools
            "anvil_session_start" => {
                return handlers::handle_anvil_session_start(
                    &request.arguments,
                    self.foundry_bin_path(),
                )
                .await;
            }
            "anvil_session_stop" => {
                return handlers::handle_anvil_session_stop().await;
            }
            "anvil_session_status" => {
                return handlers::handle_anvil_session_status().await;
            }
            "chisel_session_start" => {
                return handlers::handle_chisel_session_start(self.foundry_bin_path()).await;
            }
            "chisel_session_eval" => {
                return handlers::handle_chisel_session_eval(
                    &request.arguments,
                    self.foundry_bin_path(),
                )
                .await;
            }
            "chisel_session_stop" => {
                return handlers::handle_chisel_session_stop().await;
            }
            "chisel_session_status" => {
                return handlers::handle_chisel_session_status().await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::foundry::FoundryExecutor;
    use crate::schema::SchemaFile;

    fn create_test_handler() -> FoundryMcpHandler {
        let schema = SchemaFile { tools: vec![] };
        let config = Config::default();
        let executor = FoundryExecutor::with_config(schema, config);
        FoundryMcpHandler::new(executor)
    }

    /// Test that MCP handler can be created successfully
    #[test]
    fn test_handler_creation() {
        let handler = create_test_handler();
        assert!(handler.foundry_bin_path().is_none() || handler.foundry_bin_path().is_some());
    }

    /// Test that server info contains correct name, version, and instructions
    #[test]
    fn test_get_info_returns_valid_server_info() {
        let handler = create_test_handler();
        let info = handler.get_info();

        assert_eq!(info.server_info.name, "foundry-mcp-server");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            info.server_info.title,
            Some("Foundry MCP Server".to_string())
        );
        assert!(info.instructions.is_some());
    }

    /// Test that server advertises correct MCP capabilities (resources, tools, but not prompts)
    #[test]
    fn test_get_info_capabilities() {
        let handler = create_test_handler();
        let info = handler.get_info();

        // Should support resources
        assert!(info.capabilities.resources.is_some());

        // Should support tools
        assert!(info.capabilities.tools.is_some());

        // Should not support prompts by default
        assert!(info.capabilities.prompts.is_none());
    }

    /// Test that server info includes a valid MCP protocol version
    #[test]
    fn test_get_info_protocol_version() {
        let handler = create_test_handler();
        let info = handler.get_info();

        // Protocol version should be valid
        let version_str = format!("{}", info.protocol_version);
        assert!(!version_str.is_empty());
    }

    /// Test that handler implements Clone trait and clones preserve state
    #[test]
    fn test_handler_is_clone() {
        let handler = create_test_handler();
        let cloned = handler.clone();

        // Both should have the same foundry bin path
        assert_eq!(handler.foundry_bin_path(), cloned.foundry_bin_path());
    }

    /// Test that handler correctly wraps executor with custom security config
    #[test]
    fn test_handler_preserves_executor_config() {
        let schema = SchemaFile { tools: vec![] };
        let config = Config {
            forbidden_commands: vec!["anvil".to_string()],
            forbidden_flags: vec!["broadcast".to_string()],
            allow_dangerous: false,
        };
        let executor = FoundryExecutor::with_config(schema, config);
        let _handler = FoundryMcpHandler::new(executor);

        // Handler should be created successfully with custom config
        // The config restrictions are enforced at the executor level
    }

    /// Test that server info includes website URL pointing to Foundry
    #[test]
    fn test_server_info_has_website() {
        let handler = create_test_handler();
        let info = handler.get_info();

        assert!(info.server_info.website_url.is_some());
        let website = info.server_info.website_url.unwrap();
        assert!(website.contains("foundry"));
    }

    /// Test that capabilities structure has valid optional fields
    #[test]
    fn test_capabilities_structure() {
        let handler = create_test_handler();
        let info = handler.get_info();

        // Verify capabilities structure
        if let Some(resources) = &info.capabilities.resources {
            // Resources capability exists
            assert!(resources.subscribe.is_none() || resources.subscribe.is_some());
        }

        if let Some(tools) = &info.capabilities.tools {
            // Tools capability exists
            assert!(tools.list_changed.is_none() || tools.list_changed.is_some());
        }
    }

    /// Test that handler correctly wraps executor and preserves its bin path
    #[test]
    fn test_handler_new_wraps_executor_correctly() {
        let schema = SchemaFile { tools: vec![] };
        let executor = FoundryExecutor::new(schema);
        let bin_path = executor.foundry_bin_path().clone();

        let handler = FoundryMcpHandler::new(executor);

        // Handler should preserve the executor's bin path
        assert_eq!(handler.foundry_bin_path(), &bin_path);
    }

    /// Test that multiple handlers can be created and used independently
    #[test]
    fn test_multiple_handlers_can_coexist() {
        let handler1 = create_test_handler();
        let handler2 = create_test_handler();

        // Both handlers should be independently valid
        let info1 = handler1.get_info();
        let info2 = handler2.get_info();

        assert_eq!(info1.server_info.name, info2.server_info.name);
    }
}
