use crate::api::types::*;
use crate::api::auth::HyperLiquidAuth;
use crate::trading::types::Position;
use anyhow::Result;
use crossbeam_channel::{Sender, Receiver, unbounded};
use dashmap::DashMap;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::str::FromStr;
use tracing::{error, info, debug};

#[derive(Debug, Clone)]
pub struct AccountApi {
    pub auth: HyperLiquidAuth,
    pub config: ApiConfig,
    pub positions: Arc<DashMap<String, Position>>,
    pub account_info: Arc<RwLock<Option<HyperLiquidAccountInfo>>>,
    pub account_events_tx: Sender<ApiEvent>,
    pub last_update: Arc<RwLock<std::time::Instant>>,
}

impl AccountApi {
    pub fn new(auth: HyperLiquidAuth, config: ApiConfig) -> (Self, Receiver<ApiEvent>) {
        let (tx, rx) = unbounded();
        
        let api = Self {
            auth,
            config,
            positions: Arc::new(DashMap::new()),
            account_info: Arc::new(RwLock::new(None)),
            account_events_tx: tx,
            last_update: Arc::new(RwLock::new(std::time::Instant::now())),
        };
        
        (api, rx)
    }

    pub async fn get_account_info(&self) -> Result<HyperLiquidAccountInfo, ApiError> {
        let info_request = HyperLiquidInfoRequest {
            type_: "clearinghouseState".to_string(),
            user: self.auth.account_id.map(|id| id.to_string()),
        };

        let signed_request = self.auth.create_signed_request("info", &info_request)?;
        let headers = self.auth.get_headers()?;

        let response = self.auth.client
            .post(&format!("{}/info", self.config.base_url))
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::NetworkError(format!(
                "Account info request failed with status {}: {}", 
                status, 
                error_text
            )));
        }

        let account_response: HyperLiquidUserStateResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if account_response.status != "ok" {
            return Err(ApiError::NetworkError(
                "Account info response status not ok".to_string()
            ));
        }

        let user_state = account_response.response
            .ok_or_else(|| ApiError::ParseError("No user state in response".to_string()))?;

        let account_info = HyperLiquidAccountInfo {
            margin_summary: user_state.margin_summary,
            open_orders: user_state.open_orders,
            asset_positions: user_state.asset_positions,
        };

        // Update cached account info
        {
            let mut cached_info = self.account_info.write();
            *cached_info = Some(account_info.clone());
        }

        // Update positions
        self.update_positions_from_account_info(&account_info).await;

        // Update last update time
        {
            let mut last_update = self.last_update.write();
            *last_update = std::time::Instant::now();
        }

        info!("Account info updated successfully");
        Ok(account_info)
    }

    pub async fn get_positions(&self) -> Result<Vec<Position>, ApiError> {
        let account_info = self.get_account_info().await?;
        
        let positions: Vec<Position> = account_info.asset_positions
            .into_iter()
            .filter_map(|hl_pos| {
                let size = Decimal::from_str(&hl_pos.szi).ok()?;
                let entry_price = Decimal::from_str(&hl_pos.entry_px).ok()?;
                let unrealized_pnl = Decimal::from_str(&hl_pos.unrealized_pnl).ok()?;
                
                // Use entry price as mark price for now
                let mark_price = entry_price;
                
                Some(Position {
                    symbol: hl_pos.coin,
                    size,
                    entry_price,
                    mark_price,
                    unrealized_pnl,
                    realized_pnl: Decimal::ZERO, // This would need to be tracked separately
                    updated_at: chrono::Utc::now(),
                })
            })
            .collect();

        Ok(positions)
    }

    pub async fn get_balance(&self) -> Result<Decimal, ApiError> {
        let account_info = self.get_account_info().await?;
        let balance_str = &account_info.margin_summary.account_value;
        
        Decimal::from_str(balance_str)
            .map_err(|e| ApiError::ParseError(format!("Failed to parse balance: {}", e)))
    }

    pub async fn get_margin_used(&self) -> Result<Decimal, ApiError> {
        let account_info = self.get_account_info().await?;
        let margin_str = &account_info.margin_summary.total_margin_used;
        
        Decimal::from_str(margin_str)
            .map_err(|e| ApiError::ParseError(format!("Failed to parse margin used: {}", e)))
    }

    pub async fn get_withdrawable(&self) -> Result<Decimal, ApiError> {
        let info_request = HyperLiquidInfoRequest {
            type_: "clearinghouseState".to_string(),
            user: self.auth.account_id.map(|id| id.to_string()),
        };

        let signed_request = self.auth.create_signed_request("info", &info_request)?;
        let headers = self.auth.get_headers()?;

        let response = self.auth.client
            .post(&format!("{}/info", self.config.base_url))
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::NetworkError(
                format!("Withdrawable request failed with status: {}", response.status())
            ));
        }

        let account_response: HyperLiquidUserStateResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if account_response.status != "ok" {
            return Err(ApiError::NetworkError(
                "Withdrawable response status not ok".to_string()
            ));
        }

        let user_state = account_response.response
            .ok_or_else(|| ApiError::ParseError("No user state in response".to_string()))?;

        Decimal::from_str(&user_state.withdrawable)
            .map_err(|e| ApiError::ParseError(format!("Failed to parse withdrawable: {}", e)))
    }

    pub async fn get_open_orders(&self) -> Result<Vec<HyperLiquidOrderRest>, ApiError> {
        let account_info = self.get_account_info().await?;
        Ok(account_info.open_orders)
    }

    pub async fn get_fills(&self, start_time: Option<u64>, end_time: Option<u64>) -> Result<Vec<HyperLiquidFill>, ApiError> {
        let fills_request = HyperLiquidFillsRequest {
            user: self.auth.account_id.map(|id| id.to_string()),
            start_time,
            end_time,
        };

        let signed_request = self.auth.create_signed_request("info", &fills_request)?;
        let headers = self.auth.get_headers()?;

        let response = self.auth.client
            .post(&format!("{}/info", self.config.base_url))
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::NetworkError(
                format!("Fills request failed with status: {}", response.status())
            ));
        }

        let fills_response: HyperLiquidFillsResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if fills_response.status != "ok" {
            return Err(ApiError::NetworkError(
                "Fills response status not ok".to_string()
            ));
        }

        Ok(fills_response.response.unwrap_or_default())
    }

    async fn update_positions_from_account_info(&self, account_info: &HyperLiquidAccountInfo) {
        for hl_position in &account_info.asset_positions {
            if let (Ok(size), Ok(entry_price), Ok(unrealized_pnl)) = (
                Decimal::from_str(&hl_position.szi),
                Decimal::from_str(&hl_position.entry_px),
                Decimal::from_str(&hl_position.unrealized_pnl),
            ) {
                let position = Position {
                    symbol: hl_position.coin.clone(),
                    size,
                    entry_price,
                    mark_price: entry_price, // Use entry price as mark price for now
                    unrealized_pnl,
                    realized_pnl: Decimal::ZERO,
                    updated_at: chrono::Utc::now(),
                };

                self.positions.insert(hl_position.coin.clone(), position.clone());

                // Send position update event
                let _ = self.account_events_tx.send(ApiEvent::PositionUpdate {
                    coin: hl_position.coin.clone(),
                    size: hl_position.szi.clone(),
                    entry_price: hl_position.entry_px.clone(),
                    unrealized_pnl: hl_position.unrealized_pnl.clone(),
                });
            }
        }

        // Send account update event
        let _ = self.account_events_tx.send(ApiEvent::AccountUpdate {
            account_value: account_info.margin_summary.account_value.clone(),
            margin_used: account_info.margin_summary.total_margin_used.clone(),
            withdrawable: "0".to_string(), // Would need separate call to get withdrawable
        });
    }

    pub fn get_cached_position(&self, symbol: &str) -> Option<Position> {
        self.positions.get(symbol).map(|entry| entry.value().clone())
    }

    pub fn get_all_cached_positions(&self) -> Vec<Position> {
        self.positions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn get_cached_account_info(&self) -> Option<HyperLiquidAccountInfo> {
        self.account_info.read().clone()
    }

    pub fn is_data_fresh(&self, max_age_seconds: u64) -> bool {
        let last_update = *self.last_update.read();
        last_update.elapsed().as_secs() < max_age_seconds
    }

    pub async fn start_periodic_updates(&self, interval_seconds: u64) {
        let positions = Arc::clone(&self.positions);
        let account_info = Arc::clone(&self.account_info);
        let last_update = Arc::clone(&self.last_update);
        let account_events_tx = self.account_events_tx.clone();
        let auth = self.auth.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_seconds));
            
            loop {
                interval.tick().await;
                
                match Self::fetch_account_info(&auth, &config).await {
                    Ok(info) => {
                        // Update cached data
                        {
                            let mut cached_info = account_info.write();
                            *cached_info = Some(info.clone());
                        }
                        
                        {
                            let mut last_update_guard = last_update.write();
                            *last_update_guard = std::time::Instant::now();
                        }

                        // Update positions
                        for hl_position in &info.asset_positions {
                            if let (Ok(size), Ok(entry_price), Ok(unrealized_pnl)) = (
                                Decimal::from_str(&hl_position.szi),
                                Decimal::from_str(&hl_position.entry_px),
                                Decimal::from_str(&hl_position.unrealized_pnl),
                            ) {
                                let position = Position {
                                    symbol: hl_position.coin.clone(),
                                    size,
                                    entry_price,
                                    mark_price: entry_price,
                                    unrealized_pnl,
                                    realized_pnl: Decimal::ZERO,
                                    updated_at: chrono::Utc::now(),
                                };

                                positions.insert(hl_position.coin.clone(), position.clone());

                                let _ = account_events_tx.send(ApiEvent::PositionUpdate {
                                    coin: hl_position.coin.clone(),
                                    size: hl_position.szi.clone(),
                                    entry_price: hl_position.entry_px.clone(),
                                    unrealized_pnl: hl_position.unrealized_pnl.clone(),
                                });
                            }
                        }

                        let _ = account_events_tx.send(ApiEvent::AccountUpdate {
                            account_value: info.margin_summary.account_value.clone(),
                            margin_used: info.margin_summary.total_margin_used.clone(),
                            withdrawable: "0".to_string(),
                        });

                        debug!("Periodic account update completed");
                    }
                    Err(e) => {
                        error!("Failed to fetch account info during periodic update: {}", e);
                    }
                }
            }
        });
    }

    async fn fetch_account_info(
        auth: &HyperLiquidAuth,
        config: &ApiConfig,
    ) -> Result<HyperLiquidAccountInfo, ApiError> {
        let info_request = HyperLiquidInfoRequest {
            type_: "clearinghouseState".to_string(),
            user: auth.account_id.map(|id| id.to_string()),
        };

        let signed_request = auth.create_signed_request("info", &info_request)?;
        let headers = auth.get_headers()?;

        let response = auth.client
            .post(&format!("{}/info", config.base_url))
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiError::NetworkError(
                format!("Account info request failed with status: {}", response.status())
            ));
        }

        let account_response: HyperLiquidUserStateResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if account_response.status != "ok" {
            return Err(ApiError::NetworkError(
                "Account info response status not ok".to_string()
            ));
        }

        let user_state = account_response.response
            .ok_or_else(|| ApiError::ParseError("No user state in response".to_string()))?;

        Ok(HyperLiquidAccountInfo {
            margin_summary: user_state.margin_summary,
            open_orders: user_state.open_orders,
            asset_positions: user_state.asset_positions,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidInfoRequest {
    #[serde(rename = "type")]
    pub type_: String,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidFillsRequest {
    pub user: Option<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidFillsResponse {
    pub status: String,
    pub response: Option<Vec<HyperLiquidFill>>,
}

// Clone implementation removed to avoid conflicts
