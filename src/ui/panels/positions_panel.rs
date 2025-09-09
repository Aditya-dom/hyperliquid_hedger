use crate::trading::position_manager::PositionManager;
use egui::{Ui, Grid, Color32};
use rust_decimal::Decimal;

pub fn show(ui: &mut Ui, position_manager: &PositionManager) {
    ui.group(|ui| {
        ui.set_min_height(200.0);
        
        // Overall PnL summary
        ui.horizontal(|ui| {
            let total_pnl = position_manager.get_total_pnl();
            let unrealized_pnl = position_manager.get_total_unrealized_pnl();
            let realized_pnl = position_manager.realized_pnl.clone();
            
            let pnl_color = if total_pnl >= Decimal::ZERO {
                Color32::from_rgb(40, 167, 69)
            } else {
                Color32::from_rgb(220, 53, 69)
            };
            
            ui.colored_label(pnl_color, format!("Total PnL: ${:.2}", total_pnl));
            ui.label(format!("Unrealized: ${:.2}", unrealized_pnl));
            ui.label(format!("Realized: ${:.2}", *realized_pnl.read()));
        });
        
        ui.separator();
        
        // Positions table
        if position_manager.positions.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No positions");
            });
        } else {
            Grid::new("positions_grid")
                .num_columns(6)
                .spacing([10.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    // Header
                    ui.label("Symbol");
                    ui.label("Size");
                    ui.label("Entry Price");
                    ui.label("Mark Price");
                    ui.label("Unrealized PnL");
                    ui.label("Value");
                    ui.end_row();
                    
                    for entry in position_manager.positions.iter() {
                        let position = entry.value();
                        if position.size == Decimal::ZERO {
                            continue;
                        }
                        
                        ui.label(&position.symbol);
                        
                        // Size with color based on long/short
                        let size_color = if position.size > Decimal::ZERO {
                            Color32::from_rgb(40, 167, 69) // Green for long
                        } else {
                            Color32::from_rgb(220, 53, 69) // Red for short
                        };
                        ui.colored_label(size_color, format!("{:.4}", position.size));
                        
                        ui.label(format!("${:.4}", position.entry_price));
                        ui.label(format!("${:.4}", position.mark_price));
                        
                        // PnL with color
                        let pnl_color = if position.unrealized_pnl >= Decimal::ZERO {
                            Color32::from_rgb(40, 167, 69)
                        } else {
                            Color32::from_rgb(220, 53, 69)
                        };
                        ui.colored_label(pnl_color, format!("${:.2}", position.unrealized_pnl));
                        
                        let position_value = position.size * position.mark_price;
                        ui.label(format!("${:.2}", position_value.abs()));
                        ui.end_row();
                    }
                });
        }
        
        ui.separator();
        
        // Risk metrics
        ui.horizontal(|ui| {
            let net_exposure = position_manager.get_net_exposure();
            ui.label(format!("Net Exposure: ${:.2}", net_exposure));
            ui.label(format!("Total Fees: ${:.2}", *position_manager.total_fees.read()));
        });
    });
}
