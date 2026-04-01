use std::fs;

use anyhow::Result;
use rig::{
    agent,
    client::{CompletionClient, Nothing},
    providers::ollama::{Client, CompletionModel},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

pub type TAgent = agent::Agent<CompletionModel>;

#[derive(Clone, Deserialize, Serialize)]
pub struct Config {
    pub model: String,
    pub ollama_url: String,
}

impl Config {
    const LOG_PATH: &'static str = "terrarium.log";
    const CONFIG_PATH: &'static str = "config.toml";

    pub fn init() -> Result<Self> {
        // start logger
        let log = std::fs::File::create(Self::LOG_PATH)?;
        tracing_subscriber::fmt()
            .with_writer(log)
            .with_ansi(false)
            .init();

        if fs::exists(Self::CONFIG_PATH)? {
            info!("Loading config from {}", Self::CONFIG_PATH);
            let data = fs::read_to_string(Self::CONFIG_PATH).unwrap_or_default();
            match toml::from_str(&data) {
                Ok(data) => Ok(data),
                Err(err) => {
                    warn!("Failed to parse config file: {}, using default config", err);
                    let config = Self::default();
                    config.save()?;
                    Ok(config)
                }
            }
        } else {
            warn!(
                "Config file {} not found, using default config",
                Self::CONFIG_PATH
            );
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let data = toml::to_string_pretty(self)?;
        fs::write(Self::CONFIG_PATH, data)?;
        Ok(())
    }

    pub fn agent(&self) -> Result<TAgent> {
        let client = Client::builder()
            .base_url(self.ollama_url.clone())
            .api_key(Nothing)
            .build()?;
        Ok(client.agent(&self.model).additional_params(json!({
            "think": false,
        })).build())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: "isotnek/qwen3.5:9B-Unsloth-UD-Q4_K_XL".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
        }
    }
}
