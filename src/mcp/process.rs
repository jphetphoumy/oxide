use std::process::Stdio;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};

use crate::config::McpServerConfig;

pub struct McpProcess {
    child: Child,
    writer: tokio::io::BufWriter<tokio::process::ChildStdin>,
    reader: tokio::io::BufReader<tokio::process::ChildStdout>,
    request_id: u64,
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl McpProcess {
    pub fn spawn(config: &McpServerConfig) -> Result<Self> {
        let Some(ref command) = config.command else {
            anyhow::bail!("MCP server '{}' has no command", &config.name);
        };

        let mut cmd = Command::new(command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn MCP server '{}'", config.name))?;

        let stdin = child
            .stdin
            .take()
            .context("failed to get stdin from MCP server")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to get stdout from MCP server")?;

        let writer = tokio::io::BufWriter::new(stdin);
        let reader = tokio::io::BufReader::new(stdout);

        Ok(McpProcess {
            child,
            writer,
            reader,
            request_id: 0,
        })
    }

    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        self.request_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params,
        });

        let request_str = serde_json::to_string(&request)?;
        self.writer.write_all(request_str.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        let mut response_line = String::new();
        self.reader.read_line(&mut response_line).await?;

        let response: serde_json::Value = serde_json::from_str(&response_line)?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("MCP error: {error}");
        }

        Ok(response
            .get("result")
            .context("no result in MCP response")?
            .clone())
    }
}
