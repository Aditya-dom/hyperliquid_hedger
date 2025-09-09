use crate::trading::types::*;
use crate::trading::order_manager::OrderEvent;
use crate::trading::position_manager::PositionEvent;
use crate::model::hl_msgs::TobMsg;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    // Market data events
    MarketData {
        symbol: String,
        data: TobMsg,
        timestamp: DateTime<Utc>,
    },
    
    // Order events
    Order(OrderEvent),
    
    // Position events
    Position(PositionEvent),
    
    // Strategy events
    Strategy {
        strategy_name: String,
        event: StrategyEvent,
        timestamp: DateTime<Utc>,
    },
    
    // Connection events
    Connection {
        connection_id: String,
        event: ConnectionEvent,
        timestamp: DateTime<Utc>,
    },
    
    // Risk events
    Risk {
        symbol: String,
        event: RiskEvent,
        timestamp: DateTime<Utc>,
    },
    
    // System events
    System {
        event: SystemLevelEvent,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyEvent {
    Started,
    Stopped,
    OrdersGenerated(Vec<OrderAction>),
    ParametersUpdated,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionEvent {
    Connected,
    Disconnected,
    Reconnecting,
    Error(String),
    MessageReceived,
    MessageSent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskEvent {
    LimitExceeded {
        limit_type: String,
        current_value: String,
        limit_value: String,
    },
    PositionSizeWarning {
        current_size: String,
        limit: String,
    },
    PnlWarning {
        current_pnl: String,
        limit: String,
    },
    OrderRejected {
        order_id: Uuid,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemLevelEvent {
    Startup,
    Shutdown,
    ConfigurationChanged,
    PerformanceMetric {
        metric_name: String,
        value: f64,
        unit: String,
    },
    Error {
        component: String,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct EventMetadata {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub priority: EventPriority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl SystemEvent {
    pub fn new_market_data(symbol: String, data: TobMsg) -> Self {
        Self::MarketData {
            symbol,
            data,
            timestamp: Utc::now(),
        }
    }
    
    pub fn new_strategy_event(strategy_name: String, event: StrategyEvent) -> Self {
        Self::Strategy {
            strategy_name,
            event,
            timestamp: Utc::now(),
        }
    }
    
    pub fn new_connection_event(connection_id: String, event: ConnectionEvent) -> Self {
        Self::Connection {
            connection_id,
            event,
            timestamp: Utc::now(),
        }
    }
    
    pub fn new_risk_event(symbol: String, event: RiskEvent) -> Self {
        Self::Risk {
            symbol,
            event,
            timestamp: Utc::now(),
        }
    }
    
    pub fn new_system_event(event: SystemLevelEvent) -> Self {
        Self::System {
            event,
            timestamp: Utc::now(),
        }
    }
    
    pub fn priority(&self) -> EventPriority {
        match self {
            Self::Risk { .. } => EventPriority::High,
            Self::System { event: SystemLevelEvent::Error { .. }, .. } => EventPriority::Critical,
            Self::Connection { event: ConnectionEvent::Error(_), .. } => EventPriority::High,
            Self::Strategy { event: StrategyEvent::Error(_), .. } => EventPriority::High,
            Self::Order(_) => EventPriority::Normal,
            Self::Position(_) => EventPriority::Normal,
            Self::MarketData { .. } => EventPriority::Low,
            _ => EventPriority::Normal,
        }
    }
    
    pub fn source(&self) -> String {
        match self {
            Self::MarketData { .. } => "market_data".to_string(),
            Self::Order(_) => "order_manager".to_string(),
            Self::Position(_) => "position_manager".to_string(),
            Self::Strategy { strategy_name, .. } => format!("strategy:{}", strategy_name),
            Self::Connection { connection_id, .. } => format!("connection:{}", connection_id),
            Self::Risk { .. } => "risk_manager".to_string(),
            Self::System { .. } => "system".to_string(),
        }
    }
}
