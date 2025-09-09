use crate::api::types::ApiConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfigTemplate {
    pub name: String,
    pub description: String,
    pub config: ApiConfig,
    pub environment: String,
}

impl ApiConfigTemplate {
    pub fn development() -> Self {
        Self {
            name: "Development".to_string(),
            description: "Development environment configuration".to_string(),
            config: ApiConfig {
                base_url: "https://api.hyperliquid-testnet.xyz".to_string(),
                ws_url: "wss://api.hyperliquid-testnet.xyz/ws".to_string(),
                timeout_ms: 10000,
                max_retries: 5,
                retry_delay_ms: 2000,
            },
            environment: "development".to_string(),
        }
    }

    pub fn staging() -> Self {
        Self {
            name: "Staging".to_string(),
            description: "Staging environment configuration".to_string(),
            config: ApiConfig {
                base_url: "https://api.hyperliquid.xyz".to_string(),
                ws_url: "wss://api.hyperliquid.xyz/ws".to_string(),
                timeout_ms: 5000,
                max_retries: 3,
                retry_delay_ms: 1000,
            },
            environment: "staging".to_string(),
        }
    }

    pub fn production() -> Self {
        Self {
            name: "Production".to_string(),
            description: "Production environment configuration".to_string(),
            config: ApiConfig {
                base_url: "https://api.hyperliquid.xyz".to_string(),
                ws_url: "wss://api.hyperliquid.xyz/ws".to_string(),
                timeout_ms: 3000,
                max_retries: 2,
                retry_delay_ms: 500,
            },
            environment: "production".to_string(),
        }
    }

    pub fn get_all_templates() -> HashMap<String, Self> {
        let mut templates = HashMap::new();
        templates.insert("development".to_string(), Self::development());
        templates.insert("staging".to_string(), Self::staging());
        templates.insert("production".to_string(), Self::production());
        templates
    }
}
