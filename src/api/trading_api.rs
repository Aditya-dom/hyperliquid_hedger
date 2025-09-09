use crate::api::types::*;
use crate::api::auth::HyperLiquidAuth;
use crate::trading::types::{NewOrder, OrderType, Side};
use anyhow::Result;
use crossbeam_channel::{Sender, Receiver, unbounded};
use dashmap::DashMap;
use tokio::sync::RwLock;
use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, debug};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TradingApi {
    pub auth: HyperLiquidAuth,
    pub config: ApiConfig,
    pub pending_orders: Arc<DashMap<u64, PendingOrder>>,
    pub order_events_tx: Sender<ApiEvent>,
    pub retry_queue: Arc<RwLock<Vec<RetryRequest>>>,
    pub rate_limiter: Arc<RwLock<RateLimiter>>,
}

#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub internal_id: Uuid,
    pub client_order_id: u64,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Decimal,
    pub size: Decimal,
    pub created_at: std::time::Instant,
    pub retry_count: u32,
}

#[derive(Debug, Clone)]
pub struct RetryRequest {
    pub order: PendingOrder,
    pub retry_after: std::time::Instant,
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    pub last_request: std::time::Instant,
    pub request_count: u32,
    pub window_start: std::time::Instant,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            last_request: std::time::Instant::now(),
            request_count: 0,
            window_start: std::time::Instant::now(),
        }
    }
}

impl TradingApi {
    pub fn new(auth: HyperLiquidAuth, config: ApiConfig) -> (Self, Receiver<ApiEvent>) {
        let (tx, rx) = unbounded();
        
        let api = Self {
            auth,
            config,
            pending_orders: Arc::new(DashMap::new()),
            order_events_tx: tx,
            retry_queue: Arc::new(RwLock::new(Vec::new())),
            rate_limiter: Arc::new(RwLock::new(RateLimiter::default())),
        };
        
        (api, rx)
    }

    pub async fn place_order(&self, order: NewOrder) -> Result<Uuid, ApiError> {
        let internal_id = Uuid::new_v4();
        let client_order_id = self.generate_client_order_id();
        
        let pending_order = PendingOrder {
            internal_id,
            client_order_id,
            symbol: order.symbol.clone(),
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            size: order.size,
            created_at: std::time::Instant::now(),
            retry_count: 0,
        };

        self.pending_orders.insert(client_order_id, pending_order.clone());

        match self.submit_order_to_exchange(&pending_order).await {
            Ok(_) => {
                info!("Order placed successfully: {} for {}", internal_id, order.symbol);
                Ok(internal_id)
            }
            Err(e) => {
                warn!("Failed to place order {}: {}", internal_id, e);
                self.pending_orders.remove(&client_order_id);
                Err(e)
            }
        }
    }

    pub async fn cancel_order(&self, internal_id: Uuid) -> Result<(), ApiError> {
        // Find the order by internal ID
        let client_order_id = self.pending_orders
            .iter()
            .find(|entry| entry.value().internal_id == internal_id)
            .map(|entry| *entry.key());

        let client_order_id = match client_order_id {
            Some(id) => id,
            None => return Err(ApiError::InvalidOrder("Order not found".to_string())),
        };

        self.cancel_order_by_client_id(client_order_id).await
    }

