use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeys {
    pub api_id: i32,
    pub api_hash: String,
}

impl ApiKeys {
    fn get_config_path() -> Result<PathBuf> {
        let data_dir = ProjectDirs::from("com", "unlimcloud", "unlim-cloud")
            .ok_or_else(|| anyhow::anyhow!("Failed to get data directory"))?
            .data_dir()
            .to_path_buf();
        
        Ok(data_dir.join("api_keys.json"))
    }

    pub async fn load() -> Result<Option<Self>> {
        let config_path = Self::get_config_path()?;
        
        if !config_path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&config_path).await
            .context("Failed to read API keys file")?;
        
        let keys: ApiKeys = serde_json::from_str(&content)
            .context("Failed to parse API keys file")?;
        
        Ok(Some(keys))
    }

    pub async fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        
        // Ensure directory exists
        if let Some(parent) = config_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .context("Failed to create config directory")?;
        }

        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize API keys")?;
        
        tokio::fs::write(&config_path, content).await
            .context("Failed to write API keys file")?;
        
        Ok(())
    }

    pub async fn exists() -> bool {
        match Self::get_config_path() {
            Ok(path) => path.exists(),
            Err(_) => false,
        }
    }
}
