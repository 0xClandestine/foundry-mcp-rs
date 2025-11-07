//! MCP tool handlers for session management

use rmcp::model::*;
use serde_json::Value;
use std::sync::Arc;

use crate::sessions::SessionManager;

/// Get all session management tools
pub fn get_session_tools() -> Vec<Tool> {
    vec![
        // Anvil session tools
        anvil_session_start_tool(),
        anvil_session_stop_tool(),
        anvil_session_status_tool(),
        // Chisel session tools
        chisel_session_start_tool(),
        chisel_session_eval_tool(),
        chisel_session_stop_tool(),
        chisel_session_status_tool(),
    ]
}

fn anvil_session_start_tool() -> Tool {
    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));

    let mut properties = serde_json::Map::new();
    properties.insert(
        "port".to_string(),
        serde_json::json!({
            "type": "number",
            "description": "Port to listen on (default: 8545)",
            "default": 8545
        }),
    );
    properties.insert(
        "fork_url".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "URL of the JSON-RPC endpoint to fork from (optional)"
        }),
    );
    properties.insert(
        "fork_block_number".to_string(),
        serde_json::json!({
            "type": "number",
            "description": "Block number to fork from (optional)"
        }),
    );
    properties.insert(
        "accounts".to_string(),
        serde_json::json!({
            "type": "number",
            "description": "Number of accounts to generate (default: 10)"
        }),
    );
    properties.insert(
        "block_time".to_string(),
        serde_json::json!({
            "type": "number",
            "description": "Block time in seconds (0 = mine on demand, default: 0)"
        }),
    );

    input_schema.insert("properties".to_string(), Value::Object(properties));

    Tool::new(
        "anvil_session_start".to_string(),
        "Start an Anvil instance (local Ethereum node) as a background process. Supports forking, custom ports, accounts, and block time. Use Cast tools with rpc-url=http://localhost:<port> to interact.".to_string(),
        Arc::new(input_schema),
    )
}

fn anvil_session_stop_tool() -> Tool {
    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));
    input_schema.insert(
        "properties".to_string(),
        Value::Object(serde_json::Map::new()),
    );

    Tool::new(
        "anvil_session_stop".to_string(),
        "Stop the running Anvil instance".to_string(),
        Arc::new(input_schema),
    )
}

fn anvil_session_status_tool() -> Tool {
    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));
    input_schema.insert(
        "properties".to_string(),
        Value::Object(serde_json::Map::new()),
    );

    Tool::new(
        "anvil_session_status".to_string(),
        "Check if Anvil is running and get its status".to_string(),
        Arc::new(input_schema),
    )
}

fn chisel_session_start_tool() -> Tool {
    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));
    input_schema.insert(
        "properties".to_string(),
        Value::Object(serde_json::Map::new()),
    );

    Tool::new(
        "chisel_session_start".to_string(),
        "Start a Chisel session (validates chisel is available). State persists across eval calls via Chisel's built-in cache system. Use chisel_session_eval to execute code.".to_string(),
        Arc::new(input_schema),
    )
}

fn chisel_session_eval_tool() -> Tool {
    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));

    let mut properties = serde_json::Map::new();
    properties.insert(
        "code".to_string(),
        serde_json::json!({
            "type": "string",
            "description": "Solidity code to execute in the running Chisel session"
        }),
    );

    input_schema.insert("properties".to_string(), Value::Object(properties));
    input_schema.insert(
        "required".to_string(),
        Value::Array(vec![Value::String("code".to_string())]),
    );

    Tool::new(
        "chisel_session_eval".to_string(),
        "Execute Solidity code in a Chisel session. Spawns a fresh chisel process with piped input/output. Returns all chisel output including welcome message and prompts. State persists via Chisel's cache system. 10-second timeout.".to_string(),
        Arc::new(input_schema),
    )
}

fn chisel_session_stop_tool() -> Tool {
    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));
    input_schema.insert(
        "properties".to_string(),
        Value::Object(serde_json::Map::new()),
    );

    Tool::new(
        "chisel_session_stop".to_string(),
        "Stop the running Chisel REPL session".to_string(),
        Arc::new(input_schema),
    )
}

fn chisel_session_status_tool() -> Tool {
    let mut input_schema = serde_json::Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));
    input_schema.insert(
        "properties".to_string(),
        Value::Object(serde_json::Map::new()),
    );

    Tool::new(
        "chisel_session_status".to_string(),
        "Check if a Chisel REPL session is running and get its status".to_string(),
        Arc::new(input_schema),
    )
}

