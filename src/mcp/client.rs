use anyhow::Result;
use serde_json::json;

use super::process::McpProcess;
use super::types::McpTool;
use crate::config::McpServerConfig;

pub struct McpClient {
    process: McpProcess,
}

impl McpClient {
    pub async fn connect(config: &McpServerConfig) -> Result<Self> {
        let mut process = McpProcess::spawn(config)?;
        process
            .call(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "oxide",
                        "version": "0.1.0"
                    }
                }),
            )
            .await?;

        Ok(McpClient { process })
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let response = self.process.call("tools/list", json!({})).await?;

        let tools = response
            .get("tools")
            .and_then(|t| t.as_array())
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|tool| serde_json::from_value(tool.clone()).ok())
            .collect();

        Ok(tools)
    }

    pub async fn call_tool(&mut self, tool_name: &str, input: serde_json::Value) -> Result<String> {
        let response = self
            .process
            .call(
                "tools/call",
                json!({
                    "name": tool_name,
                    "arguments": input
                }),
            )
            .await?;

        let content = response
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        Ok(content.to_string())
    }
}
