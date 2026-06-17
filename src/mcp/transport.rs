use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, watch};
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

use crate::mcp::{McpManager, ToolCall};

const HEARTBEAT_INTERVAL: Duration = Duration::from_mins(2);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const REGISTER_BACKOFF_MIN: Duration = Duration::from_secs(1);
const REGISTER_BACKOFF_MAX: Duration = Duration::from_secs(30);

#[derive(Debug, serde::Deserialize)]
struct SseEnvelope {
    #[serde(rename = "eventId")]
    event_id: Option<String>,
    data: Option<serde_json::Value>,
}

pub struct McpTransport {
    http: reqwest::Client,
    base_url: String,
    workspace_id: String,
    manager: Arc<Mutex<McpManager>>,
    /// Sender used to forward tool calls back to the TUI event loop for approval.
    tool_tx: mpsc::UnboundedSender<ToolCall>,
    /// Sender used to notify main of the registered `server_id`.
    server_id_tx: mpsc::UnboundedSender<String>,
}

impl McpTransport {
    pub const fn new(
        http: reqwest::Client,
        base_url: String,
        workspace_id: String,
        manager: Arc<Mutex<McpManager>>,
        tool_tx: mpsc::UnboundedSender<ToolCall>,
        server_id_tx: mpsc::UnboundedSender<String>,
    ) -> Self {
        Self {
            http,
            base_url,
            workspace_id,
            manager,
            tool_tx,
            server_id_tx,
        }
    }

