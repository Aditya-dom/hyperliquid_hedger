use crate::api::types::*;
use crate::api::auth::HyperLiquidAuth;
use anyhow::Result;
use crossbeam_channel::{Sender, Receiver, unbounded};
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{error, info, warn, debug};
use yawc::{frame::FrameView, Options, WebSocket};
use futures::{StreamExt, SinkExt};

pub struct TradingWebSocket {
    pub auth: HyperLiquidAuth,
    pub config: ApiConfig,
    pub ws: Option<WebSocket>,
    pub trading_events_tx: Sender<ApiEvent>,
    pub connection_state: Arc<RwLock<ConnectionState>>,
    pub subscription_state: Arc<RwLock<SubscriptionState>>,
    pub reconnect_attempts: Arc<RwLock<u32>>,
    pub last_heartbeat: Arc<RwLock<std::time::Instant>>,
}

#[derive(Debug, Clone)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct SubscriptionState {
    pub user_events: bool,
    pub fills: bool,
    pub orders: bool,
    pub positions: bool,
}

impl Default for SubscriptionState {
    fn default() -> Self {
        Self {
            user_events: false,
            fills: false,
            orders: false,
            positions: false,
        }
    }
}

impl TradingWebSocket {
    pub fn new(auth: HyperLiquidAuth, config: ApiConfig) -> (Self, Receiver<ApiEvent>) {
        let (tx, rx) = unbounded();
        
        let ws = Self {
            auth,
            config,
            ws: None,
            trading_events_tx: tx,
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            subscription_state: Arc::new(RwLock::new(SubscriptionState::default())),
            reconnect_attempts: Arc::new(RwLock::new(0)),
            last_heartbeat: Arc::new(RwLock::new(std::time::Instant::now())),
        };
        
        (ws, rx)
    }

    pub async fn connect(&mut self) -> Result<(), ApiError> {
        info!("Connecting to HyperLiquid trading WebSocket");
        
        {
            let mut state = self.connection_state.write();
            *state = ConnectionState::Connecting;
        }

        let ws_url = &self.config.ws_url;
        let client = WebSocket::connect_with_options(
            ws_url.parse::<url::Url>().map_err(|e| ApiError::NetworkError(e.to_string()))?,
            None,
            Options::default(),
        ).await.map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.ws = Some(client);
        
        {
            let mut state = self.connection_state.write();
            *state = ConnectionState::Connected;
        }

        {
            let mut attempts = self.reconnect_attempts.write();
            *attempts = 0;
        }

        {
            let mut heartbeat = self.last_heartbeat.write();
            *heartbeat = std::time::Instant::now();
        }

        info!("Connected to HyperLiquid trading WebSocket");
        Ok(())
    }

