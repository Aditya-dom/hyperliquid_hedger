use crate::model::hl_msgs::PriceLevel;
use crate::trading::types::*;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: BTreeMap<Decimal, Decimal>, // price -> size
    pub asks: BTreeMap<Decimal, Decimal>, // price -> size
    pub last_update: DateTime<Utc>,
    pub sequence: u64,
}

impl OrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update: Utc::now(),
            sequence: 0,
        }
    }

    pub fn update_from_tob(&mut self, tob_data: &crate::model::hl_msgs::OrderBookData) {
        self.bids.clear();
        self.asks.clear();

        // Extract bids and asks from levels array
        if let Some(bid_levels) = tob_data.levels.get(0) {
            for level in bid_levels {
                if let (Ok(price), Ok(size)) = (
                    Decimal::from_str(&level.px),
                    Decimal::from_str(&level.sz),
                ) {
                    self.bids.insert(price, size);
                }
            }
        }

        if let Some(ask_levels) = tob_data.levels.get(1) {
            for level in ask_levels {
                if let (Ok(price), Ok(size)) = (
                    Decimal::from_str(&level.px),
                    Decimal::from_str(&level.sz),
                ) {
                    self.asks.insert(price, size);
                }
            }
        }

        // BTreeMap is automatically sorted by key

        self.last_update = Utc::now();
        self.sequence += 1;
    }

    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next_back().map(|(p, s)| (*p, *s))
    }

    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, s)| (*p, *s))
    }

    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some((bid + ask) / Decimal::from(2)),
            _ => None,
        }
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some(ask - bid),
            _ => None,
        }
    }

    pub fn spread_bps(&self) -> Option<Decimal> {
        self.spread().and_then(|spread| {
            self.mid_price().map(|mid| {
                if mid > Decimal::ZERO {
                    spread / mid * Decimal::from(10000)
                } else {
                    Decimal::ZERO
                }
            })
        })
    }

    pub fn volume_weighted_mid(&self, depth: usize) -> Option<Decimal> {
        let mut bid_vol = Decimal::ZERO;
        let mut ask_vol = Decimal::ZERO;
        let mut bid_weighted_sum = Decimal::ZERO;
        let mut ask_weighted_sum = Decimal::ZERO;

        for (i, (price, size)) in self.bids.iter().rev().enumerate() {
            if i >= depth { break; }
            bid_vol += size;
            bid_weighted_sum += price * size;
        }

        for (i, (price, size)) in self.asks.iter().enumerate() {
            if i >= depth { break; }
            ask_vol += size;
            ask_weighted_sum += price * size;
        }

        if bid_vol > Decimal::ZERO && ask_vol > Decimal::ZERO {
            let bid_vwap = bid_weighted_sum / bid_vol;
            let ask_vwap = ask_weighted_sum / ask_vol;
            Some((bid_vwap + ask_vwap) / Decimal::from(2))
        } else {
            None
        }
    }

    pub fn get_depth(&self, levels: usize) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let bids: Vec<(Decimal, Decimal)> = self.bids
            .iter()
            .rev()
            .take(levels)
            .map(|(p, s)| (*p, *s))
            .collect();
        
        let asks: Vec<(Decimal, Decimal)> = self.asks
            .iter()
            .take(levels)
            .map(|(p, s)| (*p, *s))
            .collect();

        (bids, asks)
    }
}