    pub async fn run(self) -> Result<()> {
        let server_id = self.register().await?;
        info!(server_id = %server_id, "MCP server registered with Dust");
        let _ = self.server_id_tx.send(server_id.clone());

        // Watch channel lets the heartbeat task always use the current server_id even after
        // re-registration on reconnect.
        let (sid_watch_tx, sid_watch_rx) = watch::channel(server_id.clone());

        // Spawn heartbeat task
        let http = self.http.clone();
        let base_url = self.base_url.clone();
        let workspace_id = self.workspace_id.clone();
        tokio::spawn(async move {
            loop {
                sleep(HEARTBEAT_INTERVAL).await;
                let current_sid = sid_watch_rx.borrow().clone();
                if let Err(e) = heartbeat(&http, &base_url, &workspace_id, &current_sid).await {
                    warn!(error = %e, "MCP heartbeat failed");
                }
            }
        });

        // Main SSE loop — re-registers on disconnect so we always hold a live server_id.
        let mut current_server_id = server_id;
        let mut last_event_id: Option<String> = None;
        let mut register_backoff = REGISTER_BACKOFF_MIN;
        loop {
            match self.poll_loop(&current_server_id, &mut last_event_id).await {
                Ok(()) => break,
                Err(e) => {
                    error!(error = %e, "MCP request stream error, re-registering...");
                    sleep(RECONNECT_DELAY).await;
                    match self.register().await {
                        Ok(new_id) => {
                            info!(server_id = %new_id, "re-registered MCP server after disconnect");
                            current_server_id.clone_from(&new_id);
                            last_event_id = None;
                            let _ = self.server_id_tx.send(new_id.clone());
                            let _ = sid_watch_tx.send(new_id);
                            register_backoff = REGISTER_BACKOFF_MIN;
                        }
                        Err(re) => {
                            error!(error = %re, "failed to re-register MCP server, backing off {backoff_secs}s", backoff_secs = register_backoff.as_secs());
                            sleep(register_backoff).await;
                            register_backoff =
                                std::cmp::min(register_backoff * 2, REGISTER_BACKOFF_MAX);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn register(&self) -> Result<String> {
        let token = crate::auth::token_refresh::get_valid_token().await?;
        let url = format!(
            "{}/api/v1/w/{}/mcp/register",
            self.base_url, self.workspace_id
        );

        let res = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "serverName": "oxide-fs" }))
            .send()
            .await
            .context("failed to register MCP server")?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("MCP register failed {status}: {body}");
        }

        let body: serde_json::Value = res
            .json()
            .await
            .context("failed to parse register response")?;
        let server_id = body
            .get("serverId")
            .and_then(|v| v.as_str())
            .context("missing serverId in register response")?
            .to_string();

        Ok(server_id)
    }

    async fn poll_loop(&self, server_id: &str, last_event_id: &mut Option<String>) -> Result<()> {
        let token = crate::auth::token_refresh::get_valid_token().await?;
        let mut url = format!(
            "{}/api/v1/w/{}/mcp/requests?serverId={}",
            self.base_url, self.workspace_id, server_id
        );
        if let Some(eid) = last_event_id.as_deref() {
            url.push_str("&lastEventId=");
            url.push_str(eid);
        }

        debug!(url = %url, "opening MCP requests SSE stream");

        let response = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .context("failed to open MCP requests stream")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("MCP requests stream failed {status}: {body}");
        }

        let mut stream = crate::dust::stream::EventStream::new(response);
        loop {
            match stream.next_raw_line().await {
                Ok(Some(raw)) => {
                    let line = raw.trim().to_string();
                    if line.is_empty() || !line.starts_with("data:") {
                        continue;
                    }
                    let data = line.trim_start_matches("data:").trim();
                    if data == "done" {
                        continue;
                    }

                    let envelope: SseEnvelope = match serde_json::from_str(data) {
                        Ok(e) => e,
                        Err(e) => {
                            debug!(error = %e, raw = %data, "failed to parse MCP SSE event");
                            continue;
                        }
                    };

                    if let Some(eid) = envelope.event_id {
                        *last_event_id = Some(eid);
                    }

                    if let Some(request) = envelope.data {
                        debug!(request = %request, "received MCP request from Dust");
                        if let Err(e) = self.handle_request(server_id, &request).await {
                            error!(error = %e, "failed to handle MCP request");
                        }
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    async fn handle_request(&self, server_id: &str, request: &serde_json::Value) -> Result<()> {
        let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");

        // JSON-RPC notifications have no `id` field — never send a response to them.
        let id = match request.get("id") {
            Some(v) if !v.is_null() => v.clone(),
            _ => {
                debug!(method = %method, "ignoring JSON-RPC notification (no response needed)");
                return Ok(());
            }
        };

        debug!(method = %method, "handling MCP request");

        let result = match method {
            "initialize" => {
                // Echo back the client's requested protocol version if we recognise it,
                // otherwise advertise the newest version we support.
                let client_version = request
                    .get("params")
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("2025-11-25");
                serde_json::json!({
                    "protocolVersion": client_version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "oxide-fs", "version": "0.1.0" }
                })
            }
            "tools/list" => {
                let tools: Vec<serde_json::Value> = self
                    .manager
                    .lock()
                    .await
                    .list_tools()
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.input_schema
                        })
                    })
                    .collect();
                serde_json::json!({ "tools": tools })
            }
            "tools/call" => {
                let name = request
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = request
                    .get("params")
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                // Send to TUI for approval (or auto-execute if auto_approve)
                let tool_call = ToolCall {
                    id: id.to_string(),
                    name,
                    input: arguments,
                };
                let _ = self.tool_tx.send(tool_call);

                // Return immediately — the TUI will post the result after approval
                return Ok(());
            }
            _ => {
                serde_json::json!({ "error": { "code": -32601, "message": format!("Method not found: {method}") } })
            }
        };

        self.post_result(
            server_id,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            }),
        )
        .await
    }

    pub async fn post_result(&self, server_id: &str, result: &serde_json::Value) -> Result<()> {
        let token = crate::auth::token_refresh::get_valid_token().await?;
        let url = format!(
            "{}/api/v1/w/{}/mcp/results",
            self.base_url, self.workspace_id
        );

        let res = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "serverId": server_id,
                "result": result
            }))
            .send()
            .await
            .context("failed to post MCP result")?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("MCP post result failed {status}: {body}");
        }

        Ok(())
    }
}

async fn heartbeat(
    http: &reqwest::Client,
    base_url: &str,
    workspace_id: &str,
    server_id: &str,
) -> Result<()> {
    let token = crate::auth::token_refresh::get_valid_token().await?;
    let url = format!("{base_url}/api/v1/w/{workspace_id}/mcp/heartbeat");

    let res = http
        .post(&url)
        .bearer_auth(&token)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "serverId": server_id }))
        .send()
        .await
        .context("heartbeat request failed")?;

    if res.status().is_success() {
        debug!("MCP heartbeat OK");
    } else {
        warn!(status = %res.status(), "MCP heartbeat not OK");
    }

    Ok(())
}
