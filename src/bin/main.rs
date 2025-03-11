use if_takehome_rs::{
    clients::ws_manager::WsManager, 
    model::hl_msgs::TobMsg
};
use tokio::sync::mpsc;
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider().install_default()
        .expect("Failed to install rustls crypto provider");
    
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("Starting HyperLiquid App");
    
    let client_url = "wss://api.hyperliquid.xyz/ws";
    let symbol = "HYPE";
    let redundant_connections = 5;
    
    // Channels
    let (msg_tx, msg_rx) = mpsc::channel::<TobMsg>(1000);
    
    // Init ws_manager
    let mut ws_manager = WsManager::new(
        redundant_connections, 
        client_url, 
        symbol, 
        msg_tx, 
        msg_rx
    ).await?;
    
    info!("ws-mangager initialized with {} connections", redundant_connections);
    
    let shutdown_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Shutdown signal received");
    });
    
    tokio::select! {
        result = ws_manager.run() => {
            match result {
                Ok(_) => info!("ws-mangager completed normally"),
                Err(e) => error!("ws_manager error: {}", e),
            }
        }
        _ = shutdown_handle => {
            info!("Initiating shutdown!");
            if let Err(e) = ws_manager.stop().await {
                error!("Error during ws_manager shutdown: {}", e);
            } else {
                info!("ws_manager manager stopped successfully");
            }
        }
    }
    
    info!("Application shutdown complete");
    Ok(())
}
