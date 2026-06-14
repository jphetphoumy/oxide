pub mod bash;
pub mod client;
pub mod jsonrpc;
pub mod process;
pub mod server;
pub mod subagent;
pub mod transport;
pub mod types;

use anyhow::{Context, Result};
use tracing::warn;

pub use bash::BashTool;
pub use client::McpClient;
pub use server::McpJsonRpcServer;
pub use transport::McpTransport;
pub use types::{McpTool, ToolCall, ToolResult};

use crate::config::McpConfig;

pub struct McpManager {
    tools: Vec<McpTool>,
    clients: Vec<McpClient>,
    skills: Vec<crate::skills::Skill>,
    dust_client: Option<crate::dust::client::DustClient>,
    current_depth: u32,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::dust::client::DustEvent>>,
    next_call_id: std::sync::atomic::AtomicU64,
}

impl McpManager {
    #[allow(clippy::too_many_lines)]
    pub async fn init(
        config: &McpConfig,
        skills: Vec<crate::skills::Skill>,
        dust_client: Option<crate::dust::client::DustClient>,
    ) -> Result<Self> {
        let mut tools = Vec::new();
        let mut clients = Vec::new();

        tracing::debug!(
            server_count = config.servers.len(),
            skill_count = skills.len(),
            "initializing MCP manager"
        );

        // Register built-in oxide_skill tool (always available)
        tools.push(McpTool {
            name: "oxide_skill".to_string(),
            description: format!("Load the full instructions for a local skill from {}/", crate::skills::SKILLS_DIR),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "skill_id": {
                        "type": "string",
                        "description": "Skill id, e.g. \"code-review\" for .agents/skills/code-review.md"
                    }
                },
                "required": ["skill_id"]
            }),
        });

        // Register built-in oxide_agent tool (only available when dust_client is present)
        if dust_client.is_some() {
            tools.push(McpTool {
                name: "oxide_agent".to_string(),
                description: "Spawn a new one-shot conversation with a Dust agent and return the agent's full response. Use this to delegate a subtask to a specialised agent. The subagent starts fresh (no shared context).".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "The full prompt to send to the subagent."
                        },
                        "agent_id": {
                            "type": "string",
                            "description": "Optional Dust agent sId or slug. Defaults to the current session agent."
                        },
                        "description": {
                            "type": "string",
                            "description": "Optional human-readable description of what this subagent call is doing (shown in the TUI)."
                        }
                    },
                    "required": ["prompt"]
                }),
            });
        }

        for server in &config.servers {
            tracing::debug!(server_name = %server.name, builtin = ?server.builtin, has_command = server.command.is_some(), "processing MCP server");
            if server.builtin == Some("bash".to_string()) {
                tracing::info!(server_name = %server.name, "discovered bash builtin tool");
                tools.push(McpTool {
                    name: "oxide_bash".to_string(),
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
                    Ok(mut client) => match client.list_tools().await {
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
                    },
                    Err(e) => {
                        warn!("failed to connect to MCP server '{}': {}", server.name, e);
                    }
                }
            }
        }

        tracing::info!(
            total_tools = tools.len(),
            total_clients = clients.len(),
            "MCP manager initialized successfully"
        );
        for tool in &tools {
            tracing::info!(tool_name = %tool.name, tool_desc = %tool.description, "MCP tool loaded");
        }
        Ok(Self {
            tools,
            clients,
            skills,
            dust_client,
            current_depth: 0,
            event_tx: None,
            next_call_id: std::sync::atomic::AtomicU64::new(0),
        })
    }

    pub fn list_tools(&self) -> Vec<McpTool> {
        self.tools.clone()
    }

    pub fn set_event_tx(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<crate::dust::client::DustEvent>,
    ) {
        self.event_tx = Some(tx);
    }

    /// Check if a tool is a built-in oxide tool that should be auto-approved.
    pub fn is_builtin_tool(name: &str) -> bool {
        matches!(name, "oxide_agent" | "oxide_bash" | "oxide_skill")
    }

    #[allow(clippy::too_many_lines)]
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> Result<ToolResult> {
        if tool_name == "oxide_skill" {
            let skill_id = input
                .get("skill_id")
                .and_then(|v| v.as_str())
                .context("oxide_skill requires 'skill_id'")?;

            // Security: validate skill ID format
            anyhow::ensure!(
                crate::skills::is_valid_skill_id(skill_id),
                "invalid skill_id: must contain only alphanumeric characters, hyphens, or underscores"
            );

            // Look up skill by id in discovered skills to get absolute path
            let skill = self
                .skills
                .iter()
                .find(|s| s.id == skill_id)
                .ok_or_else(|| anyhow::anyhow!("skill '{skill_id}' not found"))?;

            let content = std::fs::read_to_string(&skill.path)
                .with_context(|| format!("failed to read skill file: {}", skill.path.display()))?;

            return Ok(ToolResult {
                content,
                is_error: false,
                tool_use_id: String::new(),
            });
        }

        if tool_name == "oxide_bash" {
            let command = input
                .get("command")
                .and_then(|c| c.as_str())
                .context("bash tool requires 'command' argument")?;

            let mut result = BashTool::execute(command).await;
            result.tool_use_id = String::new();
            return Ok(result);
        }

        if tool_name == "oxide_agent" {
            let prompt = input
                .get("prompt")
                .and_then(|v| v.as_str())
                .context("oxide_agent requires 'prompt'")?
                .to_string();

            let agent_id = input
                .get("agent_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let description = input
                .get("description")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let depth = self.current_depth;

            let client = self
                .dust_client
                .as_ref()
                .context("oxide_agent: no Dust client available")?
                .clone();

            // Generate unique call_id
            let call_id = self
                .next_call_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                .to_string();

            // Send SubagentStarted event
            if let Some(ref tx) = self.event_tx {
                let _ = tx.send(crate::dust::client::DustEvent::SubagentStarted {
                    call_id: call_id.clone(),
                    description: description.clone(),
                });
            }

            let result = crate::mcp::subagent::run_subagent_with_timeout(
                &client,
                prompt,
                agent_id,
                depth,
                description.clone(),
            )
            .await;

            let success = result.is_ok();

            // Send SubagentFinished event
            if let Some(ref tx) = self.event_tx {
                let _ = tx.send(crate::dust::client::DustEvent::SubagentFinished {
                    call_id,
                    description,
                    success,
                });
            }

            return match result {
                Ok(text) => Ok(ToolResult {
                    content: text,
                    is_error: false,
                    tool_use_id: String::new(),
                }),
                Err(e) => Ok(ToolResult {
                    content: format!("subagent failed: {e}"),
                    is_error: true,
                    tool_use_id: String::new(),
                }),
            };
        }

        for client in &mut self.clients {
            if let Ok(output) = client.call_tool(tool_name, input.clone()).await {
                return Ok(ToolResult {
                    tool_use_id: String::new(),
                    content: output,
                    is_error: false,
                });
            }
        }

        anyhow::bail!("tool '{tool_name}' not found")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_builtin_tool_recognises_oxide_tools() {
        assert!(McpManager::is_builtin_tool("oxide_agent"));
        assert!(McpManager::is_builtin_tool("oxide_bash"));
        assert!(McpManager::is_builtin_tool("oxide_skill"));
    }

    #[test]
    fn is_builtin_tool_rejects_unknown_tools() {
        assert!(!McpManager::is_builtin_tool("my_custom_tool"));
        assert!(!McpManager::is_builtin_tool(""));
        assert!(!McpManager::is_builtin_tool("oxide_agent_extra"));
    }

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

        let manager = McpManager::init(&config, vec![], None).await.expect("init");
        let tools = manager.list_tools();

        // oxide_skill is always registered, so we should have 2 tools
        assert_eq!(tools.len(), 2);
        let bash_tool = tools.iter().find(|t| t.name == "oxide_bash");
        assert!(bash_tool.is_some());
        let skill_tool = tools.iter().find(|t| t.name == "oxide_skill");
        assert!(skill_tool.is_some());
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

        let mut manager = McpManager::init(&config, vec![], None).await.expect("init");
        let result = manager
            .call_tool("oxide_bash", serde_json::json!({"command": "echo hello"}))
            .await
            .expect("call");

        assert!(!result.is_error);
        assert_eq!(result.content, "hello");
    }

    #[tokio::test]
    async fn bash_tool_runs_ls_command() {
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

        let mut manager = McpManager::init(&config, vec![], None).await.expect("init");
        let result = manager
            .call_tool("oxide_bash", serde_json::json!({"command": "ls -al /tmp"}))
            .await
            .expect("call");

        // ls -al should succeed and return output
        assert!(!result.is_error);
        assert!(!result.content.is_empty());
        // Output should contain typical ls fields like 'total' or directory entries
        assert!(result.content.contains("total") || result.content.len() > 10);
    }

    #[tokio::test]
    async fn oxide_skill_always_in_tool_list() {
        let config = McpConfig {
            auto_approve: false,
            servers: vec![],
        };

        let manager = McpManager::init(&config, vec![], None).await.expect("init");
        let tools = manager.list_tools();

        let oxide_skill = tools.iter().find(|t| t.name == "oxide_skill");
        assert!(oxide_skill.is_some());
    }

    #[tokio::test]
    async fn oxide_skill_rejects_path_traversal() {
        let config = McpConfig {
            auto_approve: false,
            servers: vec![],
        };

        let mut manager = McpManager::init(&config, vec![], None).await.expect("init");

        // Test rejection of path traversal with ..
        let result = manager
            .call_tool(
                "oxide_skill",
                serde_json::json!({"skill_id": "../../../etc/passwd"}),
            )
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid skill_id"));

        // Test rejection of path traversal with /
        let result = manager
            .call_tool("oxide_skill", serde_json::json!({"skill_id": "etc/passwd"}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid skill_id"));

        // Test rejection of empty string
        let result = manager
            .call_tool("oxide_skill", serde_json::json!({"skill_id": ""}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid skill_id"));
    }

    #[tokio::test]
    async fn oxide_skill_accepts_valid_skill_ids() {
        let config = McpConfig {
            auto_approve: false,
            servers: vec![],
        };

        let mut manager = McpManager::init(&config, vec![], None).await.expect("init");

        // Test acceptance of alphanumeric with hyphens
        let result = manager
            .call_tool(
                "oxide_skill",
                serde_json::json!({"skill_id": "code-review"}),
            )
            .await;
        // Should fail with "not found" (skill doesn't exist), not "invalid skill_id"
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(!err_msg.contains("invalid skill_id"));
        assert!(err_msg.contains("not found"));

        // Test acceptance of alphanumeric with underscores
        let result = manager
            .call_tool("oxide_skill", serde_json::json!({"skill_id": "foo_bar_1"}))
            .await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(!err_msg.contains("invalid skill_id"));
        assert!(err_msg.contains("not found"));
    }

    #[tokio::test]
    async fn oxide_skill_missing_skill_returns_error() {
        let config = McpConfig {
            auto_approve: false,
            servers: vec![],
        };

        let mut manager = McpManager::init(&config, vec![], None).await.expect("init");
        let result = manager
            .call_tool(
                "oxide_skill",
                serde_json::json!({"skill_id": "nonexistent-skill"}),
            )
            .await;

        assert!(result.is_err());
    }
}
