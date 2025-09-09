use crate::trading::types::*;
use crate::api::types::ApiEvent;
use anyhow::Result;
use crossbeam_channel::{Sender, Receiver, unbounded};
use dashmap::DashMap;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{error, info, warn, debug};
use uuid::Uuid;

pub struct RiskManager {
    pub risk_limits: Arc<DashMap<String, RiskLimits>>,
    pub position_limits: Arc<DashMap<String, PositionLimit>>,
    pub exposure_limits: Arc<DashMap<String, ExposureLimit>>,
    pub volatility_limits: Arc<DashMap<String, VolatilityLimit>>,
    pub circuit_breakers: Arc<RwLock<Vec<CircuitBreaker>>>,
    pub risk_events_tx: Sender<RiskEvent>,
    pub daily_pnl: Arc<RwLock<Decimal>>,
    pub daily_trades: Arc<RwLock<u32>>,
    pub last_reset: Arc<RwLock<Instant>>,
    pub risk_metrics: Arc<RwLock<RiskMetrics>>,
}

#[derive(Debug, Clone)]
pub struct PositionLimit {
    pub symbol: String,
    pub max_long: Decimal,
    pub max_short: Decimal,
    pub max_net: Decimal,
    pub current_long: Decimal,
    pub current_short: Decimal,
    pub current_net: Decimal,
}

#[derive(Debug, Clone)]
pub struct ExposureLimit {
    pub symbol: String,
    pub max_notional: Decimal,
    pub current_notional: Decimal,
    pub max_leverage: Decimal,
    pub current_leverage: Decimal,
}

#[derive(Debug, Clone)]
pub struct VolatilityLimit {
    pub symbol: String,
    pub max_spread_bps: u32,
    pub max_price_change_bps: u32,
    pub current_spread_bps: u32,
    pub last_price: Decimal,
    pub price_change_bps: u32,
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    pub id: String,
    pub symbol: String,
    pub trigger_type: CircuitBreakerType,
    pub threshold: Decimal,
    pub current_value: Decimal,
    pub is_triggered: bool,
    pub triggered_at: Option<Instant>,
    pub cooldown_duration: Duration,
}

#[derive(Debug, Clone)]
pub enum CircuitBreakerType {
    MaxDailyLoss,
    MaxPositionSize,
    MaxExposure,
    MaxVolatility,
    MaxTradesPerMinute,
    MaxOrdersPerSecond,
}

#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub total_exposure: Decimal,
    pub total_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub sharpe_ratio: Decimal,
    pub win_rate: Decimal,
    pub avg_trade_size: Decimal,
    pub last_updated: Instant,
}