    pub async fn subscribe_to_user_events(&mut self) -> Result<(), ApiError> {
        if self.ws.is_none() {
            return Err(ApiError::NetworkError("WebSocket not connected".to_string()));
        }

        let subscribe_msg = serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "userEvents",
                "user": self.auth.account_id.map(|id| id.to_string())
            }
        });

        let ws = self.ws.as_mut().unwrap();
        let message = serde_json::to_string(&subscribe_msg)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        ws.send(FrameView::text(message)).await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        {
            let mut sub_state = self.subscription_state.write();
            sub_state.user_events = true;
        }

        info!("Subscribed to user events");
        Ok(())
    }

    pub async fn subscribe_to_fills(&mut self) -> Result<(), ApiError> {
        if self.ws.is_none() {
            return Err(ApiError::NetworkError("WebSocket not connected".to_string()));
        }

        let subscribe_msg = serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "fills",
                "user": self.auth.account_id.map(|id| id.to_string())
            }
        });

        let ws = self.ws.as_mut().unwrap();
        let message = serde_json::to_string(&subscribe_msg)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        ws.send(FrameView::text(message)).await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        {
            let mut sub_state = self.subscription_state.write();
            sub_state.fills = true;
        }

        info!("Subscribed to fills");
        Ok(())
    }

    pub async fn subscribe_to_orders(&mut self) -> Result<(), ApiError> {
        if self.ws.is_none() {
            return Err(ApiError::NetworkError("WebSocket not connected".to_string()));
        }

        let subscribe_msg = serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "orders",
                "user": self.auth.account_id.map(|id| id.to_string())
            }
        });

        let ws = self.ws.as_mut().unwrap();
        let message = serde_json::to_string(&subscribe_msg)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        ws.send(FrameView::text(message)).await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        {
            let mut sub_state = self.subscription_state.write();
            sub_state.orders = true;
        }

        info!("Subscribed to orders");
        Ok(())
    }

    pub async fn subscribe_to_positions(&mut self) -> Result<(), ApiError> {
        if self.ws.is_none() {
            return Err(ApiError::NetworkError("WebSocket not connected".to_string()));
        }

        let subscribe_msg = serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "positions",
                "user": self.auth.account_id.map(|id| id.to_string())
            }
        });

        let ws = self.ws.as_mut().unwrap();
        let message = serde_json::to_string(&subscribe_msg)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        ws.send(FrameView::text(message)).await
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        {
            let mut sub_state = self.subscription_state.write();
            sub_state.positions = true;
        }

        info!("Subscribed to positions");
        Ok(())
    }

    pub async fn subscribe_to_all(&mut self) -> Result<(), ApiError> {
        self.subscribe_to_user_events().await?;
        self.subscribe_to_fills().await?;
        self.subscribe_to_orders().await?;
        self.subscribe_to_positions().await?;
        Ok(())
    }

    pub async fn run(&mut self) -> Result<(), ApiError> {
        if self.ws.is_none() {
            return Err(ApiError::NetworkError("WebSocket not connected".to_string()));
        }

        let trading_events_tx = self.trading_events_tx.clone();
        let connection_state = Arc::clone(&self.connection_state);
        let last_heartbeat = Arc::clone(&self.last_heartbeat);

        // Start heartbeat monitor
        let heartbeat_monitor = {
            let connection_state = Arc::clone(&connection_state);
            let last_heartbeat = Arc::clone(&last_heartbeat);
            let trading_events_tx = trading_events_tx.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                
                loop {
                    interval.tick().await;
                    
                    let heartbeat_age = last_heartbeat.read().elapsed();
                    if heartbeat_age > std::time::Duration::from_secs(60) {
                        warn!("WebSocket heartbeat timeout");
                        {
                            let mut state = connection_state.write();
                            *state = ConnectionState::Error("Heartbeat timeout".to_string());
                        }
                        
                        let _ = trading_events_tx.send(ApiEvent::Error {
                            error: "WebSocket heartbeat timeout".to_string(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        });
                    }
                }
            })
        };

        // Message processing loop
        while let Some(frame) = self.ws.as_mut().unwrap().next().await {
            if let Err(e) = self.handle_message(frame).await {
                error!("Error handling WebSocket message: {}", e);
            }
        }

        heartbeat_monitor.abort();
        Ok(())
    }

    async fn handle_message(&mut self, frame: FrameView) -> Result<(), ApiError> {
        match frame.opcode {
            yawc::frame::OpCode::Text => {
                if let Ok(text) = std::str::from_utf8(&frame.payload) {
                    debug!("Received WebSocket message: {}", text);
                    
                    if let Ok(message) = serde_json::from_str::<serde_json::Value>(text) {
                        self.process_trading_message(message).await?;
                    }
                }
            }
            yawc::frame::OpCode::Ping => {
                // Respond to ping with pong
                if let Some(ws) = &mut self.ws {
                    ws.send(FrameView::pong(frame.payload.to_vec())).await
                        .map_err(|e| ApiError::NetworkError(e.to_string()))?;
                }
            }
            yawc::frame::OpCode::Pong => {
                // Update heartbeat
                {
                    let mut heartbeat = self.last_heartbeat.write();
                    *heartbeat = std::time::Instant::now();
                }
            }
            yawc::frame::OpCode::Close => {
                warn!("WebSocket connection closed by server");
                {
                    let mut state = self.connection_state.write();
                    *state = ConnectionState::Disconnected;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn process_trading_message(&self, message: serde_json::Value) -> Result<(), ApiError> {
        if let Some(channel) = message.get("channel").and_then(|c| c.as_str()) {
            match channel {
                "userEvents" => {
                    if let Some(data) = message.get("data") {
                        self.process_user_event(data).await?;
                    }
                }
                "fills" => {
                    if let Some(data) = message.get("data") {
                        self.process_fill(data).await?;
                    }
                }
                "orders" => {
                    if let Some(data) = message.get("data") {
                        self.process_order_update(data).await?;
                    }
                }
                "positions" => {
                    if let Some(data) = message.get("data") {
                        self.process_position_update(data).await?;
                    }
                }
                "pong" => {
                    // Update heartbeat
                    {
                        let mut heartbeat = self.last_heartbeat.write();
                        *heartbeat = std::time::Instant::now();
                    }
                }
                _ => {
                    debug!("Unknown channel: {}", channel);
                }
            }
        }
        Ok(())
    }

    async fn process_user_event(&self, data: &serde_json::Value) -> Result<(), ApiError> {
        // Process user events like account updates
        debug!("Processing user event: {:?}", data);
        Ok(())
    }

    async fn process_fill(&self, data: &serde_json::Value) -> Result<(), ApiError> {
        if let Ok(fill) = serde_json::from_value::<HyperLiquidFill>(data.clone()) {
            let event = ApiEvent::Fill {
                order_id: fill.oid,
                fill_size: fill.sz.clone(),
                fill_price: fill.px.clone(),
                fee: fill.fee.clone(),
                timestamp: fill.time,
            };

            let _ = self.trading_events_tx.send(event);
            info!("Processed fill for order {}: {} {} at {}", 
                  fill.oid, fill.sz, fill.coin, fill.px);
        }
        Ok(())
    }

    async fn process_order_update(&self, data: &serde_json::Value) -> Result<(), ApiError> {
        if let Ok(order_status) = serde_json::from_value::<HyperLiquidOrderStatus>(data.clone()) {
            if let Some(rest) = order_status.rest {
                let event = ApiEvent::OrderUpdate {
                    order_id: rest.oid,
                    status: "rest".to_string(),
                    filled_size: "0".to_string(),
                    remaining_size: rest.sz.clone(),
                    price: rest.px.clone(),
                    timestamp: rest.timestamp,
                };

                let _ = self.trading_events_tx.send(event);
                info!("Processed order update for order {}: {} remaining", 
                      rest.oid, rest.sz);
            }

            if let Some(filled) = order_status.filled {
                let event = ApiEvent::OrderUpdate {
                    order_id: filled.oid,
                    status: "filled".to_string(),
                    filled_size: filled.sz.clone(),
                    remaining_size: "0".to_string(),
                    price: filled.px.clone(),
                    timestamp: filled.timestamp,
                };

                let _ = self.trading_events_tx.send(event);
                info!("Processed order fill for order {}: {} filled", 
                      filled.oid, filled.sz);
            }
        }
        Ok(())
    }

    async fn process_position_update(&self, data: &serde_json::Value) -> Result<(), ApiError> {
        if let Ok(position) = serde_json::from_value::<HyperLiquidPosition>(data.clone()) {
            let event = ApiEvent::PositionUpdate {
                coin: position.coin.clone(),
                size: position.szi.clone(),
                entry_price: position.entry_px.clone(),
                unrealized_pnl: position.unrealized_pnl.clone(),
            };

            let _ = self.trading_events_tx.send(event);
            info!("Processed position update for {}: {} at {}", 
                  position.coin, position.szi, position.entry_px);
        }
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), ApiError> {
        if let Some(ws) = &mut self.ws {
            ws.close().await.map_err(|e| ApiError::NetworkError(e.to_string()))?;
        }
        
        {
            let mut state = self.connection_state.write();
            *state = ConnectionState::Disconnected;
        }

        info!("Disconnected from HyperLiquid trading WebSocket");
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        matches!(*self.connection_state.read(), ConnectionState::Connected)
    }

    pub fn get_connection_state(&self) -> ConnectionState {
        self.connection_state.read().clone()
    }

    pub fn get_subscription_state(&self) -> SubscriptionState {
        self.subscription_state.read().clone()
    }

    pub async fn start_reconnect_loop(&mut self) {
        let connection_state = Arc::clone(&self.connection_state);
        let reconnect_attempts = Arc::clone(&self.reconnect_attempts);
        let trading_events_tx = self.trading_events_tx.clone();
        let auth = self.auth.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let should_reconnect = {
                    let state = connection_state.read();
                    matches!(*state, ConnectionState::Disconnected | ConnectionState::Error(_))
                };

                if should_reconnect {
                    let attempts = {
                        let mut attempts = reconnect_attempts.write();
                        *attempts += 1;
                        *attempts
                    };

                    if attempts <= 10 { // Max 10 reconnection attempts
                        info!("Attempting to reconnect to trading WebSocket (attempt {})", attempts);
                        
                        {
                            let mut state = connection_state.write();
                            *state = ConnectionState::Reconnecting;
                        }

                        // Create new WebSocket connection
                        let ws_result = WebSocket::connect_with_options(
                            config.ws_url.parse().unwrap(),
                            None,
                            Options::default(),
                        ).await;

                        match ws_result {
                            Ok(ws) => {
                                // Reset connection state and attempts
                                {
                                    let mut state = connection_state.write();
                                    *state = ConnectionState::Connected;
                                }
                                {
                                    let mut attempts = reconnect_attempts.write();
                                    *attempts = 0;
                                }

                                info!("Successfully reconnected to trading WebSocket");
                                
                                // Note: In a real implementation, you'd need to resubscribe to all channels
                                // and handle the WebSocket in the main loop
                            }
                            Err(e) => {
                                error!("Failed to reconnect to trading WebSocket: {}", e);
                                {
                                    let mut state = connection_state.write();
                                    *state = ConnectionState::Error(e.to_string());
                                }
                            }
                        }
                    } else {
                        error!("Max reconnection attempts reached for trading WebSocket");
                        break;
                    }
                }
            }
        });
    }
}

// Clone implementation removed to avoid conflicts
