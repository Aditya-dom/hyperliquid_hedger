use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub base_url: String,
    pub ws_url: String,
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.hyperliquid.xyz".to_string(),
            ws_url: "wss://api.hyperliquid.xyz/ws".to_string(),
            timeout_ms: 5000,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidOrder {
    pub a: Option<u64>, // account id
    pub b: bool,        // is buy
    pub p: String,      // price
    pub s: String,      // size
    pub r: bool,        // reduce only
    pub t: String,      // order type
    pub cid: u64,       // client order id
    pub oid: Option<u64>, // order id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidOrderResponse {
    pub status: String,
    pub response: Option<HyperLiquidOrderResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidOrderResult {
    pub type_: String,
    pub data: Option<HyperLiquidOrderData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidOrderData {
    pub statuses: Vec<HyperLiquidOrderStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidOrderStatus {
    pub rest: Option<HyperLiquidOrderRest>,
    pub filled: Option<HyperLiquidOrderFilled>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidOrderRest {
    pub oid: u64,
    pub total_sz: String,
    pub sz: String,
    pub px: String,
    pub side: String,
    pub cloid: Option<String>,
    pub reduce_only: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidOrderFilled {
    pub oid: u64,
    pub total_sz: String,
    pub sz: String,
    pub px: String,
    pub side: String,
    pub cloid: Option<String>,
    pub reduce_only: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidPosition {
    pub coin: String,
    pub szi: String,
    pub entry_px: String,
    pub position_value: String,
    pub unrealized_pnl: String,
    pub margin_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidAccountInfo {
    pub margin_summary: HyperLiquidMarginSummary,
    pub open_orders: Vec<HyperLiquidOrderRest>,
    pub asset_positions: Vec<HyperLiquidPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidMarginSummary {
    pub account_value: String,
    pub total_margin_used: String,
    pub total_ntl_pos: String,
    pub total_raw_usd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidFill {
    pub coin: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub time: u64,
    pub start_position: String,
    pub dir: String,
    pub closed_pnl: String,
    pub hash: String,
    pub oid: u64,
    pub crossed: bool,
    pub fee: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidUserState {
    pub asset_positions: Vec<HyperLiquidPosition>,
    pub cross_margin_summary: HyperLiquidMarginSummary,
    pub margin_summary: HyperLiquidMarginSummary,
    pub withdrawable: String,
    pub open_orders: Vec<HyperLiquidOrderRest>,
    pub equity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperLiquidUserStateResponse {
    pub status: String,
    pub response: Option<HyperLiquidUserState>,
}

#[derive(Debug, Clone)]
pub enum ApiError {
    NetworkError(String),
    ParseError(String),
    AuthenticationError(String),
    RateLimitError(String),
    OrderRejected(String),
    InsufficientBalance(String),
    InvalidOrder(String),
    Timeout(String),
    Unknown(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ApiError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ApiError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            ApiError::RateLimitError(msg) => write!(f, "Rate limit error: {}", msg),
            ApiError::OrderRejected(msg) => write!(f, "Order rejected: {}", msg),
            ApiError::InsufficientBalance(msg) => write!(f, "Insufficient balance: {}", msg),
            ApiError::InvalidOrder(msg) => write!(f, "Invalid order: {}", msg),
            ApiError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            ApiError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::ParseError(err.to_string())
    }
}

impl From<reqwest::header::InvalidHeaderValue> for ApiError {
    fn from(err: reqwest::header::InvalidHeaderValue) -> Self {
        ApiError::NetworkError(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum ApiEvent {
    OrderUpdate {
        order_id: u64,
        status: String,
        filled_size: String,
        remaining_size: String,
        price: String,
        timestamp: u64,
    },
    Fill {
        order_id: u64,
        fill_size: String,
        fill_price: String,
        fee: String,
        timestamp: u64,
    },
    PositionUpdate {
        coin: String,
        size: String,
        entry_price: String,
        unrealized_pnl: String,
    },
    AccountUpdate {
        account_value: String,
        margin_used: String,
        withdrawable: String,
    },
    Error {
        error: String,
        timestamp: u64,
    },
}
