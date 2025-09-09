use crate::strategies::base_strategy::{TradingStrategy, StrategyConfig};
use crate::trading::types::*;
use crate::trading::order_book::OrderBook;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakingConfig {
    pub base_config: StrategyConfig,
    pub spread_bps: u32,              // Spread in basis points
    pub order_size: Decimal,          // Size per order
    pub max_orders_per_side: usize,   // Maximum orders per side
    pub inventory_target: Decimal,    // Target inventory (0 = neutral)
    pub inventory_skew_factor: Decimal, // How much to skew based on inventory
    pub min_edge_bps: u32,            // Minimum edge required
    pub order_refresh_interval_ms: u64, // How often to refresh orders
}

impl Default for MarketMakingConfig {
    fn default() -> Self {
        Self {
            base_config: StrategyConfig::default(),
            spread_bps: 20,               // 20 bps spread
            order_size: dec!(1.0),        // 1 unit per order
            max_orders_per_side: 3,       // Max 3 orders per side
            inventory_target: dec!(0.0),  // Neutral inventory
            inventory_skew_factor: dec!(0.1), // 10% skew per unit
            min_edge_bps: 5,              // 5 bps minimum edge
            order_refresh_interval_ms: 1000, // 1 second refresh
        }
    }
}

#[derive(Debug, Clone)]
pub struct MarketMakingStrategy {
    pub config: MarketMakingConfig,
    pub active_orders: HashMap<Uuid, Order>,
    pub last_order_time: DateTime<Utc>,
    pub last_price: Option<Decimal>,
    pub current_inventory: Decimal,
    pub enabled: bool,
}

impl MarketMakingStrategy {
    pub fn new(config: MarketMakingConfig) -> Self {
        Self {
            config,
            active_orders: HashMap::new(),
            last_order_time: Utc::now() - Duration::hours(1),
            last_price: None,
            current_inventory: dec!(0.0),
            enabled: true,
        }
    }

    fn should_refresh_orders(&self, current_price: Decimal) -> bool {
        let time_elapsed = Utc::now().signed_duration_since(self.last_order_time);
        let time_threshold = Duration::milliseconds(self.config.order_refresh_interval_ms as i64);
        
        // Refresh if enough time has passed
        if time_elapsed > time_threshold {
            return true;
        }
        
        // Refresh if price moved significantly
        if let Some(last_price) = self.last_price {
            let price_change = (current_price - last_price).abs() / last_price;
            if price_change > dec!(0.001) { // 0.1% price change
                return true;
            }
        }
        
        false
    }

    fn calculate_fair_price(&self, order_book: &OrderBook) -> Option<Decimal> {
        order_book.mid_price()
    }

    fn calculate_spread(&self, _order_book: &OrderBook, fair_price: Decimal) -> Decimal {
        let base_spread = fair_price * Decimal::from(self.config.spread_bps) / dec!(10000);
        
        // Add inventory skew
        let inventory_adjustment = self.current_inventory * self.config.inventory_skew_factor;
        
        // Ensure minimum spread
        let min_spread = fair_price * Decimal::from(self.config.min_edge_bps) / dec!(10000);
        
        (base_spread + inventory_adjustment.abs()).max(min_spread)
    }

