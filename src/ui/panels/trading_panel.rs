use crate::ui::app::ManualOrderState;
use crate::trading::order_manager::OrderManager;
use crate::trading::types::*;
use egui::{Ui, ComboBox, Button, Color32};
use rust_decimal::Decimal;
use std::str::FromStr;

pub fn show(ui: &mut Ui, manual_order: &mut ManualOrderState, order_manager: &OrderManager) {
    ui.group(|ui| {
        ui.set_min_height(200.0);
        
        egui::Grid::new("manual_order_grid")
            .num_columns(2)
            .spacing([10.0, 5.0])
            .show(ui, |ui| {
                // Symbol
                ui.label("Symbol:");
                ui.text_edit_singleline(&mut manual_order.symbol);
                ui.end_row();
                
                // Side
                ui.label("Side:");
                ComboBox::from_id_salt("order_side")
                    .selected_text(format!("{:?}", manual_order.side))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut manual_order.side, Side::Buy, "Buy");
                        ui.selectable_value(&mut manual_order.side, Side::Sell, "Sell");
                    });
                ui.end_row();
                
                // Order Type
                ui.label("Type:");
                ComboBox::from_id_salt("order_type")
                    .selected_text(format!("{:?}", manual_order.order_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut manual_order.order_type, OrderType::Limit, "Limit");
                        ui.selectable_value(&mut manual_order.order_type, OrderType::Market, "Market");
                        ui.selectable_value(&mut manual_order.order_type, OrderType::PostOnly, "Post Only");
                    });
                ui.end_row();
                
                // Price
                ui.label("Price:");
                ui.text_edit_singleline(&mut manual_order.price);
                ui.end_row();
                
                // Size
                ui.label("Size:");
                ui.text_edit_singleline(&mut manual_order.size);
                ui.end_row();
            });
        
        ui.separator();
        
        // Order buttons
        ui.horizontal(|ui| {
            let buy_button = Button::new("Place Buy Order")
                .fill(Color32::from_rgb(40, 167, 69));
            
            if ui.add(buy_button).clicked() {
                if let (Ok(price), Ok(size)) = (
                    Decimal::from_str(&manual_order.price),
                    Decimal::from_str(&manual_order.size),
                ) {
                    let new_order = NewOrder {
                        symbol: manual_order.symbol.clone(),
                        side: Side::Buy,
                        order_type: manual_order.order_type,
                        price,
                        size,
                        client_id: Some("manual_buy".to_string()),
                    };
                    order_manager.add_order(new_order);
                }
            }
            
            let sell_button = Button::new("Place Sell Order")
                .fill(Color32::from_rgb(220, 53, 69));
            
            if ui.add(sell_button).clicked() {
                if let (Ok(price), Ok(size)) = (
                    Decimal::from_str(&manual_order.price),
                    Decimal::from_str(&manual_order.size),
                ) {
                    let new_order = NewOrder {
                        symbol: manual_order.symbol.clone(),
                        side: Side::Sell,
                        order_type: manual_order.order_type,
                        price,
                        size,
                        client_id: Some("manual_sell".to_string()),
                    };
                    order_manager.add_order(new_order);
                }
            }
        });
        
        ui.separator();
        
        // Active orders
        ui.label("Active Orders:");
        
        let active_orders = order_manager.get_active_orders(Some(&manual_order.symbol));
        
        if active_orders.is_empty() {
            ui.label("No active orders");
        } else {
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    for order in active_orders {
                        ui.horizontal(|ui| {
                            let side_color = match order.side {
                                Side::Buy => Color32::from_rgb(40, 167, 69),
                                Side::Sell => Color32::from_rgb(220, 53, 69),
                            };
                            
                            ui.colored_label(side_color, format!("{:?}", order.side));
                            ui.label(format!("{:.4}", order.price));
                            ui.label(format!("{:.4}", order.remaining_size));
                            ui.label(format!("{:?}", order.status));
                            
                            if ui.button("Cancel").clicked() {
                                order_manager.cancel_order(order.id);
                            }
                        });
                    }
                });
        }
        
        ui.separator();
        
        // Cancel all button
        if ui.button("Cancel All Orders").clicked() {
            order_manager.cancel_all_orders(Some(&manual_order.symbol));
        }
    });
}
