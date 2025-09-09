use crate::trading::types::*;
use chrono::Utc;
use rust_decimal::Decimal;
use dashmap::DashMap;
use crossbeam_channel::{Sender, Receiver};
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use uuid::Uuid;

pub struct OrderManager {
    pub orders: Arc<DashMap<Uuid, Order>>,
    pub orders_by_symbol: Arc<DashMap<String, Vec<Uuid>>>,
    pub pending_actions: Arc<RwLock<Vec<OrderAction>>>,
    pub order_events_tx: Sender<OrderEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderEvent {
    OrderPlaced(Order),
    OrderUpdated(Order),
    OrderCancelled(Uuid),
    OrderFilled(Order),
}

impl OrderManager {
    pub fn new() -> (Self, Receiver<OrderEvent>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        
        let manager = Self {
            orders: Arc::new(DashMap::new()),
            orders_by_symbol: Arc::new(DashMap::new()),
            pending_actions: Arc::new(RwLock::new(Vec::new())),
            order_events_tx: tx,
        };
        
        (manager, rx)
    }

    pub fn add_order(&self, new_order: NewOrder) -> Uuid {
        let order_id = Uuid::new_v4();
        let order = Order {
            id: order_id,
            client_id: new_order.client_id,
            symbol: new_order.symbol.clone(),
            side: new_order.side,
            order_type: new_order.order_type,
            price: new_order.price,
            size: new_order.size,
            filled_size: Decimal::ZERO,
            remaining_size: new_order.size,
            status: OrderStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.orders.insert(order_id, order.clone());
        
        // Index by symbol
        self.orders_by_symbol
            .entry(new_order.symbol)
            .or_insert_with(Vec::new)
            .push(order_id);

        // Send event
        let _ = self.order_events_tx.send(OrderEvent::OrderPlaced(order));

        order_id
    }

    pub fn update_order(&self, order_id: Uuid, status: OrderStatus, filled_size: Option<Decimal>) {
        if let Some(mut order) = self.orders.get_mut(&order_id) {
            order.status = status;
            order.updated_at = Utc::now();
            
            if let Some(filled) = filled_size {
                order.filled_size = filled;
                order.remaining_size = order.size - filled;
            }

            // Send event
            let _ = self.order_events_tx.send(OrderEvent::OrderUpdated(order.clone()));
            
            if matches!(status, OrderStatus::Filled) {
                let _ = self.order_events_tx.send(OrderEvent::OrderFilled(order.clone()));
            }
        }
    }

    pub fn cancel_order(&self, order_id: Uuid) {
        let mut pending = self.pending_actions.write();
        pending.push(OrderAction {
            action_type: OrderActionType::Cancel,
            order: None,
            order_id: Some(order_id),
        });
        
        // Send event
        let _ = self.order_events_tx.send(OrderEvent::OrderCancelled(order_id));
    }

    pub fn cancel_all_orders(&self, symbol: Option<&str>) {
        let orders_to_cancel: Vec<Uuid> = if let Some(symbol) = symbol {
            self.orders_by_symbol
                .get(symbol)
                .map(|order_ids| {
                    order_ids.iter()
                        .filter(|&&id| {
                            self.orders.get(&id)
                                .map(|o| matches!(o.status, OrderStatus::Pending | OrderStatus::Submitted | OrderStatus::PartiallyFilled))
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect()
                })
                .unwrap_or_default()
        } else {
            self.orders
                .iter()
                .filter(|entry| {
                    matches!(entry.value().status, OrderStatus::Pending | OrderStatus::Submitted | OrderStatus::PartiallyFilled)
                })
                .map(|entry| *entry.key())
                .collect()
        };

        for order_id in orders_to_cancel {
            self.cancel_order(order_id);
        }
    }

    pub fn get_active_orders(&self, symbol: Option<&str>) -> Vec<Order> {
        if let Some(symbol) = symbol {
            self.orders_by_symbol
                .get(symbol)
                .map(|order_ids| {
                    order_ids.iter()
                        .filter_map(|&order_id| self.orders.get(&order_id))
                        .filter(|order| {
                            matches!(order.status, OrderStatus::Pending | OrderStatus::Submitted | OrderStatus::PartiallyFilled)
                        })
                        .map(|order| order.clone())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            self.orders
                .iter()
                .filter(|entry| {
                    matches!(entry.value().status, OrderStatus::Pending | OrderStatus::Submitted | OrderStatus::PartiallyFilled)
                })
                .map(|entry| entry.value().clone())
                .collect()
        }
    }

    pub fn get_orders_by_side(&self, symbol: &str, side: Side) -> Vec<Order> {
        self.get_active_orders(Some(symbol))
            .into_iter()
            .filter(|order| order.side == side)
            .collect()
    }

    pub fn drain_pending_actions(&self) -> Vec<OrderAction> {
        let mut pending = self.pending_actions.write();
        std::mem::take(&mut *pending)
    }

    pub fn get_order_count(&self, symbol: &str) -> (usize, usize) {
        let buy_count = self.get_orders_by_side(symbol, Side::Buy).len();
        let sell_count = self.get_orders_by_side(symbol, Side::Sell).len();
        (buy_count, sell_count)
    }

    pub fn get_total_exposure(&self, symbol: &str) -> (Decimal, Decimal) {
        let active_orders = self.get_active_orders(Some(symbol));
        let mut buy_exposure = Decimal::ZERO;
        let mut sell_exposure = Decimal::ZERO;

        for order in active_orders {
            let exposure = order.remaining_size * order.price;
            match order.side {
                Side::Buy => buy_exposure += exposure,
                Side::Sell => sell_exposure += exposure,
            }
        }

        (buy_exposure, sell_exposure)
    }

    pub fn get_order(&self, order_id: &Uuid) -> Option<Order> {
        self.orders.get(order_id).map(|order| order.clone())
    }

    pub fn len(&self) -> usize {
        self.orders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new().0
    }
}

impl Clone for OrderManager {
    fn clone(&self) -> Self {
        Self {
            orders: Arc::clone(&self.orders),
            orders_by_symbol: Arc::clone(&self.orders_by_symbol),
            pending_actions: Arc::clone(&self.pending_actions),
            order_events_tx: self.order_events_tx.clone(),
        }
    }
}
