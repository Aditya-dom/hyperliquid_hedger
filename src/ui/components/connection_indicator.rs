use egui::{Ui, Color32};
use crate::ui::app::ConnectionStatus;

pub fn show_connection_status(ui: &mut Ui, status: &ConnectionStatus) {
        match status {
            ConnectionStatus::Connected => {
                ui.colored_label(Color32::from_rgb(40, 167, 69), "🟢 Connected");
            }
            ConnectionStatus::Connecting => {
                ui.colored_label(Color32::from_rgb(255, 193, 7), "🟡 Connecting");
            }
            ConnectionStatus::Disconnected => {
                ui.colored_label(Color32::from_rgb(220, 53, 69), "🔴 Disconnected");
            }
            ConnectionStatus::Error(err) => {
                let error_text = format!("❌ Error: {}", err.chars().take(30).collect::<String>());
                ui.colored_label(Color32::from_rgb(220, 53, 69), error_text);
            },
        }
}