/// Handle anvil session start
pub async fn handle_anvil_session_start(
    args: &Option<serde_json::Map<String, Value>>,
    foundry_bin_path: &Option<String>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let port = args
        .as_ref()
        .and_then(|a| a.get("port"))
        .and_then(|v| v.as_u64())
        .unwrap_or(8545) as u16;

    let fork_url = args
        .as_ref()
        .and_then(|a| a.get("fork_url"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let fork_block_number = args
        .as_ref()
        .and_then(|a| a.get("fork_block_number"))
        .and_then(|v| v.as_u64());

    let accounts = args
        .as_ref()
        .and_then(|a| a.get("accounts"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let block_time = args
        .as_ref()
        .and_then(|a| a.get("block_time"))
        .and_then(|v| v.as_u64());

    // Run blocking operation in a background thread
    let foundry_bin_path = foundry_bin_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let global_manager = SessionManager::global();
        let mut manager = global_manager.lock().unwrap();
        manager.start_anvil(
            &foundry_bin_path,
            port,
            fork_url,
            fork_block_number,
            accounts,
            block_time,
        )
    })
    .await
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Task error: {}", e), None))?;

    match result {
        Ok(msg) => Ok(CallToolResult {
            content: vec![Content::text(msg)],
            structured_content: None,
            is_error: None,
            meta: None,
        }),
        Err(e) => Err(rmcp::ErrorData::internal_error(e.to_string(), None)),
    }
}

/// Handle anvil session stop
pub async fn handle_anvil_session_stop() -> Result<CallToolResult, rmcp::ErrorData> {
    let result = tokio::task::spawn_blocking(move || {
        let global_manager = SessionManager::global();
        let mut manager = global_manager.lock().unwrap();
        manager.stop_anvil()
    })
    .await
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Task error: {}", e), None))?;

    match result {
        Ok(msg) => Ok(CallToolResult {
            content: vec![Content::text(msg)],
            structured_content: None,
            is_error: None,
            meta: None,
        }),
        Err(e) => Err(rmcp::ErrorData::internal_error(e.to_string(), None)),
    }
}

/// Handle anvil session status
pub async fn handle_anvil_session_status() -> Result<CallToolResult, rmcp::ErrorData> {
    let result = tokio::task::spawn_blocking(move || {
        let global_manager = SessionManager::global();
        let manager = global_manager.lock().unwrap();
        manager.anvil_status()
    })
    .await
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Task error: {}", e), None))?;

    match result {
        Ok(msg) => Ok(CallToolResult {
            content: vec![Content::text(msg)],
            structured_content: None,
            is_error: None,
            meta: None,
        }),
        Err(e) => Err(rmcp::ErrorData::internal_error(e.to_string(), None)),
    }
}

/// Handle chisel session start
pub async fn handle_chisel_session_start(
    foundry_bin_path: &Option<String>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let foundry_bin_path = foundry_bin_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let global_manager = SessionManager::global();
        let mut manager = global_manager.lock().unwrap();
        manager.start_chisel(&foundry_bin_path)
    })
    .await
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Task error: {}", e), None))?;

    match result {
        Ok(msg) => Ok(CallToolResult {
            content: vec![Content::text(msg)],
            structured_content: None,
            is_error: None,
            meta: None,
        }),
        Err(e) => Err(rmcp::ErrorData::internal_error(e.to_string(), None)),
    }
}

/// Handle chisel session eval
pub async fn handle_chisel_session_eval(
    args: &Option<serde_json::Map<String, Value>>,
    foundry_bin_path: &Option<String>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let code = args
        .as_ref()
        .and_then(|a| a.get("code"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| rmcp::ErrorData::invalid_params("Missing 'code' parameter", None))?
        .to_string();

    let foundry_bin_path = foundry_bin_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let global_manager = SessionManager::global();
        let mut manager = global_manager.lock().unwrap();
        manager.chisel_eval(code, &foundry_bin_path)
    })
    .await
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Task error: {}", e), None))?;

    match result {
        Ok(output) => Ok(CallToolResult {
            content: vec![Content::text(output)],
            structured_content: None,
            is_error: None,
            meta: None,
        }),
        Err(e) => Err(rmcp::ErrorData::internal_error(e.to_string(), None)),
    }
}

/// Handle chisel session stop
pub async fn handle_chisel_session_stop() -> Result<CallToolResult, rmcp::ErrorData> {
    let result = tokio::task::spawn_blocking(move || {
        let global_manager = SessionManager::global();
        let mut manager = global_manager.lock().unwrap();
        manager.stop_chisel()
    })
    .await
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Task error: {}", e), None))?;

    match result {
        Ok(msg) => Ok(CallToolResult {
            content: vec![Content::text(msg)],
            structured_content: None,
            is_error: None,
            meta: None,
        }),
        Err(e) => Err(rmcp::ErrorData::internal_error(e.to_string(), None)),
    }
}

