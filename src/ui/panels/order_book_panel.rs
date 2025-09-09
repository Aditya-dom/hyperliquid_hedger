use crate::trading::order_book::OrderBook;
use egui::{Ui, Grid, Color32};
use rust_decimal::Decimal;

pub fn show(ui: &mut Ui, order_book: &OrderBook) {
    ui.group(|ui| {
        ui.set_min_height(300.0);
        
        if order_book.bids.is_empty() && order_book.asks.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No order book data");
            });
            return;
        }

        let (bids, asks) = order_book.get_depth(10);
        
        Grid::new("order_book_grid")
            .num_columns(3)
            .spacing([10.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                // Header
                ui.label("Size");
                ui.label("Price");
                ui.label("Side");
                ui.end_row();
                
                // Show asks in reverse order (highest to lowest)
                for (price, size) in asks.iter().rev() {
                    ui.label(format!("{:.4}", size));
                    ui.colored_label(Color32::from_rgb(220, 53, 69), format!("{:.4}", price));
                    ui.colored_label(Color32::from_rgb(220, 53, 69), "ASK");
                    ui.end_row();
                }
                
                // Spread row
                if let (Some((best_bid, _)), Some((best_ask, _))) = (bids.first(), asks.first()) {
                    let spread = best_ask - best_bid;
                    let spread_pct = (spread / ((best_bid + best_ask) / Decimal::from(2))) * Decimal::from(100);
                    
                    ui.label("");
                    ui.colored_label(
                        Color32::from_rgb(108, 117, 125),
                        format!("Spread: {:.4} ({:.2}%)", spread, spread_pct)
                    );
                    ui.label("");
                    ui.end_row();
                }
                
                // Show bids (highest to lowest)
                for (price, size) in &bids {
                    ui.label(format!("{:.4}", size));
                    ui.colored_label(Color32::from_rgb(40, 167, 69), format!("{:.4}", price));
                    ui.colored_label(Color32::from_rgb(40, 167, 69), "BID");
                    ui.end_row();
                }
            });
        
        ui.separator();
        
        // Order book statistics
        ui.horizontal(|ui| {
            ui.label("Stats:");
            if let Some(mid) = order_book.mid_price() {
                ui.label(format!("Mid: {:.4}", mid));
            }
            if let Some(vwap) = order_book.volume_weighted_mid(5) {
                ui.label(format!("VWAP(5): {:.4}", vwap));
            }
            ui.label(format!("Updates: {}", order_book.sequence));
        });
    });
}
