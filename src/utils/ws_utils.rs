use tokio::time::{interval_at, Instant, Interval};
use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::borrow::Cow;

pub struct ConnectionTimers {
    pub ping_timer: Interval,
    pub stale_timer: Interval,
    pub stats_timer: Interval,
    pub last_alert: i64,
}

impl Default for ConnectionTimers {
    fn default() -> Self { 
       let start = Instant::now() + Duration::from_secs(10);
       Self { ping_timer: interval_at(start, Duration::from_millis(10)), 
        stale_timer: interval_at(start, Duration::from_secs(10)), 
        stats_timer: interval_at(start, Duration::from_secs(30)), 
        last_alert: 0,
        }
    }
}

/// See: <https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/websocket/subscriptions>
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HypeStreamRequest<'h> {
    pub method: &'static str,
    pub subscription: SubscriptionType<'h>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SubscriptionType<'h> {
    L2Book(L2BookSubscription<'h>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct L2BookSubscription<'h> {
    #[serde(rename = "type")]
    pub type_field: Cow<'h, str>,
    pub coin: Cow<'h, str>,
}

pub struct Ping {
    pub method: &'static str,
}

impl Ping {
    pub fn ping() -> Self {
        Self { method: "ping "}
    }
}