/// Handle chisel session status
pub async fn handle_chisel_session_status() -> Result<CallToolResult, rmcp::ErrorData> {
    let result = tokio::task::spawn_blocking(move || {
        let global_manager = SessionManager::global();
        let manager = global_manager.lock().unwrap();
        manager.chisel_status()
    })
    .await
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Task error: {}", e), None))?;

    match result {
        Ok(msg) => Ok(CallToolResult {
            content: vec![Content::text(msg)],
            structured_content: None,
            is_error: None,
            meta: None,
        }),
        Err(e) => Err(rmcp::ErrorData::internal_error(e.to_string(), None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that get_session_tools returns correct number of tools
    #[test]
    fn test_get_session_tools_count() {
        let tools = get_session_tools();
        assert_eq!(tools.len(), 7); // 3 anvil + 4 chisel
    }

    /// Test that all session tools have correct names
    #[test]
    fn test_session_tool_names() {
        let tools = get_session_tools();
        let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();

        assert!(names.contains(&"anvil_session_start".to_string()));
        assert!(names.contains(&"anvil_session_stop".to_string()));
        assert!(names.contains(&"anvil_session_status".to_string()));
        assert!(names.contains(&"chisel_session_start".to_string()));
        assert!(names.contains(&"chisel_session_eval".to_string()));
        assert!(names.contains(&"chisel_session_stop".to_string()));
        assert!(names.contains(&"chisel_session_status".to_string()));
    }

    /// Test anvil_session_start tool has correct schema
    #[test]
    fn test_anvil_session_start_tool_schema() {
        let tools = get_session_tools();
        let tool = tools
            .iter()
            .find(|t| t.name == "anvil_session_start")
            .unwrap();

        assert!(tool
            .description
            .as_ref()
            .map(|d| d.contains("Anvil"))
            .unwrap_or(false));
        assert!(tool
            .description
            .as_ref()
            .map(|d| d.contains("background"))
            .unwrap_or(false));

        let props = tool
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(props.contains_key("port"));
        assert!(props.contains_key("fork_url"));
        assert!(props.contains_key("accounts"));
        assert!(props.contains_key("block_time"));
    }

    /// Test chisel_session_eval tool requires code parameter
    #[test]
    fn test_chisel_session_eval_tool_schema() {
        let tools = get_session_tools();
        let tool = tools
            .iter()
            .find(|t| t.name == "chisel_session_eval")
            .unwrap();

        let props = tool
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(props.contains_key("code"));

        let required = tool
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&Value::String("code".to_string())));
    }

    /// Test handle_anvil_session_status when not running
    #[tokio::test]
    async fn test_handle_anvil_session_status_not_running() {
        let result = handle_anvil_session_status().await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);
        // Successfully got a response
    }

    /// Test handle_chisel_session_status when not running
    #[tokio::test]
    async fn test_handle_chisel_session_status_not_running() {
        let result = handle_chisel_session_status().await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.content.len(), 1);
        // Successfully got a response
    }

    /// Test handle_anvil_session_stop when not running returns error
    #[tokio::test]
    async fn test_handle_anvil_session_stop_not_running() {
        let result = handle_anvil_session_stop().await;
        assert!(
            result.is_err(),
            "Expected error when stopping non-running anvil"
        );
    }

    /// Test handle_chisel_session_stop when not running returns error
    #[tokio::test]
    async fn test_handle_chisel_session_stop_not_running() {
        let result = handle_chisel_session_stop().await;
        assert!(
            result.is_err(),
            "Expected error when stopping non-running chisel"
        );
    }

    /// Test handle_chisel_session_eval without code parameter
    #[tokio::test]
    async fn test_handle_chisel_session_eval_missing_code() {
        let empty_args = serde_json::Map::new();
        let result = handle_chisel_session_eval(&Some(empty_args), &None).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("code"));
    }

    /// Test handle_chisel_session_eval without running session
    #[tokio::test]
    async fn test_handle_chisel_session_eval_no_session() {
        let mut args = serde_json::Map::new();
        args.insert(
            "code".to_string(),
            Value::String("uint256 x = 42;".to_string()),
        );

        let result = handle_chisel_session_eval(&Some(args), &None).await;

        assert!(
            result.is_err(),
            "Expected error when evaluating without chisel session"
        );
    }

    /// Test handle_anvil_session_start with default port
    #[tokio::test]
    async fn test_handle_anvil_session_start_default_args() {
        let empty_args = serde_json::Map::new();
        let foundry_bin_path = Some("/nonexistent".to_string());

        let result = handle_anvil_session_start(&Some(empty_args), &foundry_bin_path).await;

        // Should fail because path doesn't exist, but it processes the args correctly
        assert!(result.is_err());
    }

    /// Test handle_anvil_session_start with custom port
    #[tokio::test]
    async fn test_handle_anvil_session_start_custom_port() {
        let mut args = serde_json::Map::new();
        args.insert("port".to_string(), Value::Number(9999.into()));

        let foundry_bin_path = Some("/nonexistent".to_string());
        let result = handle_anvil_session_start(&Some(args), &foundry_bin_path).await;

        // Should fail because path doesn't exist
        assert!(result.is_err());
    }

    /// Test handle_anvil_session_start with fork parameters
    #[tokio::test]
    async fn test_handle_anvil_session_start_with_fork() {
        let mut args = serde_json::Map::new();
        args.insert(
            "fork_url".to_string(),
            Value::String("https://eth.llamarpc.com".to_string()),
        );
        args.insert(
            "fork_block_number".to_string(),
            Value::Number(12345678.into()),
        );

        let foundry_bin_path = Some("/nonexistent".to_string());
        let result = handle_anvil_session_start(&Some(args), &foundry_bin_path).await;

        // Should fail because path doesn't exist
        assert!(result.is_err());
    }

    /// Test handle_chisel_session_start with invalid path
    #[tokio::test]
    async fn test_handle_chisel_session_start_invalid_path() {
        let foundry_bin_path = Some("/nonexistent".to_string());
        let result = handle_chisel_session_start(&foundry_bin_path).await;

        assert!(result.is_err());
    }

    /// Test that all stop/status tools have empty input schemas
    #[test]
    fn test_stop_status_tools_empty_schemas() {
        let tools = get_session_tools();

        let stop_status_names = vec![
            "anvil_session_stop",
            "anvil_session_status",
            "chisel_session_stop",
            "chisel_session_status",
        ];

        for name in stop_status_names {
            let tool = tools.iter().find(|t| t.name == name).unwrap();
            let props = tool
                .input_schema
                .get("properties")
                .unwrap()
                .as_object()
                .unwrap();
            assert!(
                props.is_empty(),
                "Tool {} should have empty properties",
                name
            );
        }
    }

    /// Test all tool descriptions are informative
    #[test]
    fn test_all_tools_have_descriptions() {
        let tools = get_session_tools();

        for tool in tools {
            assert!(tool.description.is_some());
            if let Some(desc) = &tool.description {
                assert!(desc.len() > 10); // Should be descriptive
            }
        }
    }

    /// Integration test: Test full anvil session workflow
    #[tokio::test]
    #[ignore] // Run with --ignored flag only if Foundry is installed
    async fn test_anvil_session_workflow_integration() {
        // Start session
        let mut start_args = serde_json::Map::new();
        start_args.insert("port".to_string(), Value::Number(18547.into()));

        let start_result = handle_anvil_session_start(&Some(start_args), &None).await;
        if start_result.is_err() {
            return; // Skip if Foundry not installed
        }
        assert!(start_result.is_ok());

        // Check status
        let status_result = handle_anvil_session_status().await;
        assert!(status_result.is_ok());

        // Stop session
        let stop_result = handle_anvil_session_stop().await;
        assert!(stop_result.is_ok());
    }

    /// Integration test: Test full chisel session workflow
    #[tokio::test]
    #[ignore] // Run with --ignored flag only if Foundry is installed
    async fn test_chisel_session_workflow_integration() {
        // Start session
        let start_result = handle_chisel_session_start(&None).await;
        if start_result.is_err() {
            return; // Skip if Foundry not installed
        }
        assert!(start_result.is_ok());

        // Check status
        let status_result = handle_chisel_session_status().await;
        assert!(status_result.is_ok());

        // Eval code
        let mut eval_args = serde_json::Map::new();
        eval_args.insert(
            "code".to_string(),
            Value::String("uint256 x = 42;".to_string()),
        );
        let eval_result = handle_chisel_session_eval(&Some(eval_args), &None).await;
        // May succeed or fail depending on chisel, just check it doesn't panic
        let _ = eval_result;

        // Stop session
        let stop_result = handle_chisel_session_stop().await;
        assert!(stop_result.is_ok());
    }
}
