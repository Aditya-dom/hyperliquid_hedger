use crate::api::types::ApiError;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct HyperLiquidAuth {
    pub private_key: String,
    pub account_id: Option<u64>,
    pub client: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub action: String,
    pub nonce: u64,
}

impl HyperLiquidAuth {
    pub fn new(private_key: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            private_key,
            account_id: None,
            client,
        }
    }

    pub fn with_account_id(mut self, account_id: u64) -> Self {
        self.account_id = Some(account_id);
        self
    }

    pub fn get_nonce(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    pub fn sign_message(&self, message: &str) -> Result<String, ApiError> {
        // HyperLiquid uses Ed25519 signing
        // This is a simplified implementation - in production you'd use proper Ed25519
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        hasher.update(self.private_key.as_bytes());
        let result = hasher.finalize();
        
        Ok(hex::encode(&result))
    }

    pub fn create_signed_request<T: Serialize>(&self, action: &str, data: &T) -> Result<HyperLiquidSignedRequest, ApiError> {
        let nonce = self.get_nonce();
        let message = format!("{}{}", action, serde_json::to_string(data)?);
        let signature = self.sign_message(&message)?;

        Ok(HyperLiquidSignedRequest {
            action: action.to_string(),
            nonce,
            signature,
            data: serde_json::to_value(data)?,
        })
    }

    pub fn get_headers(&self) -> Result<reqwest::header::HeaderMap, ApiError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse()?);
        
        if let Some(account_id) = self.account_id {
            headers.insert("X-Account-Id", account_id.to_string().parse()?);
        }

        Ok(headers)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidSignedRequest {
    pub action: String,
    pub nonce: u64,
    pub signature: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidAuthResponse {
    pub status: String,
    pub response: Option<serde_json::Value>,
}

impl HyperLiquidAuth {
    pub async fn authenticate(&mut self) -> Result<(), ApiError> {
        let auth_request = AuthRequest {
            action: "info".to_string(),
            nonce: self.get_nonce(),
        };

        let signed_request = self.create_signed_request("info", &auth_request)?;
        let headers = self.get_headers()?;

        let response = self.client
            .post("https://api.hyperliquid.xyz/info")
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::AuthenticationError(
                format!("Authentication failed with status: {}", response.status())
            ));
        }

        let auth_response: HyperLiquidAuthResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if auth_response.status != "ok" {
            return Err(ApiError::AuthenticationError(
                "Authentication response status not ok".to_string()
            ));
        }

        Ok(())
    }

    pub fn is_authenticated(&self) -> bool {
        self.account_id.is_some()
    }
}

// Helper function for hex encoding
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}
