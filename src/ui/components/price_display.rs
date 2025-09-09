use egui::{Ui, Color32};
use rust_decimal::Decimal;

pub fn show_price(ui: &mut Ui, price: Decimal, precision: usize) {
    ui.label(format!("${:.prec$}", price, prec = precision));
}

pub fn show_colored_price(ui: &mut Ui, price: Decimal, reference_price: Option<Decimal>, precision: usize) {
    let color = if let Some(ref_price) = reference_price {
        if price > ref_price {
            Color32::from_rgb(40, 167, 69)  // Green
        } else if price < ref_price {
            Color32::from_rgb(220, 53, 69)  // Red
        } else {
            Color32::default()
        }
    } else {
        Color32::default()
    };
    
    ui.colored_label(color, format!("${:.prec$}", price, prec = precision));
}
