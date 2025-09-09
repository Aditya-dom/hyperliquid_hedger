use egui::{Ui, Grid, Color32};
use crate::trading::types::{Order, Side, OrderStatus};

pub fn show_order_table(ui: &mut Ui, orders: &[&Order]) {
    Grid::new("order_table")
        .num_columns(6)
        .spacing([10.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // Header
            ui.label("Side");
            ui.label("Price");
            ui.label("Size");
            ui.label("Filled");
            ui.label("Status");
            ui.label("Action");
            ui.end_row();
            
            for order in orders {
                let side_color = match order.side {
                    Side::Buy => Color32::from_rgb(40, 167, 69),
                    Side::Sell => Color32::from_rgb(220, 53, 69),
                };
                
                ui.colored_label(side_color, format!("{:?}", order.side));
                ui.label(format!("{:.4}", order.price));
                ui.label(format!("{:.4}", order.size));
                ui.label(format!("{:.4}", order.filled_size));
                ui.label(format!("{:?}", order.status));
                
                if matches!(order.status, OrderStatus::Pending | OrderStatus::Submitted | OrderStatus::PartiallyFilled) {
                    if ui.button("Cancel").clicked() {
                        // Cancel order - would need callback
                    }
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
}