    fn generate_orders(&self, fair_price: Decimal, spread: Decimal) -> Vec<OrderAction> {
        let mut actions = Vec::new();
        
        // Calculate bid/ask prices with inventory skew
        let inventory_skew = self.current_inventory * self.config.inventory_skew_factor;
        let half_spread = spread / dec!(2.0);
        
        let bid_price = fair_price - half_spread - inventory_skew;
        let ask_price = fair_price + half_spread - inventory_skew;
        
        // Generate buy orders
        for i in 0..self.config.max_orders_per_side {
            let price_offset = Decimal::from(i) * (spread / dec!(4.0)); // Ladder orders
            let order = NewOrder {
                symbol: self.config.base_config.symbol.clone(),
                side: Side::Buy,
                order_type: OrderType::Limit,
                price: bid_price - price_offset,
                size: self.config.order_size,
                client_id: Some(format!("mm_buy_{}", i)),
            };
            
            actions.push(OrderAction {
                action_type: OrderActionType::Place,
                order: Some(order),
                order_id: None,
            });
        }
        
        // Generate sell orders
        for i in 0..self.config.max_orders_per_side {
            let price_offset = Decimal::from(i) * (spread / dec!(4.0)); // Ladder orders
            let order = NewOrder {
                symbol: self.config.base_config.symbol.clone(),
                side: Side::Sell,
                order_type: OrderType::Limit,
                price: ask_price + price_offset,
                size: self.config.order_size,
                client_id: Some(format!("mm_sell_{}", i)),
            };
            
            actions.push(OrderAction {
                action_type: OrderActionType::Place,
                order: Some(order),
                order_id: None,
            });
        }
        
        actions
    }

    pub fn generate_actions_sync(&self, order_book: &OrderBook) -> Vec<OrderAction> {
        if !self.enabled {
            return vec![];
        }

        let Some(fair_price) = self.calculate_fair_price(order_book) else {
            return vec![];
        };

        // Check if we should refresh orders
        if !self.should_refresh_orders(fair_price) {
            return vec![];
        }

        let mut actions = Vec::new();

        // Cancel existing orders first
        if !self.active_orders.is_empty() {
            actions.extend(self.cancel_all_orders());
        }

        // Calculate new spread and generate orders
        let spread = self.calculate_spread(order_book, fair_price);
        actions.extend(self.generate_orders(fair_price, spread));

        actions
    }

    pub fn update_last_price(&mut self, price: Decimal) {
        self.last_price = Some(price);
        self.last_order_time = Utc::now();
    }

    fn cancel_all_orders(&self) -> Vec<OrderAction> {
        self.active_orders
            .keys()
            .map(|&order_id| OrderAction {
                action_type: OrderActionType::Cancel,
                order: None,
                order_id: Some(order_id),
            })
            .collect()
    }
}

#[async_trait]
impl TradingStrategy for MarketMakingStrategy {
    async fn on_market_data(&mut self, order_book: &OrderBook) -> Vec<OrderAction> {
        if !self.enabled {
            return vec![];
        }

        let Some(fair_price) = self.calculate_fair_price(order_book) else {
            return vec![];
        };

        // Check if we should refresh orders
        if !self.should_refresh_orders(fair_price) {
            return vec![];
        }

        let mut actions = Vec::new();

        // Cancel existing orders first
        if !self.active_orders.is_empty() {
            actions.extend(self.cancel_all_orders());
        }

        // Calculate new spread and generate orders
        let spread = self.calculate_spread(order_book, fair_price);
        actions.extend(self.generate_orders(fair_price, spread));

        self.last_price = Some(fair_price);
        self.last_order_time = Utc::now();

        actions
    }

    async fn on_order_update(&mut self, order: &Order) -> Vec<OrderAction> {
        match order.status {
            OrderStatus::Submitted => {
                self.active_orders.insert(order.id, order.clone());
            }
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected => {
                self.active_orders.remove(&order.id);
            }
            OrderStatus::PartiallyFilled => {
                self.active_orders.insert(order.id, order.clone());
            }
            _ => {}
        }
        vec![]
    }

    async fn on_position_update(&mut self, position: &Position) -> Vec<OrderAction> {
        if position.symbol == self.config.base_config.symbol {
            self.current_inventory = position.size;
        }
        vec![]
    }

    async fn on_fill(&mut self, fill: &Fill) -> Vec<OrderAction> {
        // Update inventory based on fill
        match fill.side {
            Side::Buy => self.current_inventory += fill.size,
            Side::Sell => self.current_inventory -= fill.size,
        }
        vec![]
    }

    fn get_name(&self) -> &str {
        &self.config.base_config.name
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            // Cancel all orders when disabled
            self.active_orders.clear();
        }
    }
}
