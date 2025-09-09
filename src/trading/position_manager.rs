use crate::trading::types::*;
use rust_decimal::Decimal;
use dashmap::DashMap;
use crossbeam_channel::{Sender, Receiver};
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

pub struct PositionManager {
    pub positions: Arc<DashMap<String, Position>>,
    pub realized_pnl: Arc<RwLock<Decimal>>,
    pub total_fees: Arc<RwLock<Decimal>>,
    pub position_events_tx: Sender<PositionEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionEvent {
    PositionUpdated(Position),
    FillProcessed(Fill),
    PnlRealized(Decimal),
}

impl PositionManager {
    pub fn new() -> (Self, Receiver<PositionEvent>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        
        let manager = Self {
            positions: Arc::new(DashMap::new()),
            realized_pnl: Arc::new(RwLock::new(Decimal::ZERO)),
            total_fees: Arc::new(RwLock::new(Decimal::ZERO)),
            position_events_tx: tx,
        };
        
        (manager, rx)
    }

    pub fn update_position(&self, symbol: String, size: Decimal, entry_price: Decimal, mark_price: Decimal) {
        let mut position = self.positions.entry(symbol.clone()).or_insert_with(|| Position {
            symbol: symbol.clone(),
            size: Decimal::ZERO,
            entry_price: Decimal::ZERO,
            mark_price,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            updated_at: chrono::Utc::now(),
        });

        position.size = size;
        position.entry_price = entry_price;
        position.mark_price = mark_price;
        position.updated_at = chrono::Utc::now();
        
        // Calculate unrealized PnL
        if position.size != Decimal::ZERO {
            position.unrealized_pnl = (mark_price - entry_price) * size;
        } else {
            position.unrealized_pnl = Decimal::ZERO;
        }

        // Send event
        let _ = self.position_events_tx.send(PositionEvent::PositionUpdated(position.clone()));
    }

    pub fn process_fill(&self, fill: &Fill) {
        let mut position = self.positions.entry(fill.symbol.clone()).or_insert_with(|| Position {
            symbol: fill.symbol.clone(),
            size: Decimal::ZERO,
            entry_price: Decimal::ZERO,
            mark_price: fill.price,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            updated_at: chrono::Utc::now(),
        });

        let fill_size = match fill.side {
            Side::Buy => fill.size,
            Side::Sell => -fill.size,
        };

        // Calculate realized PnL if reducing position
        if position.size != Decimal::ZERO && position.size.is_sign_positive() != fill_size.is_sign_positive() {
            let reducing_size = fill_size.abs().min(position.size.abs());
            let pnl_per_unit = match fill.side {
                Side::Sell => fill.price - position.entry_price,
                Side::Buy => position.entry_price - fill.price,
            };
            let realized_pnl = pnl_per_unit * reducing_size;
            position.realized_pnl += realized_pnl;
            
            // Update global realized PnL
            *self.realized_pnl.write() += realized_pnl;
            
            // Send event
            let _ = self.position_events_tx.send(PositionEvent::PnlRealized(realized_pnl));
        }

        // Update position
        let new_size = position.size + fill_size;
        
        if new_size == Decimal::ZERO {
            // Position closed
            position.size = Decimal::ZERO;
            position.entry_price = Decimal::ZERO;
            position.unrealized_pnl = Decimal::ZERO;
        } else if position.size == Decimal::ZERO || position.size.is_sign_positive() != new_size.is_sign_positive() {
            // New position or flipped position
            position.size = new_size;
            position.entry_price = fill.price;
        } else {
            // Adding to existing position - update weighted average entry price
            let total_cost = position.size * position.entry_price + fill_size * fill.price;
            position.size = new_size;
            position.entry_price = total_cost / new_size;
        }

        position.mark_price = fill.price;
        position.updated_at = chrono::Utc::now();
        
        // Update total fees
        *self.total_fees.write() += fill.fee;

        // Recalculate unrealized PnL
        if position.size != Decimal::ZERO {
            position.unrealized_pnl = (position.mark_price - position.entry_price) * position.size;
        }

        // Send events
        let _ = self.position_events_tx.send(PositionEvent::FillProcessed(fill.clone()));
        let _ = self.position_events_tx.send(PositionEvent::PositionUpdated(position.clone()));
    }

    pub fn update_mark_prices(&self, symbol: &str, mark_price: Decimal) {
        if let Some(mut position) = self.positions.get_mut(symbol) {
            position.mark_price = mark_price;
            if position.size != Decimal::ZERO {
                position.unrealized_pnl = (mark_price - position.entry_price) * position.size;
            }
            position.updated_at = chrono::Utc::now();

            // Send event
            let _ = self.position_events_tx.send(PositionEvent::PositionUpdated(position.clone()));
        }
    }

    pub fn get_position(&self, symbol: &str) -> Option<Position> {
        self.positions.get(symbol).map(|p| p.clone())
    }

    pub fn get_total_unrealized_pnl(&self) -> Decimal {
        self.positions
            .iter()
            .map(|entry| entry.value().unrealized_pnl)
            .sum()
    }

    pub fn get_total_pnl(&self) -> Decimal {
        *self.realized_pnl.read() + self.get_total_unrealized_pnl()
    }

    pub fn get_position_value(&self, symbol: &str) -> Decimal {
        if let Some(position) = self.positions.get(symbol) {
            position.size * position.mark_price
        } else {
            Decimal::ZERO
        }
    }

    pub fn get_net_exposure(&self) -> Decimal {
        self.positions
            .iter()
            .map(|entry| {
                let pos = entry.value();
                pos.size.abs() * pos.mark_price
            })
            .sum()
    }

    pub fn check_risk_limits(&self, limits: &RiskLimits, symbol: &str, new_order_size: Decimal) -> Result<(), String> {
        // Check position size limit
        let current_position = self.get_position(symbol).map_or(Decimal::ZERO, |p| p.size);
        let new_position_size = (current_position + new_order_size).abs();
        
        if new_position_size > limits.max_position_size {
            return Err(format!(
                "Position size {} would exceed limit {}",
                new_position_size, limits.max_position_size
            ));
        }

        // Check daily loss limit
        if self.get_total_pnl() < -limits.max_daily_loss {
            return Err(format!(
                "Daily loss {} exceeds limit {}",
                self.get_total_pnl(), limits.max_daily_loss
            ));
        }

        Ok(())
    }

    pub fn get_realized_pnl(&self) -> Decimal {
        *self.realized_pnl.read()
    }

    pub fn get_total_fees(&self) -> Decimal {
        *self.total_fees.read()
    }

    pub fn get_all_positions(&self) -> Vec<Position> {
        self.positions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

impl Default for PositionManager {
    fn default() -> Self {
        Self::new().0
    }
}

impl Clone for PositionManager {
    fn clone(&self) -> Self {
        Self {
            positions: Arc::clone(&self.positions),
            realized_pnl: Arc::clone(&self.realized_pnl),
            total_fees: Arc::clone(&self.total_fees),
            position_events_tx: self.position_events_tx.clone(),
        }
    }
}
