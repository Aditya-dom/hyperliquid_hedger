use tokio::time::{interval_at, Instant, Interval};
use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::borrow::Cow;

pub enum WSState {
    Continue,
    Closed,
    Err(anyhow::Error),
}

pub struct ConnectionTimers {
    pub ping_timer: Interval,
    pub stale_timer: Interval,
    pub stats_timer: Interval,
    pub last_alert: Instant,
}

impl Default for ConnectionTimers {
    fn default() -> Self { 
       let start = Instant::now() + Duration::from_secs(10);
       Self { ping_timer: interval_at(start, Duration::from_secs(56)), 
        stale_timer: interval_at(start, Duration::from_secs(10)), 
        stats_timer: interval_at(start, Duration::from_secs(30)), // can extend to have latency stats
        last_alert: Instant::now(),
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
