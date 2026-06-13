use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    dust: DustConfig,
    #[serde(default)]
    mcp: McpConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DustConfig {
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default)]
    pub builtin: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };

        if !path.exists() {
            return Ok(Self::default());
        }

        let body = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        toml::from_str(&body).context("failed to parse oxide config")
    }

    pub fn agent_id(&self) -> Option<&str> {
        self.dust.agent_id.as_deref()
    }

    pub fn mcp(&self) -> &McpConfig {
        &self.mcp
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("oxide").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent_id_from_dust_table() {
        let config = toml::from_str::<Config>(
            r#"
                [dust]
                agent_id = "agent_123"
            "#,
        )
        .expect("parse");

        assert_eq!(config.agent_id(), Some("agent_123"));
    }

    #[test]
    fn defaults_when_dust_table_is_missing() {
        let config = toml::from_str::<Config>("").expect("parse");
        assert_eq!(config.agent_id(), None);
    }

    #[test]
    fn mcp_config_has_bash_server() {
        let config = toml::from_str::<Config>(
            r#"
                [[mcp.servers]]
                name = "bash"
                builtin = "bash"
            "#,
        )
        .expect("parse");

        let mcp_cfg = config.mcp();
        assert_eq!(mcp_cfg.servers.len(), 1);
        assert_eq!(mcp_cfg.servers[0].name, "bash");
        assert_eq!(mcp_cfg.servers[0].builtin, Some("bash".to_string()));
    }

    #[test]
    fn parses_mcp_builtin_bash_server() {
        let config = toml::from_str::<Config>(
            r#"
                [[mcp.servers]]
                name = "bash"
                builtin = "bash"
            "#,
        )
        .expect("parse");

        let servers = &config.mcp().servers;
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "bash");
        assert_eq!(servers[0].builtin, Some("bash".to_string()));
        assert_eq!(servers[0].command, None);
    }

    #[test]
    fn parses_mcp_external_server() {
        let config = toml::from_str::<Config>(
            r#"
                [mcp]
                auto_approve = false

                [[mcp.servers]]
                name = "my-tools"
                command = "npx"
                args = ["-y", "@my-org/mcp-server"]
            "#,
        )
        .expect("parse");

        assert_eq!(config.mcp().auto_approve, false);
        let servers = &config.mcp().servers;
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "my-tools");
        assert_eq!(servers[0].command, Some("npx".to_string()));
        assert_eq!(servers[0].args, vec!["-y", "@my-org/mcp-server"]);
    }

    #[test]
    fn defaults_mcp_config_when_missing() {
        let config = toml::from_str::<Config>("").expect("parse");
        assert_eq!(config.mcp().auto_approve, false);
        assert_eq!(config.mcp().servers.len(), 0);
    }
}
