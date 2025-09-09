use crate::trading::types::*;
use crate::trading::order_book::OrderBook;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

#[async_trait]
pub trait TradingStrategy: Send + Sync {
    async fn on_market_data(&mut self, order_book: &OrderBook) -> Vec<OrderAction>;
    async fn on_order_update(&mut self, order: &Order) -> Vec<OrderAction>;
    async fn on_position_update(&mut self, position: &Position) -> Vec<OrderAction>;
    async fn on_fill(&mut self, fill: &Fill) -> Vec<OrderAction>;
    
    fn get_name(&self) -> &str;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub name: String,
    pub enabled: bool,
    pub symbol: String,
    pub risk_limits: RiskLimits,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            name: "base_strategy".to_string(),
            enabled: false,
            symbol: "HYPE".to_string(),
            risk_limits: RiskLimits::default(),
        }
    }
}
