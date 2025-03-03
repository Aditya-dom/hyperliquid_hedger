use tokio::time::{interval_at, Instant, Interval};
use std::time::Duration;

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