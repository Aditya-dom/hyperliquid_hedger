use hyper_liquid_connector::{
    api::{auth::HyperLiquidAuth, trading_api::TradingApi, account_api::AccountApi, ws_trading::TradingWebSocket},
    config::bot_config::ConfigManager,
    trading::{order_manager::OrderManager, position_manager::PositionManager, risk_manager::RiskManager, order_book::OrderBook},
    strategies::{market_making::MarketMakingStrategy, base_strategy::TradingStrategy},
    events::event_bus::EventBus,
    clients::ws_manager::WsManager,
};
use anyhow::Result;
use crossbeam_channel::{Receiver, unbounded};
use dashmap::DashMap;
use tokio::sync::RwLock;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn, debug};
use uuid::Uuid;

pub struct TradingBot {
    pub config_manager: ConfigManager,
    pub auth: HyperLiquidAuth,
    pub trading_api: TradingApi,
    pub account_api: AccountApi,
    pub trading_ws: TradingWebSocket,
    pub order_manager: OrderManager,
    pub position_manager: PositionManager,
    pub risk_manager: RiskManager,
    pub market_making_strategy: MarketMakingStrategy,
    pub event_bus: EventBus,
    pub ws_manager: WsManager,
    pub order_books: Arc<DashMap<String, OrderBook>>,
    pub is_running: Arc<RwLock<bool>>,
    pub bot_events_tx: crossbeam_channel::Sender<BotEvent>,
}

#[derive(Debug, Clone)]
pub enum BotEvent {
    Started,
    Stopped,
    StrategyEnabled { name: String },
    StrategyDisabled { name: String },
    OrderPlaced { order_id: Uuid, symbol: String },
    OrderFilled { order_id: Uuid, symbol: String, size: Decimal, price: Decimal },
    PositionUpdated { symbol: String, size: Decimal, pnl: Decimal },
    RiskAlert { message: String, severity: String },
    Error { error: String },
}

