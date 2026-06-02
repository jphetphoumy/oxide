use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    dust: DustConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct DustConfig {
    #[serde(default)]
    agent_id: Option<String>,
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
}
