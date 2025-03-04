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
    
    info!("WebSocket manager initialized with {} connections", redundant_connections);
    
    let shutdown_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Shutdown signal received");
    });
    
    tokio::select! {
        result = ws_manager.run() => {
            match result {
                Ok(_) => info!("WebSocket manager completed normally"),
                Err(e) => error!("WebSocket manager error: {}", e),
            }
        }
        _ = shutdown_handle => {
            info!("Initiating graceful shutdown sequence");
            if let Err(e) = ws_manager.stop().await {
                error!("Error during WebSocket manager shutdown: {}", e);
            } else {
                info!("WebSocket manager stopped successfully");
            }
        }
    }
    
    info!("Application shutdown complete");
    Ok(())
}

// 1. Connect to Hyperliquid and pull (only) the top of book from the orderbook feed.
// 2. Add support for multiple websocket connections to receive the same data, simulating redundant data streams.
// 3. Develop a data structure that allows discarding repeated messages. Limit the data structure to not hold more than 100 items. If, for example, you are using a HashSet, ensure that no more than 100 elements are being stored.
// 4. Use [yawc](https://docs.rs/yawc) for handling websocket connections.
// 5. Print out the latest top of book and log every eviction and addition from the data structure.

// ### Notes
// - Code can be written using tokio tasks or single-thread tasks (aka polling the streams).
// - The data structure should support continuous real-time updates.
// - The program should run indefinitely, maintaining the latest data while discarding duplicates and limiting the total number of stored items