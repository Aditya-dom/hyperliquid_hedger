use crate::trading::types::*;
use crate::trading::order_book::OrderBook;
use crate::trading::order_manager::{OrderManager, OrderEvent};
use crate::trading::position_manager::{PositionManager, PositionEvent};
use crate::strategies::market_making::{MarketMakingStrategy, MarketMakingConfig};
use crate::strategies::base_strategy::TradingStrategy;
use crate::events::event_bus::{EventBus, EventBusConfig, EventPublisher};
use crate::events::types::*;
use crate::ui::panels::*;
use egui::{CentralPanel, SidePanel, TopBottomPanel, Context, Ui};
use std::collections::VecDeque;
use crossbeam_channel::Receiver;
use rust_decimal::Decimal;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

pub struct TradingApp {
    // Core trading components
    pub order_book: Arc<RwLock<OrderBook>>,
    pub order_manager: OrderManager,
    pub position_manager: PositionManager,
    pub market_making_strategy: Arc<RwLock<MarketMakingStrategy>>,
    
    // Event system
    pub event_bus: Arc<EventBus>,
    pub event_publisher: EventPublisher,
    
    // Event receivers
    pub order_events_rx: Option<Receiver<OrderEvent>>,
    pub position_events_rx: Option<Receiver<PositionEvent>>,
    pub system_events_rx: Option<Receiver<SystemEvent>>,
    
    // UI state
    pub connection_status: ConnectionStatus,
    pub logs: Arc<RwLock<VecDeque<LogEntry>>>,
    pub selected_symbol: String,
    pub manual_order: ManualOrderState,
    
    // UI panels
    pub show_order_book: bool,
    pub show_positions: bool,
    pub show_strategy: bool,
    pub show_logs: bool,
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ManualOrderState {
    pub side: Side,
    pub order_type: OrderType,
    pub price: String,
    pub size: String,
    pub symbol: String,
}

impl Default for ManualOrderState {
    fn default() -> Self {
        Self {
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: "0.0".to_string(),
            size: "0.0".to_string(),
            symbol: "HYPE".to_string(),
        }
    }
}

impl TradingApp {
    pub fn new() -> Self {
        // Create event bus
        let event_bus_config = EventBusConfig::default();
        let event_bus = Arc::new(EventBus::new(event_bus_config));
        let event_publisher = event_bus.get_publisher();
        
        // Create trading components with event channels
        let (order_manager, order_events_rx) = OrderManager::new();
        let (position_manager, position_events_rx) = PositionManager::new();
        
        // Create order book and strategy
        let order_book = Arc::new(RwLock::new(OrderBook::new("HYPE".to_string())));
        let mm_config = MarketMakingConfig::default();
        let market_making_strategy = Arc::new(RwLock::new(MarketMakingStrategy::new(mm_config)));
        
        // Subscribe to system events
        let system_events_rx = event_bus.subscribe("*");
        
        // Start event bus processing
        event_bus.start_processing();
        
        Self {
            order_book,
            order_manager,
            position_manager,
            market_making_strategy,
            event_bus,
            event_publisher,
            order_events_rx: Some(order_events_rx),
            position_events_rx: Some(position_events_rx),
            system_events_rx: Some(system_events_rx),
            connection_status: ConnectionStatus::Disconnected,
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            selected_symbol: "HYPE".to_string(),
            manual_order: ManualOrderState::default(),
            show_order_book: true,
            show_positions: true,
            show_strategy: true,
            show_logs: true,
        }
    }

    pub fn add_log(&self, level: LogLevel, message: String) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            message,
        };
        
        let mut logs = self.logs.write();
        logs.push_back(entry);
        
