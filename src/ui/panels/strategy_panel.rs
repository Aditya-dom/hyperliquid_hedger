use crate::strategies::market_making::MarketMakingStrategy;
use crate::strategies::base_strategy::TradingStrategy;
use egui::{Ui, Slider, Button, Color32, DragValue};
use rust_decimal::Decimal;

pub fn show(ui: &mut Ui, strategy: &mut MarketMakingStrategy) {
    ui.group(|ui| {
        ui.set_min_height(250.0);
        
        // Strategy controls
        ui.horizontal(|ui| {
            let mut enabled = strategy.is_enabled();
            if ui.checkbox(&mut enabled, "Strategy Enabled").changed() {
                strategy.set_enabled(enabled);
            }
            
            if enabled {
                ui.colored_label(Color32::from_rgb(40, 167, 69), "● RUNNING");
            } else {
                ui.colored_label(Color32::from_rgb(220, 53, 69), "● STOPPED");
            }
        });
        
        ui.separator();
        
        // Strategy parameters
        egui::Grid::new("strategy_params")
            .num_columns(2)
            .spacing([10.0, 5.0])
            .show(ui, |ui| {
                ui.label("Spread (bps):");
                ui.add(DragValue::new(&mut strategy.config.spread_bps).range(1..=1000));
                ui.end_row();
                
                ui.label("Order Size:");
                let mut order_size_f64 = strategy.config.order_size.to_string().parse::<f64>().unwrap_or(1.0);
                if ui.add(DragValue::new(&mut order_size_f64).range(0.001..=100.0).speed(0.1)).changed() {
                    strategy.config.order_size = Decimal::try_from(order_size_f64).unwrap_or(strategy.config.order_size);
                }
                ui.end_row();
                
                ui.label("Max Orders/Side:");
                ui.add(DragValue::new(&mut strategy.config.max_orders_per_side).range(1..=10));
                ui.end_row();
                
                ui.label("Min Edge (bps):");
                ui.add(DragValue::new(&mut strategy.config.min_edge_bps).range(1..=100));
                ui.end_row();
                
                ui.label("Refresh Interval (ms):");
                ui.add(DragValue::new(&mut strategy.config.order_refresh_interval_ms).range(100..=10000));
                ui.end_row();
                
                ui.label("Inventory Skew:");
                let mut skew_f64 = strategy.config.inventory_skew_factor.to_string().parse::<f64>().unwrap_or(0.1);
                if ui.add(DragValue::new(&mut skew_f64).range(0.0..=1.0).speed(0.01)).changed() {
                    strategy.config.inventory_skew_factor = Decimal::try_from(skew_f64).unwrap_or(strategy.config.inventory_skew_factor);
                }
                ui.end_row();
            });
        
        ui.separator();
        
        // Strategy status
        ui.label("Status:");
        ui.horizontal(|ui| {
            ui.label(format!("Active Orders: {}", strategy.active_orders.len()));
            ui.label(format!("Current Inventory: {:.4}", strategy.current_inventory));
            if let Some(last_price) = strategy.last_price {
                ui.label(format!("Last Price: ${:.4}", last_price));
            }
        });
        
        ui.separator();
        
        // Manual controls
        ui.horizontal(|ui| {
            if ui.button("Cancel All MM Orders").clicked() {
                // This would trigger order cancellation
                strategy.active_orders.clear();
            }
            
            if ui.button("Reset Inventory").clicked() {
                strategy.current_inventory = Decimal::ZERO;
            }
        });
        
        ui.separator();
        
        // Performance metrics (placeholder)
        ui.label("Performance (Today):");
        ui.horizontal(|ui| {
            ui.label("Fills: 0");
            ui.label("Volume: $0.00");
            ui.label("Avg Spread Captured: 0.0 bps");
        });
    });
}