impl TradingBot {
    pub async fn new(config_path: Option<String>) -> Result<(Self, Receiver<BotEvent>)> {
        let (bot_events_tx, bot_events_rx) = unbounded();

        // Initialize configuration manager
        let (config_manager, _config_events_rx) = ConfigManager::new();
        if let Some(path) = config_path {
            config_manager.load_from_file(path).await
                .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;
        }

        let config = config_manager.get_config();

        // Initialize authentication
        let private_key = std::env::var("HYPERLIQUID_PRIVATE_KEY")
            .map_err(|_| anyhow::anyhow!("HYPERLIQUID_PRIVATE_KEY environment variable not set"))?;
        let auth = HyperLiquidAuth::new(private_key);
        
        // Authenticate with HyperLiquid
        let mut auth = auth;
        auth.authenticate().await
            .map_err(|e| anyhow::anyhow!("Authentication failed: {}", e))?;

        // Initialize API clients
        let (trading_api, _trading_events_rx) = TradingApi::new(auth.clone(), config.api_config.clone());
        let (account_api, _account_events_rx) = AccountApi::new(auth.clone(), config.api_config.clone());
        let (trading_ws, _trading_ws_events_rx) = TradingWebSocket::new(auth.clone(), config.api_config.clone());

        // Initialize managers
        let (order_manager, _order_events_rx) = OrderManager::new();
        let (position_manager, _position_events_rx) = PositionManager::new();
        let (risk_manager, _risk_events_rx) = RiskManager::new();

        // Initialize market making strategy
        let strategy_config = config.strategies.get("market_making_HYPE")
            .ok_or_else(|| anyhow::anyhow!("Market making strategy not found in config"))?;
        
        let market_making_config = serde_json::from_value(strategy_config.config.clone())
            .map_err(|e| anyhow::anyhow!("Failed to parse market making config: {}", e))?;
        
        let market_making_strategy = MarketMakingStrategy::new(market_making_config);

        // Initialize event bus
        let event_bus = EventBus::new(Default::default());
        event_bus.start_processing();

        // Initialize WebSocket manager for market data
        let (msg_tx, msg_rx) = mpsc::channel(1000);
        let ws_manager = WsManager::new(
            3, // 3 redundant connections
            &config.api_config.ws_url,
            "HYPE",
            msg_tx,
            msg_rx,
        ).await?;

        let bot = Self {
            config_manager,
            auth,
            trading_api,
            account_api,
            trading_ws,
            order_manager,
            position_manager,
            risk_manager,
            market_making_strategy,
            event_bus,
            ws_manager,
            order_books: Arc::new(DashMap::new()),
            is_running: Arc::new(RwLock::new(false)),
            bot_events_tx,
        };

        Ok((bot, bot_events_rx))
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting trading bot");

        {
            let mut is_running = self.is_running.write().await;
            *is_running = true;
        }

        // Start risk manager daily reset timer
        self.risk_manager.start_daily_reset_timer();

        // Start trading API retry processor
        self.trading_api.start_retry_processor().await;

        // Start account API periodic updates
        self.account_api.start_periodic_updates(30).await; // Update every 30 seconds

        // Connect to trading WebSocket
        self.trading_ws.connect().await
            .map_err(|e| anyhow::anyhow!("Failed to connect trading WebSocket: {}", e))?;

        // Subscribe to all trading events
        self.trading_ws.subscribe_to_all().await
            .map_err(|e| anyhow::anyhow!("Failed to subscribe to trading events: {}", e))?;

        // Start trading WebSocket reconnect loop
        self.trading_ws.start_reconnect_loop().await;

        // Start main event processing loop
        self.start_event_processing().await;

        let _ = self.bot_events_tx.send(BotEvent::Started);
        info!("Trading bot started successfully");

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping trading bot");

        {
            let mut is_running = self.is_running.write().await;
            *is_running = false;
        }

        // Cancel all open orders
        self.trading_api.cancel_all_orders(None).await
            .map_err(|e| anyhow::anyhow!("Failed to cancel all orders: {}", e))?;

        // Disconnect from WebSockets
        self.trading_ws.disconnect().await
            .map_err(|e| anyhow::anyhow!("Failed to disconnect trading WebSocket: {}", e))?;

        self.ws_manager.stop().await
            .map_err(|e| anyhow::anyhow!("Failed to stop WebSocket manager: {}", e))?;

        let _ = self.bot_events_tx.send(BotEvent::Stopped);
        info!("Trading bot stopped successfully");

        Ok(())
    }

