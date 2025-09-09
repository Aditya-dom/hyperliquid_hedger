use crate::ui::app::{LogEntry, LogLevel};
use egui::{Ui, Color32, ScrollArea};
use std::collections::VecDeque;

pub fn show(ui: &mut Ui, logs: &VecDeque<LogEntry>) {
    ui.group(|ui| {
        ui.set_min_height(150.0);
        
        // Controls
        ui.horizontal(|ui| {
            ui.label("Logs:");
            ui.label(format!("({} entries)", logs.len()));
            
            // Add filter controls here if needed
            if ui.button("Clear").clicked() {
                // This would clear logs - need to pass mutable reference
            }
        });
        
        ui.separator();
        
        // Logs display
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for log_entry in logs.iter().rev().take(100) { // Show last 100 logs
                    ui.horizontal(|ui| {
                        // Timestamp
                        let timestamp = log_entry.timestamp.format("%H:%M:%S%.3f");
                        ui.label(format!("[{}]", timestamp));
                        
                        // Log level with color
                        let (level_text, level_color) = match log_entry.level {
                            LogLevel::Info => ("INFO", Color32::from_rgb(23, 162, 184)),
                            LogLevel::Warning => ("WARN", Color32::from_rgb(255, 193, 7)),
                            LogLevel::Error => ("ERROR", Color32::from_rgb(220, 53, 69)),
                            LogLevel::Debug => ("DEBUG", Color32::from_rgb(108, 117, 125)),
                        };
                        ui.colored_label(level_color, level_text);
                        
                        // Message
                        ui.label(&log_entry.message);
                    });
                }
                
                if logs.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("No logs yet");
                    });
                }
            });
    });
}
