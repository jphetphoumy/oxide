use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mcp::McpManager;
use crate::mcp::jsonrpc::{JsonRpcRequest, JsonRpcResponse};

pub struct McpJsonRpcServer {
    manager: Arc<Mutex<McpManager>>,
}

impl McpJsonRpcServer {
    pub const fn new(manager: Arc<Mutex<McpManager>>) -> Self {
        Self { manager }
    }

    #[allow(clippy::future_not_send)]
    pub async fn run(&self) -> anyhow::Result<()> {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());
        let mut stdout = std::io::stdout();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Error reading stdin: {e}");
                    continue;
                }
            };

            if line.trim().is_empty() {
                continue;
            }

            match self.handle_request(&line).await {
                Ok(response) => {
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = writeln!(stdout, "{json}");
                        let _ = stdout.flush();
                    }
                }
                Err(e) => {
                    eprintln!("Error processing request: {e}");
                }
            }
        }

        Ok(())
    }

    async fn handle_request(&self, line: &str) -> anyhow::Result<JsonRpcResponse> {
        let request: JsonRpcRequest = serde_json::from_str(line)?;

        match request.method.as_str() {
            "initialize" => Ok(Self::handle_initialize(&request)),
            "tools/list" => self.handle_tools_list(&request).await,
            "tools/call" => self.handle_tools_call(&request).await,
            _ => Ok(JsonRpcResponse::error(
                request.id,
                -32601,
                format!("Method not found: {}", request.method),
            )),
        }
    }

    fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
        let response = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "oxide",
                "version": "0.1.0"
            }
        });

        JsonRpcResponse::success(request.id, response)
    }

    async fn handle_tools_list(&self, request: &JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let tools = self.manager.lock().await.list_tools();

        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema
                })
            })
            .collect();

        let response = serde_json::json!({
            "tools": tools_json
        });

        Ok(JsonRpcResponse::success(request.id, response))
    }

    async fn handle_tools_call(&self, request: &JsonRpcRequest) -> anyhow::Result<JsonRpcResponse> {
        let name = request
            .params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' parameter"))?;

        let input = request
            .params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let mut manager = self.manager.lock().await;
        match manager.call_tool(name, input).await {
            Ok(result) => {
                let response = serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": result.content
                        }
                    ],
                    "isError": result.is_error
                });
                Ok(JsonRpcResponse::success(request.id, response))
            }
            Err(e) => {
                let response = serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Error: {}", e)
                        }
                    ],
                    "isError": true
                });
                Ok(JsonRpcResponse::success(request.id, response))
            }
        }
    }
}
