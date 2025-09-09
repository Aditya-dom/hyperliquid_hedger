use hyper_liquid_connector::ui::app::TradingApp;
use eframe::egui;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Initialize crypto provider
    rustls::crypto::ring::default_provider().install_default()
        .expect("Failed to install rustls crypto provider");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("HyperLiquid Market Making Bot"),
        ..Default::default()
    };

    eframe::run_native(
        "HyperLiquid Trading Bot",
        options,
        Box::new(|_cc| Ok(Box::new(TradingApp::new()))),
    )
}
