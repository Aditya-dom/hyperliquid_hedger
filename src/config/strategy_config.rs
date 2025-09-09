use crate::strategies::market_making::MarketMakingConfig;
use crate::trading::types::RiskLimits;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfigTemplate {
    pub name: String,
    pub description: String,
    pub strategy_type: String,
    pub default_config: serde_json::Value,
    pub required_fields: Vec<String>,
    pub optional_fields: Vec<String>,
}

impl StrategyConfigTemplate {
    pub fn market_making() -> Self {
        Self {
            name: "Market Making".to_string(),
            description: "Provides liquidity by placing buy and sell orders around the current market price".to_string(),
            strategy_type: "MarketMaking".to_string(),
            default_config: serde_json::to_value(MarketMakingConfig::default()).unwrap(),
            required_fields: vec![
                "symbol".to_string(),
                "spread_bps".to_string(),
                "order_size".to_string(),
            ],
            optional_fields: vec![
                "max_orders_per_side".to_string(),
                "inventory_target".to_string(),
                "inventory_skew_factor".to_string(),
                "min_edge_bps".to_string(),
                "order_refresh_interval_ms".to_string(),
            ],
        }
    }

    pub fn get_all_templates() -> HashMap<String, Self> {
        let mut templates = HashMap::new();
        templates.insert("market_making".to_string(), Self::market_making());
        templates
    }
}