    pub async fn cancel_order_by_client_id(&self, client_order_id: u64) -> Result<(), ApiError> {
        let cancel_request = HyperLiquidCancelRequest {
            oid: client_order_id,
        };

        let signed_request = self.auth.create_signed_request("cancel", &cancel_request)?;
        let headers = self.auth.get_headers()?;

        self.enforce_rate_limit().await;

        let response = self.auth.client
            .post(&format!("{}/exchange", self.config.base_url))
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::OrderRejected(format!(
                "Cancel failed with status {}: {}", 
                status, 
                error_text
            )));
        }

        let cancel_response: HyperLiquidOrderResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if cancel_response.status != "ok" {
            return Err(ApiError::OrderRejected(
                "Cancel response status not ok".to_string()
            ));
        }

        self.pending_orders.remove(&client_order_id);
        info!("Order cancelled successfully: {}", client_order_id);
        Ok(())
    }

    pub async fn cancel_all_orders(&self, symbol: Option<&str>) -> Result<(), ApiError> {
        let orders_to_cancel: Vec<u64> = if let Some(symbol) = symbol {
            self.pending_orders
                .iter()
                .filter(|entry| entry.value().symbol == symbol)
                .map(|entry| *entry.key())
                .collect()
        } else {
            self.pending_orders
                .iter()
                .map(|entry| *entry.key())
                .collect()
        };

        for client_order_id in orders_to_cancel {
            if let Err(e) = self.cancel_order_by_client_id(client_order_id).await {
                warn!("Failed to cancel order {}: {}", client_order_id, e);
            }
        }

        Ok(())
    }

    async fn submit_order_to_exchange(&self, pending_order: &PendingOrder) -> Result<(), ApiError> {
        let hl_order = HyperLiquidOrder {
            a: self.auth.account_id,
            b: matches!(pending_order.side, Side::Buy),
            p: pending_order.price.to_string(),
            s: pending_order.size.to_string(),
            r: false, // reduce only
            t: self.map_order_type(&pending_order.order_type),
            cid: pending_order.client_order_id,
            oid: None,
        };

        let signed_request = self.auth.create_signed_request("order", &hl_order)?;
        let headers = self.auth.get_headers()?;

        self.enforce_rate_limit().await;

        let response = self.auth.client
            .post(&format!("{}/exchange", self.config.base_url))
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::OrderRejected(format!(
                "Order failed with status {}: {}", 
                status, 
                error_text
            )));
        }

        let order_response: HyperLiquidOrderResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if order_response.status != "ok" {
            return Err(ApiError::OrderRejected(
                "Order response status not ok".to_string()
            ));
        }

        debug!("Order submitted to exchange: {}", pending_order.client_order_id);
        Ok(())
    }

    fn map_order_type(&self, order_type: &OrderType) -> String {
        match order_type {
            OrderType::Market => "Market".to_string(),
            OrderType::Limit => "Limit".to_string(),
            OrderType::PostOnly => "Limit".to_string(), // HyperLiquid doesn't have PostOnly, use Limit
        }
    }

    fn generate_client_order_id(&self) -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    async fn enforce_rate_limit(&self) {
        {
            let mut rate_limiter = self.rate_limiter.write().await;
            let now = std::time::Instant::now();
            
            // Reset window if more than 1 second has passed
            if now.duration_since(rate_limiter.window_start) > Duration::from_secs(1) {
                rate_limiter.window_start = now;
                rate_limiter.request_count = 0;
            }
            
            // HyperLiquid rate limit: 100 requests per second
            if rate_limiter.request_count >= 100 {
                let sleep_duration = Duration::from_secs(1) - now.duration_since(rate_limiter.window_start);
                if sleep_duration > Duration::ZERO {
                    drop(rate_limiter); // Release lock before sleeping
                    sleep(sleep_duration).await;
                    // Re-acquire lock after sleeping
                    let mut rate_limiter = self.rate_limiter.write().await;
                    rate_limiter.request_count += 1;
                    rate_limiter.last_request = std::time::Instant::now();
                } else {
                    rate_limiter.request_count += 1;
                    rate_limiter.last_request = now;
                }
            } else {
                rate_limiter.request_count += 1;
                rate_limiter.last_request = now;
            }
        }
    }

    pub async fn start_retry_processor(&self) {
        let retry_queue = Arc::clone(&self.retry_queue);
        let pending_orders = Arc::clone(&self.pending_orders);
        let order_events_tx = self.order_events_tx.clone();
        let config = self.config.clone();
        let auth = self.auth.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            
            loop {
                interval.tick().await;
                
                let now = std::time::Instant::now();
                let retry_requests = {
                    let mut queue = retry_queue.write().await;
                    let mut to_retry = Vec::new();
                    let mut remaining = Vec::new();
                    
                    for request in queue.drain(..) {
                        if now >= request.retry_after {
                            to_retry.push(request);
                        } else {
                            remaining.push(request);
                        }
                    }
                    
                    *queue = remaining;
                    to_retry
                };

                for retry_request in retry_requests {
                    if retry_request.order.retry_count >= config.max_retries {
                        warn!("Max retries exceeded for order: {}", retry_request.order.internal_id);
                        pending_orders.remove(&retry_request.order.client_order_id);
                        let _ = order_events_tx.send(ApiEvent::Error {
                            error: format!("Max retries exceeded for order {}", retry_request.order.internal_id),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        });
                        continue;
                    }

                    let mut updated_order = retry_request.order.clone();
                    updated_order.retry_count += 1;

                    match Self::submit_order_with_auth(&auth, &config, &updated_order).await {
                        Ok(_) => {
                            info!("Order retry successful: {}", updated_order.internal_id);
                            pending_orders.insert(updated_order.client_order_id, updated_order);
                        }
                        Err(e) => {
                            warn!("Order retry failed: {} - {}", updated_order.internal_id, e);
                            let retry_after = now + Duration::from_millis(config.retry_delay_ms);
                            let new_retry_request = RetryRequest {
                                order: updated_order,
                                retry_after,
                            };
                            retry_queue.write().await.push(new_retry_request);
                        }
                    }
                }
            }
        });
    }

    async fn submit_order_with_auth(
        auth: &HyperLiquidAuth,
        config: &ApiConfig,
        pending_order: &PendingOrder,
    ) -> Result<(), ApiError> {
        let hl_order = HyperLiquidOrder {
            a: auth.account_id,
            b: matches!(pending_order.side, Side::Buy),
            p: pending_order.price.to_string(),
            s: pending_order.size.to_string(),
            r: false,
            t: match pending_order.order_type {
                OrderType::Market => "Market".to_string(),
                OrderType::Limit => "Limit".to_string(),
                OrderType::PostOnly => "Limit".to_string(),
            },
            cid: pending_order.client_order_id,
            oid: None,
        };

        let signed_request = auth.create_signed_request("order", &hl_order)?;
        let headers = auth.get_headers()?;

        let response = auth.client
            .post(&format!("{}/exchange", config.base_url))
            .headers(headers)
            .json(&signed_request)
            .send()
            .await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ApiError::OrderRejected(format!(
                "Order failed with status {}: {}", 
                status, 
                error_text
            )));
        }

        let order_response: HyperLiquidOrderResponse = response
            .json()
            .await
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        if order_response.status != "ok" {
            return Err(ApiError::OrderRejected(
                "Order response status not ok".to_string()
            ));
        }

        Ok(())
    }

    pub fn get_pending_orders(&self) -> Vec<PendingOrder> {
        self.pending_orders
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn get_pending_order(&self, internal_id: Uuid) -> Option<PendingOrder> {
        self.pending_orders
            .iter()
            .find(|entry| entry.value().internal_id == internal_id)
            .map(|entry| entry.value().clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidCancelRequest {
    pub oid: u64,
}

// Clone implementation removed to avoid conflicts