    async fn start_event_processing(&self) {
        let is_running = Arc::clone(&self.is_running);
        let order_books = Arc::clone(&self.order_books);
        let market_making_strategy = Arc::new(RwLock::new(self.market_making_strategy.clone()));
        let trading_api = self.trading_api.clone();
        let risk_manager = self.risk_manager.clone();
        let bot_events_tx = self.bot_events_tx.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));

            while *is_running.read().await {
                interval.tick().await;

                // Process market data and generate orders
                for entry in order_books.iter() {
                    let (symbol, order_book) = (entry.key(), entry.value());
                    
                    // Clone order book to avoid holding references across await
                    let order_book_clone = order_book.clone();
                    
                    // Extract actions without holding lock across await
                    let actions = {
                        let strategy = market_making_strategy.read().await;
                        // Generate actions synchronously to avoid Send issues
                        strategy.generate_actions_sync(&order_book_clone)
                    };
                    
                    for action in actions {
                        match action.action_type {
                            hyper_liquid_connector::trading::types::OrderActionType::Place => {
                                if let Some(new_order) = action.order {
                                    match risk_manager.check_order_risk(&new_order) {
                                        Ok(_) => {
                                            match trading_api.place_order(new_order.clone()).await {
                                                Ok(order_id) => {
                                                    info!("Order placed: {} for {}", order_id, symbol);
                                                    let _ = bot_events_tx.send(BotEvent::OrderPlaced {
                                                        order_id,
                                                        symbol: symbol.clone(),
                                                    });
                                                }
                                                Err(e) => {
                                                    error!("Failed to place order: {}", e);
                                                    let _ = bot_events_tx.send(BotEvent::Error {
                                                        error: format!("Failed to place order: {}", e),
                                                    });
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Order rejected by risk manager: {}", e);
                                            let _ = bot_events_tx.send(BotEvent::RiskAlert {
                                                message: format!("Order rejected: {}", e),
                                                severity: "high".to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                            hyper_liquid_connector::trading::types::OrderActionType::Cancel => {
                                if let Some(order_id) = action.order_id {
                                    if let Err(e) = trading_api.cancel_order(order_id).await {
                                        error!("Failed to cancel order {}: {}", order_id, e);
                                    }
                                }
                            }
                            hyper_liquid_connector::trading::types::OrderActionType::Modify => {
                                warn!("Order modification not implemented yet");
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn get_positions(&self) -> Vec<hyper_liquid_connector::trading::types::Position> {
        self.position_manager.get_all_positions()
    }

    pub fn get_active_orders(&self) -> Vec<hyper_liquid_connector::trading::types::Order> {
        self.order_manager.get_active_orders(None)
    }

    pub fn get_total_pnl(&self) -> Decimal {
        self.position_manager.get_total_pnl()
    }

    pub fn get_risk_score(&self, symbol: &str) -> Decimal {
        self.risk_manager.get_risk_score(symbol)
    }

    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    pub async fn enable_strategy(&mut self, name: &str) -> Result<()> {
        self.config_manager.update_config(|config| {
            if let Some(strategy) = config.strategies.get_mut(name) {
                strategy.enabled = true;
            }
        }).map_err(|e| anyhow::anyhow!("{}", e))?;

        // Update strategy state
        if name == "market_making_HYPE" {
            self.market_making_strategy.set_enabled(true);
        }

        let _ = self.bot_events_tx.send(BotEvent::StrategyEnabled {
            name: name.to_string(),
        });

        info!("Strategy enabled: {}", name);
        Ok(())
    }

    pub async fn disable_strategy(&mut self, name: &str) -> Result<()> {
        self.config_manager.update_config(|config| {
            if let Some(strategy) = config.strategies.get_mut(name) {
                strategy.enabled = false;
            }
        }).map_err(|e| anyhow::anyhow!("{}", e))?;

        // Update strategy state
        if name == "market_making_HYPE" {
            self.market_making_strategy.set_enabled(false);
        }

        let _ = self.bot_events_tx.send(BotEvent::StrategyDisabled {
            name: name.to_string(),
        });

        info!("Strategy disabled: {}", name);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create trading bot
    let (mut bot, bot_events_rx) = TradingBot::new(Some("config/bot.toml".to_string())).await?;

    // Start bot
    bot.start().await?;

    // Handle bot events
    tokio::spawn(async move {
        while let Ok(event) = bot_events_rx.recv() {
            match event {
                BotEvent::Started => info!("Bot started"),
                BotEvent::Stopped => info!("Bot stopped"),
                BotEvent::StrategyEnabled { name } => info!("Strategy enabled: {}", name),
                BotEvent::StrategyDisabled { name } => info!("Strategy disabled: {}", name),
                BotEvent::OrderPlaced { order_id, symbol } => {
                    info!("Order placed: {} for {}", order_id, symbol);
                }
                BotEvent::OrderFilled { order_id, symbol, size, price } => {
                    info!("Order filled: {} for {} - {} at {}", order_id, symbol, size, price);
                }
                BotEvent::PositionUpdated { symbol, size, pnl } => {
                    info!("Position updated: {} - {} (PnL: {})", symbol, size, pnl);
                }
                BotEvent::RiskAlert { message, severity } => {
                    warn!("Risk alert [{}]: {}", severity, message);
                }
                BotEvent::Error { error } => {
                    error!("Bot error: {}", error);
                }
            }
        }
    });

    // Handle shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received");
            bot.stop().await?;
        }
    }

    Ok(())
}
