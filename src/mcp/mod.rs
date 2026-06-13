pub mod bash;
pub mod client;
pub mod process;
pub mod types;

use anyhow::{Context, Result};
use tracing::warn;

pub use bash::BashTool;
pub use client::McpClient;
pub use types::{McpTool, ToolCall, ToolResult, ToolApproval};

use crate::config::McpConfig;

pub struct McpManager {
    tools: Vec<McpTool>,
    clients: Vec<McpClient>,
}

impl McpManager {
    pub async fn init(config: &McpConfig) -> Result<Self> {
        let mut tools = Vec::new();
        let mut clients = Vec::new();

        for server in &config.servers {
            if server.builtin == Some("bash".to_string()) {
                tools.push(McpTool {
                    name: "bash".to_string(),
                    description: "Run a bash command and return its stdout/stderr".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "Shell command to execute"
                            }
                        },
                        "required": ["command"]
                    }),
                });
            } else if server.command.is_some() {
                match McpClient::connect(server).await {
                    Ok(mut client) => {
                        match client.list_tools().await {
                            Ok(mut server_tools) => {
                                tools.append(&mut server_tools);
                                clients.push(client);
                            }
                            Err(e) => {
                                warn!(
                                    "failed to list tools from MCP server '{}': {}",
                                    server.name, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "failed to connect to MCP server '{}': {}",
                            server.name, e
                        );
                    }
                }
            }
        }

        Ok(McpManager { tools, clients })
    }

    pub fn list_tools(&self) -> Vec<McpTool> {
        self.tools.clone()
    }

    pub async fn call_tool(&mut self, tool_name: &str, input: serde_json::Value) -> Result<ToolResult> {
        if tool_name == "bash" {
            let command = input
                .get("command")
                .and_then(|c| c.as_str())
                .context("bash tool requires 'command' argument")?;

            let mut result = BashTool::execute(command).await;
            result.tool_use_id = String::new();
            return Ok(result);
        }

        for client in &mut self.clients {
            match client.call_tool(tool_name, input.clone()).await {
                Ok(output) => {
                    return Ok(ToolResult {
                        tool_use_id: String::new(),
                        content: output,
                        is_error: false,
                    });
                }
                Err(_) => continue,
            }
        }

        anyhow::bail!("tool '{}' not found", tool_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_builtin_bash_tool() {
        let config = McpConfig {
            auto_approve: false,
            servers: vec![crate::config::McpServerConfig {
                name: "bash".to_string(),
                builtin: Some("bash".to_string()),
                command: None,
                args: vec![],
                env: std::collections::HashMap::new(),
            }],
        };

        let manager = McpManager::init(&config).await.expect("init");
        let tools = manager.list_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "bash");
    }

    #[tokio::test]
    async fn calls_bash_tool() {
        let config = McpConfig {
            auto_approve: false,
            servers: vec![crate::config::McpServerConfig {
                name: "bash".to_string(),
                builtin: Some("bash".to_string()),
                command: None,
                args: vec![],
                env: std::collections::HashMap::new(),
            }],
        };

        let mut manager = McpManager::init(&config).await.expect("init");
        let result = manager
            .call_tool("bash", serde_json::json!({"command": "echo hello"}))
            .await
            .expect("call");

        assert!(!result.is_error);
        assert_eq!(result.content, "hello");
    }
}