        // Keep only last 1000 logs
        if logs.len() > 1000 {
            logs.pop_front();
        }
    }

    pub fn process_events(&mut self) {
        // Process order events
        if let Some(rx) = &self.order_events_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    OrderEvent::OrderPlaced(order) => {
                        self.add_log(LogLevel::Info, format!("Order placed: {} - {:?} {} @ {}", 
                                     order.id, order.side, order.size, order.price));
                    }
                    OrderEvent::OrderUpdated(order) => {
                        self.add_log(LogLevel::Info, format!("Order updated: {} - {:?}", 
                                     order.id, order.status));
                    }
                    OrderEvent::OrderCancelled(order_id) => {
                        self.add_log(LogLevel::Info, format!("Order cancelled: {}", order_id));
                    }
                    OrderEvent::OrderFilled(order) => {
                        self.add_log(LogLevel::Info, format!("Order filled: {} - {} filled", 
                                     order.id, order.filled_size));
                    }
                }
            }
        }
        
        // Process position events
        if let Some(rx) = &self.position_events_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    PositionEvent::PositionUpdated(position) => {
                        self.add_log(LogLevel::Info, format!("Position updated: {} - size: {}, PnL: {}", 
                                     position.symbol, position.size, position.unrealized_pnl));
                    }
                    PositionEvent::FillProcessed(fill) => {
                        self.add_log(LogLevel::Info, format!("Fill processed: {} {} @ {} (fee: {})", 
                                     fill.symbol, fill.size, fill.price, fill.fee));
                    }
                    PositionEvent::PnlRealized(pnl) => {
                        self.add_log(LogLevel::Info, format!("PnL realized: ${:.2}", pnl));
                    }
                }
            }
        }
        
        // Process system events
        if let Some(rx) = &self.system_events_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    SystemEvent::MarketData { symbol, data, .. } => {
                        // Update order book
                        {
                            let mut order_book = self.order_book.write();
                            order_book.update_from_tob(&data.data);
                        }
                        
                        // Update position mark prices
                        if let Some(mid_price) = self.order_book.read().mid_price() {
                            self.position_manager.update_mark_prices(&symbol, mid_price);
                        }
                        
                        // Process strategy
                        let order_book_read = self.order_book.read();
                        let mut strategy = self.market_making_strategy.write();
                        
                        // This would need to be adapted for async in a real implementation
                        // For now, we'll skip the async strategy processing in the UI thread
                    }
                    SystemEvent::Risk { symbol, event, .. } => {
                        match event {
                            RiskEvent::LimitExceeded { limit_type, current_value, limit_value } => {
                                self.add_log(LogLevel::Error, format!(
                                    "Risk limit exceeded for {}: {} = {} (limit: {})", 
                                    symbol, limit_type, current_value, limit_value
                                ));
                            }
                            RiskEvent::OrderRejected { order_id, reason } => {
                                self.add_log(LogLevel::Warning, format!(
                                    "Order {} rejected: {}", order_id, reason
                                ));
                            }
                            _ => {}
                        }
                    }
                    SystemEvent::Connection { connection_id, event, .. } => {
                        match event {
                            ConnectionEvent::Connected => {
                                self.connection_status = ConnectionStatus::Connected;
                                self.add_log(LogLevel::Info, format!("Connected: {}", connection_id));
                            }
                            ConnectionEvent::Disconnected => {
                                self.connection_status = ConnectionStatus::Disconnected;
                                self.add_log(LogLevel::Warning, format!("Disconnected: {}", connection_id));
                            }
                            ConnectionEvent::Error(err) => {
                                self.connection_status = ConnectionStatus::Error(err.clone());
                                self.add_log(LogLevel::Error, format!("Connection error {}: {}", connection_id, err));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

impl eframe::App for TradingApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Process events (non-async version)
        self.process_events();

        // Request continuous repaint for real-time updates
        ctx.request_repaint();

        // Top menu bar
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("HyperLiquid Market Making Bot");
                
                ui.separator();
                
                // Connection status
                let status_text = match &self.connection_status {
                    ConnectionStatus::Connected => "🟢 Connected",
                    ConnectionStatus::Connecting => "🟡 Connecting",
                    ConnectionStatus::Disconnected => "🔴 Disconnected",
                    ConnectionStatus::Error(err) => &format!("❌ Error: {}", err),
                };
                ui.label(status_text);
                
                ui.separator();
                
                // Event bus metrics
                let metrics = self.event_bus.get_metrics();
                ui.label(format!("Events: {} processed, {} dropped", 
                               metrics.events_processed, metrics.events_dropped));
                ui.label(format!("Queues: H:{} N:{} L:{}", 
                               metrics.high_priority_queue_len,
                               metrics.normal_priority_queue_len,
                               metrics.low_priority_queue_len));
                
                ui.separator();
                
                // Panel toggles
                ui.checkbox(&mut self.show_order_book, "Order Book");
                ui.checkbox(&mut self.show_positions, "Positions");
                ui.checkbox(&mut self.show_strategy, "Strategy");
                ui.checkbox(&mut self.show_logs, "Logs");
            });
        });

        // Left panel - Order Book and Trading
        if self.show_order_book {
            SidePanel::left("left_panel").resizable(true).show(ctx, |ui| {
                ui.heading("Order Book");
                let order_book = self.order_book.read();
                order_book_panel::show(ui, &*order_book);
                
                ui.separator();
                
                ui.heading("Manual Trading");
                trading_panel::show(ui, &mut self.manual_order, &self.order_manager);
            });
        }

        // Right panel - Strategy and Positions
        if self.show_strategy || self.show_positions {
            SidePanel::right("right_panel").resizable(true).show(ctx, |ui| {
                if self.show_strategy {
                    ui.heading("Market Making Strategy");
                    let mut strategy = self.market_making_strategy.write();
                    strategy_panel::show(ui, &mut *strategy);
                    ui.separator();
                }
                
                if self.show_positions {
                    ui.heading("Positions & PnL");
                    positions_panel::show(ui, &self.position_manager);
                }
            });
        }

        // Bottom panel - Logs
        if self.show_logs {
            TopBottomPanel::bottom("bottom_panel").resizable(true).show(ctx, |ui| {
                ui.heading("Logs");
                let logs = self.logs.read();
                logs_panel::show(ui, &*logs);
            });
        }

        // Central panel - Market data and charts
        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Market Overview");
            
            // Market stats
            ui.horizontal(|ui| {
                let order_book = self.order_book.read();
                if let Some(mid_price) = order_book.mid_price() {
                    ui.label(format!("Mid Price: ${:.4}", mid_price));
                }
                if let Some(spread) = order_book.spread() {
                    ui.label(format!("Spread: ${:.4}", spread));
                }
                if let Some(spread_bps) = order_book.spread_bps() {
                    ui.label(format!("Spread: {:.1} bps", spread_bps));
                }
            });
            
            ui.separator();
            
            // PnL Summary
            ui.horizontal(|ui| {
                let total_pnl = self.position_manager.get_total_pnl();
                let unrealized_pnl = self.position_manager.get_total_unrealized_pnl();
                let realized_pnl = self.position_manager.get_realized_pnl();
                
                ui.label(format!("Total PnL: ${:.2}", total_pnl));
                ui.label(format!("Unrealized: ${:.2}", unrealized_pnl));
                ui.label(format!("Realized: ${:.2}", realized_pnl));
                ui.label(format!("Fees: ${:.2}", self.position_manager.get_total_fees()));
            });
            
            ui.separator();
            
            // Active orders summary
            let active_orders = self.order_manager.get_active_orders(Some(&self.selected_symbol));
            let (buy_count, sell_count) = self.order_manager.get_order_count(&self.selected_symbol);
            let (buy_exposure, sell_exposure) = self.order_manager.get_total_exposure(&self.selected_symbol);
            
            ui.horizontal(|ui| {
                ui.label(format!("Active Orders: {}", active_orders.len()));
                ui.label(format!("Buy: {} (${:.2})", buy_count, buy_exposure));
                ui.label(format!("Sell: {} (${:.2})", sell_count, sell_exposure));
            });
            
            ui.separator();
            
            // Strategy status
            ui.horizontal(|ui| {
                let strategy = self.market_making_strategy.read();
                let strategy_status = if strategy.is_enabled() {
                    "Running"
                } else {
                    "Stopped"
                };
                ui.label(format!("Strategy: {}", strategy_status));
                ui.label(format!("Active MM Orders: {}", strategy.active_orders.len()));
                if let Some(last_price) = strategy.last_price {
                    ui.label(format!("Last Price: ${:.4}", last_price));
                }
                ui.label(format!("Inventory: {:.4}", strategy.current_inventory));
            });
        });
    }
}
