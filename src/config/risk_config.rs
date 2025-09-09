use crate::trading::types::RiskLimits;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfigTemplate {
    pub name: String,
    pub description: String,
    pub risk_limits: RiskLimits,
    pub position_limits: HashMap<String, PositionLimitTemplate>,
    pub exposure_limits: HashMap<String, ExposureLimitTemplate>,
    pub volatility_limits: HashMap<String, VolatilityLimitTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionLimitTemplate {
    pub max_long: Decimal,
    pub max_short: Decimal,
    pub max_net: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureLimitTemplate {
    pub max_notional: Decimal,
    pub max_leverage: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityLimitTemplate {
    pub max_spread_bps: u32,
    pub max_price_change_bps: u32,
}

impl RiskConfigTemplate {
    pub fn conservative() -> Self {
        let mut position_limits = HashMap::new();
        position_limits.insert("HYPE".to_string(), PositionLimitTemplate {
            max_long: Decimal::from(10),
            max_short: Decimal::from(10),
            max_net: Decimal::from(5),
        });

        let mut exposure_limits = HashMap::new();
        exposure_limits.insert("HYPE".to_string(), ExposureLimitTemplate {
            max_notional: Decimal::from(1000),
            max_leverage: Decimal::from(2),
        });

        let mut volatility_limits = HashMap::new();
        volatility_limits.insert("HYPE".to_string(), VolatilityLimitTemplate {
            max_spread_bps: 50,
            max_price_change_bps: 100,
        });

        Self {
            name: "Conservative".to_string(),
            description: "Conservative risk limits for low-risk trading".to_string(),
            risk_limits: RiskLimits {
                max_position_size: Decimal::from(10),
                max_daily_loss: Decimal::from(100),
                max_order_size: Decimal::from(1),
                max_orders_per_side: 3,
            },
            position_limits,
            exposure_limits,
            volatility_limits,
        }
    }

    pub fn moderate() -> Self {
        let mut position_limits = HashMap::new();
        position_limits.insert("HYPE".to_string(), PositionLimitTemplate {
            max_long: Decimal::from(50),
            max_short: Decimal::from(50),
            max_net: Decimal::from(25),
        });

        let mut exposure_limits = HashMap::new();
        exposure_limits.insert("HYPE".to_string(), ExposureLimitTemplate {
            max_notional: Decimal::from(5000),
            max_leverage: Decimal::from(5),
        });

        let mut volatility_limits = HashMap::new();
        volatility_limits.insert("HYPE".to_string(), VolatilityLimitTemplate {
            max_spread_bps: 100,
            max_price_change_bps: 200,
        });

        Self {
            name: "Moderate".to_string(),
            description: "Moderate risk limits for balanced trading".to_string(),
            risk_limits: RiskLimits {
                max_position_size: Decimal::from(50),
                max_daily_loss: Decimal::from(500),
                max_order_size: Decimal::from(5),
                max_orders_per_side: 5,
            },
            position_limits,
            exposure_limits,
            volatility_limits,
        }
    }

    pub fn aggressive() -> Self {
        let mut position_limits = HashMap::new();
        position_limits.insert("HYPE".to_string(), PositionLimitTemplate {
            max_long: Decimal::from(100),
            max_short: Decimal::from(100),
            max_net: Decimal::from(50),
        });

        let mut exposure_limits = HashMap::new();
        exposure_limits.insert("HYPE".to_string(), ExposureLimitTemplate {
            max_notional: Decimal::from(10000),
            max_leverage: Decimal::from(10),
        });

        let mut volatility_limits = HashMap::new();
        volatility_limits.insert("HYPE".to_string(), VolatilityLimitTemplate {
            max_spread_bps: 200,
            max_price_change_bps: 500,
        });

        Self {
            name: "Aggressive".to_string(),
            description: "Aggressive risk limits for high-risk, high-reward trading".to_string(),
            risk_limits: RiskLimits {
                max_position_size: Decimal::from(100),
                max_daily_loss: Decimal::from(1000),
                max_order_size: Decimal::from(10),
                max_orders_per_side: 10,
            },
            position_limits,
            exposure_limits,
            volatility_limits,
        }
    }

    pub fn get_all_templates() -> HashMap<String, Self> {
        let mut templates = HashMap::new();
        templates.insert("conservative".to_string(), Self::conservative());
        templates.insert("moderate".to_string(), Self::moderate());
        templates.insert("aggressive".to_string(), Self::aggressive());
        templates
    }
}
