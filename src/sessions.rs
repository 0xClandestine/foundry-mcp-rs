//! Session management for long-running processes like Anvil and Chisel

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Global session manager instance
static SESSION_MANAGER: Lazy<Arc<Mutex<SessionManager>>> =
    Lazy::new(|| Arc::new(Mutex::new(SessionManager::new())));

/// Type of background session
#[derive(Debug, Clone, PartialEq)]
pub enum SessionType {
    Anvil,
    Chisel,
}

/// Information about a running session
pub struct SessionInfo {
    pub session_type: SessionType,
    pub process: Child,
    pub port: Option<u16>,
    pub created_at: std::time::SystemTime,
}

/// Manages long-running background processes
pub struct SessionManager {
    sessions: HashMap<String, SessionInfo>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Get the global session manager instance
    pub fn global() -> Arc<Mutex<SessionManager>> {
        SESSION_MANAGER.clone()
    }

    /// Start an Anvil session
    pub fn start_anvil(
        &mut self,
        foundry_bin_path: &Option<String>,
        port: u16,
        fork_url: Option<String>,
        fork_block_number: Option<u64>,
        accounts: Option<u32>,
        block_time: Option<u64>,
    ) -> Result<String> {
        // Check if anvil is already running
        if self.is_anvil_running() {
            anyhow::bail!("Anvil is already running. Stop it first with anvil_session_stop.");
        }

        let anvil_cmd = if let Some(bin_path) = foundry_bin_path {
            format!("{}/anvil", bin_path)
        } else {
            "anvil".to_string()
        };

        let mut cmd = Command::new(&anvil_cmd);
        cmd.arg("--port").arg(port.to_string());

        if let Some(url) = fork_url {
            cmd.arg("--fork-url").arg(url);
        }

        if let Some(block_num) = fork_block_number {
            cmd.arg("--fork-block-number").arg(block_num.to_string());
        }

        if let Some(acc) = accounts {
            cmd.arg("--accounts").arg(acc.to_string());
        }

        if let Some(time) = block_time {
            cmd.arg("--block-time").arg(time.to_string());
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .context("Failed to start Anvil. Is Foundry installed?")?;

        let pid = child.id();

        // Wait a moment for anvil to start
        std::thread::sleep(std::time::Duration::from_millis(1000));

        self.sessions.insert(
            "anvil".to_string(),
            SessionInfo {
                session_type: SessionType::Anvil,
                process: child,
                port: Some(port),
                created_at: std::time::SystemTime::now(),
            },
        );

        Ok(format!(
            "Anvil started successfully on port {}. RPC URL: http://localhost:{}\nProcess ID: {}",
            port, port, pid
        ))
    }

    /// Stop the Anvil session
    pub fn stop_anvil(&mut self) -> Result<String> {
        if let Some(mut session) = self.sessions.remove("anvil") {
            session
                .process
                .kill()
                .context("Failed to kill Anvil process")?;
            session
                .process
                .wait()
                .context("Failed to wait for Anvil process")?;
            Ok("Anvil has been stopped successfully.".to_string())
        } else {
            anyhow::bail!("No Anvil session is currently running.")
        }
    }

    /// Get Anvil session status
    pub fn anvil_status(&self) -> Result<String> {
        if let Some(session) = self.sessions.get("anvil") {
            let port = session.port.unwrap_or(8545);
            let uptime = session
                .created_at
                .elapsed()
                .map(|d| format!("{}s", d.as_secs()))
                .unwrap_or_else(|_| "unknown".to_string());

            Ok(format!(
                "Anvil is running on port {}. RPC URL: http://localhost:{}\nUptime: {}",
                port, port, uptime
            ))
        } else {
            Ok("Anvil is not currently running.".to_string())
        }
    }

    /// Check if Anvil is running
    pub fn is_anvil_running(&self) -> bool {
        self.sessions.contains_key("anvil")
    }

    /// Start a Chisel session (validates chisel is available)
    pub fn start_chisel(&mut self, foundry_bin_path: &Option<String>) -> Result<String> {
        // Check if chisel is already running
        if self.is_chisel_running() {
            anyhow::bail!("Chisel is already running. Stop it first with chisel_session_stop.");
        }

        let chisel_cmd = if let Some(bin_path) = foundry_bin_path {
            format!("{}/chisel", bin_path)
        } else {
            "chisel".to_string()
        };

        // Validate chisel is available by trying to run --help
        let test_result = Command::new(&chisel_cmd)
            .arg("--help")
            .output()
            .context("Failed to start Chisel. Is Foundry installed?")?;

        if !test_result.status.success() {
            anyhow::bail!("Chisel command failed. Is Foundry installed?");
        }

        // Mark chisel session as active (we spawn fresh processes per eval)
        self.sessions.insert(
            "chisel".to_string(),
            SessionInfo {
                session_type: SessionType::Chisel,
                process: Command::new("true").spawn()?, // Dummy process for tracking
                port: None,
                created_at: std::time::SystemTime::now(),
            },
        );

        Ok(
            "Chisel REPL session started successfully.\n\nSession is ready for code execution. Use chisel_session_eval to execute Solidity code.\n\nNote: Each eval spawns a fresh chisel process. State persists via Chisel's cache system.\n\nTips:\n- Variables and functions are cached between eval calls\n- Use semicolons to suppress output\n- Use !help for chisel commands"
                .to_string(),
        )
    }

    /// Evaluate Solidity code in the running Chisel session
    ///
    /// Note: This spawns a fresh chisel process for each eval to avoid blocking I/O issues.
    /// Chisel's cache system preserves state across invocations.
    pub fn chisel_eval(
        &mut self,
        code: String,
        foundry_bin_path: &Option<String>,
    ) -> Result<String> {
        // Verify session is active
        if !self.is_chisel_running() {
            anyhow::bail!("No Chisel session is running. Start one with chisel_session_start.");
        }

        let chisel_cmd = if let Some(bin_path) = foundry_bin_path {
            format!("{}/chisel", bin_path)
        } else {
            "chisel".to_string()
        };

        // Use chisel with piped input - it processes line by line and exits on EOF
        let mut cmd = Command::new(&chisel_cmd);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("Failed to start Chisel")?;

        // Write the code and close stdin (signals EOF to chisel)
        if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{}", code)?;
            writeln!(stdin, "!quit")?;
            stdin.flush()?;
        }

        // Wait for chisel to finish (with timeout)
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(10);

        loop {
            match child.try_wait()? {
                Some(_status) => break,
                None => {
                    if start.elapsed() >= timeout {
                        child.kill()?;
                        return Err(anyhow::anyhow!(
                            "Chisel execution timed out after 10 seconds"
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }

        // Collect output
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let combined = format!("{}{}", stdout, stderr);

        // Filter out welcome message and prompts, keep only the actual output
        let lines: Vec<&str> = combined.lines().collect();
        let mut filtered_lines = Vec::new();
        let mut skip_welcome = true;

        for line in lines {
            let trimmed = line.trim();

            // Skip welcome message and prompts
            if skip_welcome {
                if trimmed.is_empty()
                    || trimmed == "➜"
                    || trimmed.contains("Welcome to Chisel")
                    || trimmed.contains("Type `!help`")
                {
                    continue;
                }
                // Once we see actual content, stop skipping welcome
                skip_welcome = false;
            }

            // Skip standalone prompts
            if trimmed == "➜" {
                continue;
            }

            // Remove leading prompt from lines with content after it
            let cleaned = if line.starts_with("➜ ") {
                line.chars().skip(2).collect::<String>() // Skip "➜ " (multi-byte safe)
            } else {
                line.to_string()
            };

            filtered_lines.push(cleaned);
        }

        let result = filtered_lines.join("\n").trim().to_string();

        if result.is_empty() {
            Ok("Code executed (no output)".to_string())
        } else {
            Ok(result)
        }
    }

    /// Stop the Chisel session
    pub fn stop_chisel(&mut self) -> Result<String> {
        if let Some(mut session) = self.sessions.remove("chisel") {
            // Try to exit gracefully first
            if let Some(stdin) = session.process.stdin.as_mut() {
                let _ = writeln!(stdin, "!quit");
                let _ = stdin.flush();
            }

            // Wait a moment, then force kill if needed
            std::thread::sleep(std::time::Duration::from_millis(500));

            let _ = session.process.kill();
            let _ = session.process.wait();

            Ok("Chisel session has been stopped successfully.".to_string())
        } else {
            anyhow::bail!("No Chisel session is currently running.")
        }
    }

    /// Get Chisel session status
    pub fn chisel_status(&self) -> Result<String> {
        if let Some(session) = self.sessions.get("chisel") {
            let uptime = session
                .created_at
                .elapsed()
                .map(|d| format!("{}s", d.as_secs()))
                .unwrap_or_else(|_| "unknown".to_string());

            Ok(format!(
                "Chisel REPL session is active.\nUptime: {}\nUse chisel_session_eval to execute code.",
                uptime
            ))
        } else {
            Ok("Chisel session is not currently running.".to_string())
        }
    }

    /// Check if Chisel is running
    pub fn is_chisel_running(&self) -> bool {
        self.sessions.contains_key("chisel")
    }

    /// Stop all sessions (cleanup)
    pub fn stop_all(&mut self) -> Vec<String> {
        let mut results = Vec::new();

        if self.is_anvil_running() {
            match self.stop_anvil() {
                Ok(msg) => results.push(msg),
                Err(e) => results.push(format!("Error stopping Anvil: {}", e)),
            }
        }

        if self.is_chisel_running() {
            match self.stop_chisel() {
                Ok(msg) => results.push(msg),
                Err(e) => results.push(format!("Error stopping Chisel: {}", e)),
            }
        }

        results
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        // Clean up any running sessions
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that session manager can be created successfully
    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::new();
        assert!(!manager.is_anvil_running());
        assert!(!manager.is_chisel_running());
    }

    /// Test that global session manager can be accessed
    #[test]
    fn test_global_session_manager() {
        let manager1 = SessionManager::global();
        let manager2 = SessionManager::global();

        // Both should point to the same instance
        assert!(Arc::ptr_eq(&manager1, &manager2));
    }

    /// Test anvil status when not running
    #[test]
    fn test_anvil_status_when_not_running() {
        let manager = SessionManager::new();
        let status = manager.anvil_status().unwrap();
        assert!(status.contains("not currently running"));
    }

    /// Test chisel status when not running
    #[test]
    fn test_chisel_status_when_not_running() {
        let manager = SessionManager::new();
        let status = manager.chisel_status().unwrap();
        assert!(status.contains("not currently running"));
    }

    /// Test stopping anvil when not running returns error
    #[test]
    fn test_stop_anvil_when_not_running() {
        let mut manager = SessionManager::new();
        let result = manager.stop_anvil();
        assert!(
            result.is_err(),
            "Expected error when stopping non-running anvil"
        );
    }

    /// Test stopping chisel when not running returns error
    #[test]
    fn test_stop_chisel_when_not_running() {
        let mut manager = SessionManager::new();
        let result = manager.stop_chisel();
        assert!(
            result.is_err(),
            "Expected error when stopping non-running chisel"
        );
    }

    /// Test that is_anvil_running returns false initially
    #[test]
    fn test_is_anvil_running_initially_false() {
        let manager = SessionManager::new();
        assert!(!manager.is_anvil_running());
    }

    /// Test that is_chisel_running returns false initially
    #[test]
    fn test_is_chisel_running_initially_false() {
        let manager = SessionManager::new();
        assert!(!manager.is_chisel_running());
    }

    /// Test stop_all on empty manager
    #[test]
    fn test_stop_all_when_empty() {
        let mut manager = SessionManager::new();
        let results = manager.stop_all();
        assert!(results.is_empty());
    }

    /// Test SessionType enum equality
    #[test]
    fn test_session_type_equality() {
        assert_eq!(SessionType::Anvil, SessionType::Anvil);
        assert_eq!(SessionType::Chisel, SessionType::Chisel);
        assert_ne!(SessionType::Anvil, SessionType::Chisel);
    }

    /// Test SessionType clone
    #[test]
    fn test_session_type_clone() {
        let anvil = SessionType::Anvil;
        let anvil_clone = anvil.clone();
        assert_eq!(anvil, anvil_clone);
    }

    /// Test that start_anvil with invalid binary path fails gracefully
    #[test]
    fn test_start_anvil_with_invalid_path() {
        let mut manager = SessionManager::new();
        let invalid_path = Some("/nonexistent/path/to/foundry".to_string());

        let result = manager.start_anvil(&invalid_path, 8545, None, None, None, None);

        assert!(result.is_err());
    }

    /// Test that start_chisel with invalid binary path fails gracefully
    #[test]
    fn test_start_chisel_with_invalid_path() {
        let mut manager = SessionManager::new();
        let invalid_path = Some("/nonexistent/path/to/foundry".to_string());

        let result = manager.start_chisel(&invalid_path);

        assert!(result.is_err());
    }

    /// Test that multiple sessions can be tracked
    #[test]
    fn test_sessions_hashmap() {
        let mut manager = SessionManager::new();
        assert_eq!(manager.sessions.len(), 0);

        // After failed starts, should still be 0
        let _ = manager.start_anvil(&Some("/invalid".to_string()), 8545, None, None, None, None);
        assert_eq!(manager.sessions.len(), 0);
    }

    /// Test chisel eval without running session
    #[test]
    fn test_chisel_eval_without_session() {
        let mut manager = SessionManager::new();
        let result = manager.chisel_eval("uint256 x = 42;".to_string(), &None);

        assert!(
            result.is_err(),
            "Expected error when evaluating without chisel session"
        );
    }

    /// Integration test: Test anvil lifecycle (requires Foundry installed)
    #[test]
    #[ignore] // Run with --ignored flag only if Foundry is installed
    fn test_anvil_lifecycle_integration() {
        let mut manager = SessionManager::new();

        // Start anvil
        let start_result = manager.start_anvil(&None, 18545, None, None, None, None);
        if start_result.is_err() {
            // Skip test if Foundry not installed
            return;
        }

        assert!(start_result.is_ok());
        assert!(manager.is_anvil_running());

        // Check status
        let status = manager.anvil_status().unwrap();
        assert!(status.contains("running"));
        assert!(status.contains("18545"));

        // Stop anvil
        let stop_result = manager.stop_anvil();
        assert!(stop_result.is_ok());
        assert!(!manager.is_anvil_running());
    }

    /// Integration test: Test chisel lifecycle (requires Foundry installed)
    #[test]
    #[ignore] // Run with --ignored flag only if Foundry is installed
    fn test_chisel_lifecycle_integration() {
        let mut manager = SessionManager::new();

        // Start chisel
        let start_result = manager.start_chisel(&None);
        if start_result.is_err() {
            // Skip test if Foundry not installed
            return;
        }

        assert!(start_result.is_ok());
        assert!(manager.is_chisel_running());

        // Check status
        let status = manager.chisel_status().unwrap();
        assert!(status.contains("active"));

        // Eval code
        let eval_result = manager.chisel_eval("uint256 x = 42;".to_string(), &None);
        // May succeed or fail depending on chisel behavior, just check it doesn't panic
        let _ = eval_result;

        // Stop chisel
        let stop_result = manager.stop_chisel();
        assert!(stop_result.is_ok());
        assert!(!manager.is_chisel_running());
    }

    /// Test that starting anvil twice fails
    #[test]
    #[ignore] // Integration test
    fn test_start_anvil_twice_fails() {
        let mut manager = SessionManager::new();

        // Start once
        let first_start = manager.start_anvil(&None, 18546, None, None, None, None);
        if first_start.is_err() {
            return; // Skip if Foundry not installed
        }

        // Try to start again
        let second_start = manager.start_anvil(&None, 18546, None, None, None, None);
        assert!(second_start.is_err());
        assert!(second_start
            .unwrap_err()
            .to_string()
            .contains("already running"));

        // Cleanup
        let _ = manager.stop_anvil();
    }

    /// Test that starting chisel twice fails
    #[test]
    #[ignore] // Integration test
    fn test_start_chisel_twice_fails() {
        let mut manager = SessionManager::new();

        // Start once
        let first_start = manager.start_chisel(&None);
        if first_start.is_err() {
            return; // Skip if Foundry not installed
        }

        // Try to start again
        let second_start = manager.start_chisel(&None);
        assert!(second_start.is_err());
        assert!(second_start
            .unwrap_err()
            .to_string()
            .contains("already running"));

        // Cleanup
        let _ = manager.stop_chisel();
    }
}