#[derive(Debug, Clone)]
pub enum RiskEvent {
    LimitExceeded {
        limit_type: String,
        symbol: String,
        current_value: Decimal,
        limit_value: Decimal,
        severity: RiskSeverity,
    },
    CircuitBreakerTriggered {
        breaker_id: String,
        symbol: String,
        trigger_type: CircuitBreakerType,
        threshold: Decimal,
        current_value: Decimal,
    },
    RiskWarning {
        message: String,
        symbol: String,
        severity: RiskSeverity,
    },
    PositionRisk {
        symbol: String,
        position_size: Decimal,
        exposure: Decimal,
        pnl: Decimal,
        risk_score: Decimal,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskManager {
    pub fn new() -> (Self, Receiver<RiskEvent>) {
        let (tx, rx) = unbounded();
        
        let manager = Self {
            risk_limits: Arc::new(DashMap::new()),
            position_limits: Arc::new(DashMap::new()),
            exposure_limits: Arc::new(DashMap::new()),
            volatility_limits: Arc::new(DashMap::new()),
            circuit_breakers: Arc::new(RwLock::new(Vec::new())),
            risk_events_tx: tx,
            daily_pnl: Arc::new(RwLock::new(Decimal::ZERO)),
            daily_trades: Arc::new(RwLock::new(0)),
            last_reset: Arc::new(RwLock::new(Instant::now())),
            risk_metrics: Arc::new(RwLock::new(RiskMetrics {
                total_exposure: Decimal::ZERO,
                total_pnl: Decimal::ZERO,
                max_drawdown: Decimal::ZERO,
                sharpe_ratio: Decimal::ZERO,
                win_rate: Decimal::ZERO,
                avg_trade_size: Decimal::ZERO,
                last_updated: Instant::now(),
            })),
        };
        
        (manager, rx)
    }

    pub fn add_risk_limits(&self, symbol: String, limits: RiskLimits) {
        self.risk_limits.insert(symbol, limits);
        info!("Added risk limits for symbol");
    }

    pub fn add_position_limit(&self, symbol: String, limit: PositionLimit) {
        let symbol_clone = symbol.clone();
        self.position_limits.insert(symbol, limit);
        info!("Added position limit for {}", symbol_clone);
    }

    pub fn add_exposure_limit(&self, symbol: String, limit: ExposureLimit) {
        let symbol_clone = symbol.clone();
        self.exposure_limits.insert(symbol, limit);
        info!("Added exposure limit for {}", symbol_clone);
    }

    pub fn add_volatility_limit(&self, symbol: String, limit: VolatilityLimit) {
        let symbol_clone = symbol.clone();
        self.volatility_limits.insert(symbol, limit);
        info!("Added volatility limit for {}", symbol_clone);
    }

    pub fn add_circuit_breaker(&self, breaker: CircuitBreaker) {
        let mut breakers = self.circuit_breakers.write();
        breakers.push(breaker);
        info!("Added circuit breaker");
    }

    pub fn check_order_risk(&self, order: &NewOrder) -> Result<(), String> {
        let symbol = &order.symbol;
        
        // Check position limits
        if let Some(position_limit) = self.position_limits.get(symbol) {
            let new_position = match order.side {
                Side::Buy => position_limit.current_net + order.size,
                Side::Sell => position_limit.current_net - order.size,
            };

            if new_position.abs() > position_limit.max_net {
                return Err(format!(
                    "Order would exceed position limit: {} > {}",
                    new_position.abs(), position_limit.max_net
                ));
            }
        }

        // Check exposure limits
        if let Some(exposure_limit) = self.exposure_limits.get(symbol) {
            let order_notional = order.price * order.size;
            let new_exposure = exposure_limit.current_notional + order_notional;

            if new_exposure > exposure_limit.max_notional {
                return Err(format!(
                    "Order would exceed exposure limit: {} > {}",
                    new_exposure, exposure_limit.max_notional
                ));
            }
        }

        // Check daily loss limit
        {
            let daily_pnl = *self.daily_pnl.read();
            if let Some(risk_limits) = self.risk_limits.get(symbol) {
                if daily_pnl < -risk_limits.max_daily_loss {
                    return Err(format!(
                        "Daily loss limit exceeded: {} < {}",
                        daily_pnl, -risk_limits.max_daily_loss
                    ));
                }
            }
        }

        // Check circuit breakers
        {
            let breakers = self.circuit_breakers.read();
            for breaker in breakers.iter() {
                if breaker.symbol == *symbol && breaker.is_triggered {
                    if let Some(triggered_at) = breaker.triggered_at {
                        if triggered_at.elapsed() < breaker.cooldown_duration {
                            return Err(format!(
                                "Circuit breaker {} is still active",
                                breaker.id
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn update_position(&self, symbol: &str, size: Decimal, price: Decimal) {
        // Update position limits
        if let Some(mut position_limit) = self.position_limits.get_mut(symbol) {
            position_limit.current_net = size;
            if size > Decimal::ZERO {
                position_limit.current_long = size;
                position_limit.current_short = Decimal::ZERO;
            } else {
                position_limit.current_long = Decimal::ZERO;
                position_limit.current_short = -size;
            }
        }

        // Update exposure limits
        if let Some(mut exposure_limit) = self.exposure_limits.get_mut(symbol) {
            exposure_limit.current_notional = size.abs() * price;
            if exposure_limit.current_notional > Decimal::ZERO {
                // Calculate leverage (simplified)
                exposure_limit.current_leverage = exposure_limit.current_notional / Decimal::from(1000); // Assuming 1000 base
            }
        }

        // Check for limit violations
        self.check_position_limits(symbol);
        self.check_exposure_limits(symbol);
    }

    pub fn update_pnl(&self, pnl: Decimal) {
        let mut daily_pnl = self.daily_pnl.write();
        *daily_pnl += pnl;

        // Check daily loss limit
        for entry in self.risk_limits.iter() {
            let (symbol, limits) = (entry.key(), entry.value());
            if *daily_pnl < -limits.max_daily_loss {
                let _ = self.risk_events_tx.send(RiskEvent::LimitExceeded {
                    limit_type: "daily_loss".to_string(),
                    symbol: symbol.clone(),
                    current_value: *daily_pnl,
                    limit_value: -limits.max_daily_loss,
                    severity: RiskSeverity::Critical,
                });
            }
        }

        // Update risk metrics
        {
            let mut metrics = self.risk_metrics.write();
            metrics.total_pnl = *daily_pnl;
            metrics.last_updated = Instant::now();
        }
    }

    pub fn update_trade_count(&self) {
        let mut daily_trades = self.daily_trades.write();
        *daily_trades += 1;

        // Check trades per minute circuit breaker
        {
            let breakers = self.circuit_breakers.read();
            for breaker in breakers.iter() {
                if matches!(breaker.trigger_type, CircuitBreakerType::MaxTradesPerMinute) {
                    // This would need more sophisticated tracking in production
                    if *daily_trades > breaker.threshold.to_string().parse::<u32>().unwrap_or(0) {
                        self.trigger_circuit_breaker(breaker.id.clone());
                    }
                }
            }
        }
    }

    pub fn update_volatility(&self, symbol: &str, spread_bps: u32, price_change_bps: u32, current_price: Decimal) {
        if let Some(mut vol_limit) = self.volatility_limits.get_mut(symbol) {
            vol_limit.current_spread_bps = spread_bps;
            vol_limit.price_change_bps = price_change_bps;
            vol_limit.last_price = current_price;

            // Check volatility limits
            if spread_bps > vol_limit.max_spread_bps {
                let _ = self.risk_events_tx.send(RiskEvent::LimitExceeded {
                    limit_type: "spread".to_string(),
                    symbol: symbol.to_string(),
                    current_value: Decimal::from(spread_bps),
                    limit_value: Decimal::from(vol_limit.max_spread_bps),
                    severity: RiskSeverity::High,
                });
            }

            if price_change_bps > vol_limit.max_price_change_bps {
                let _ = self.risk_events_tx.send(RiskEvent::LimitExceeded {
                    limit_type: "price_change".to_string(),
                    symbol: symbol.to_string(),
                    current_value: Decimal::from(price_change_bps),
                    limit_value: Decimal::from(vol_limit.max_price_change_bps),
                    severity: RiskSeverity::High,
                });
            }
        }
    }

    fn check_position_limits(&self, symbol: &str) {
        if let Some(position_limit) = self.position_limits.get(symbol) {
            if position_limit.current_net.abs() > position_limit.max_net {
                let _ = self.risk_events_tx.send(RiskEvent::LimitExceeded {
                    limit_type: "position_size".to_string(),
                    symbol: symbol.to_string(),
                    current_value: position_limit.current_net.abs(),
                    limit_value: position_limit.max_net,
                    severity: RiskSeverity::Critical,
                });
            }
        }
    }

    fn check_exposure_limits(&self, symbol: &str) {
        if let Some(exposure_limit) = self.exposure_limits.get(symbol) {
            if exposure_limit.current_notional > exposure_limit.max_notional {
                let _ = self.risk_events_tx.send(RiskEvent::LimitExceeded {
                    limit_type: "exposure".to_string(),
                    symbol: symbol.to_string(),
                    current_value: exposure_limit.current_notional,
                    limit_value: exposure_limit.max_notional,
                    severity: RiskSeverity::High,
                });
            }

            if exposure_limit.current_leverage > exposure_limit.max_leverage {
                let _ = self.risk_events_tx.send(RiskEvent::LimitExceeded {
                    limit_type: "leverage".to_string(),
                    symbol: symbol.to_string(),
                    current_value: exposure_limit.current_leverage,
                    limit_value: exposure_limit.max_leverage,
                    severity: RiskSeverity::Critical,
                });
            }
        }
    }

    fn trigger_circuit_breaker(&self, breaker_id: String) {
        let mut breakers = self.circuit_breakers.write();
        if let Some(breaker) = breakers.iter_mut().find(|b| b.id == breaker_id) {
            breaker.is_triggered = true;
            breaker.triggered_at = Some(Instant::now());

            let _ = self.risk_events_tx.send(RiskEvent::CircuitBreakerTriggered {
                breaker_id: breaker.id.clone(),
                symbol: breaker.symbol.clone(),
                trigger_type: breaker.trigger_type.clone(),
                threshold: breaker.threshold,
                current_value: breaker.current_value,
            });

            warn!("Circuit breaker triggered: {}", breaker_id);
        }
    }

    pub fn reset_daily_metrics(&self) {
        {
            let mut daily_pnl = self.daily_pnl.write();
            *daily_pnl = Decimal::ZERO;
        }
        {
            let mut daily_trades = self.daily_trades.write();
            *daily_trades = 0;
        }
        {
            let mut last_reset = self.last_reset.write();
            *last_reset = Instant::now();
        }

        // Reset circuit breakers
        {
            let mut breakers = self.circuit_breakers.write();
            for breaker in breakers.iter_mut() {
                breaker.is_triggered = false;
                breaker.triggered_at = None;
            }
        }

        info!("Daily risk metrics reset");
    }

    pub fn get_risk_score(&self, symbol: &str) -> Decimal {
        let mut score = Decimal::ZERO;

        // Position size risk
        if let Some(position_limit) = self.position_limits.get(symbol) {
            let position_ratio = position_limit.current_net.abs() / position_limit.max_net;
            score += position_ratio * Decimal::from(40); // 40% weight
        }

        // Exposure risk
        if let Some(exposure_limit) = self.exposure_limits.get(symbol) {
            let exposure_ratio = exposure_limit.current_notional / exposure_limit.max_notional;
            score += exposure_ratio * Decimal::from(30); // 30% weight
        }

        // Volatility risk
        if let Some(vol_limit) = self.volatility_limits.get(symbol) {
            let spread_ratio = Decimal::from(vol_limit.current_spread_bps) / Decimal::from(vol_limit.max_spread_bps);
            score += spread_ratio * Decimal::from(20); // 20% weight

            let price_change_ratio = Decimal::from(vol_limit.price_change_bps) / Decimal::from(vol_limit.max_price_change_bps);
            score += price_change_ratio * Decimal::from(10); // 10% weight
        }

        score.min(Decimal::from(100)) // Cap at 100
    }

    pub fn get_risk_metrics(&self) -> RiskMetrics {
        self.risk_metrics.read().clone()
    }

    pub fn get_daily_pnl(&self) -> Decimal {
        *self.daily_pnl.read()
    }

    pub fn get_daily_trades(&self) -> u32 {
        *self.daily_trades.read()
    }

    pub fn is_circuit_breaker_active(&self, symbol: &str) -> bool {
        let breakers = self.circuit_breakers.read();
        breakers.iter().any(|breaker| {
            breaker.symbol == symbol && breaker.is_triggered && 
            breaker.triggered_at.map_or(false, |t| t.elapsed() < breaker.cooldown_duration)
        })
    }

    pub fn start_daily_reset_timer(&self) {
        let daily_pnl = Arc::clone(&self.daily_pnl);
        let daily_trades = Arc::clone(&self.daily_trades);
        let last_reset = Arc::clone(&self.last_reset);
        let circuit_breakers = Arc::clone(&self.circuit_breakers);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Check every hour
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let last_reset_time = *last_reset.read();
                
                // Reset if 24 hours have passed
                if now.duration_since(last_reset_time) >= Duration::from_secs(86400) {
                    *daily_pnl.write() = Decimal::ZERO;
                    *daily_trades.write() = 0;
                    *last_reset.write() = now;
                    
                    // Reset circuit breakers
                    {
                        let mut breakers = circuit_breakers.write();
                        for breaker in breakers.iter_mut() {
                            breaker.is_triggered = false;
                            breaker.triggered_at = None;
                        }
                    }
                    
                    info!("Daily risk metrics reset");
                }
            }
        });
    }
}

impl Default for RiskManager {
    fn default() -> Self {
        Self::new().0
    }
}

impl Clone for RiskManager {
    fn clone(&self) -> Self {
        Self {
            risk_limits: Arc::clone(&self.risk_limits),
            position_limits: Arc::clone(&self.position_limits),
            exposure_limits: Arc::clone(&self.exposure_limits),
            volatility_limits: Arc::clone(&self.volatility_limits),
            circuit_breakers: Arc::clone(&self.circuit_breakers),
            risk_events_tx: self.risk_events_tx.clone(),
            daily_pnl: Arc::clone(&self.daily_pnl),
            daily_trades: Arc::clone(&self.daily_trades),
            last_reset: Arc::clone(&self.last_reset),
            risk_metrics: Arc::clone(&self.risk_metrics),
        }
    }
}
