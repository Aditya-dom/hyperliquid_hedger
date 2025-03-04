use if_takehome_rs::{clients::hl_client::HypeClient, model::hl_msgs::TobMsg};
use tokio::sync::mpsc;
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Starting HyperLiquid orderbook client");
    
    // Channels
    let (msg_tx, mut msg_rx) = mpsc::channel::<TobMsg>(1000);
    
    let client_url = "wss://api.hyperliquid.xyz/ws";
    let symbol = "HYPE"; 
    let client_no = 1; 

    let client_handle = tokio::spawn(async move {
        match HypeClient::new(client_url, symbol, msg_tx, client_no).await {
            Ok(mut client) => {
                if let Err(e) = client.run().await {
                    error!("Client error: {}", e);
                }
            },
            Err(e) => {
                error!("Failed to create client: {}", e);
            }
        }
    });
    
    let processor_handle = tokio::spawn(async move {
        info!("Message processor started");
        while let Some(msg) = msg_rx.recv().await {
            info!("Received top-of-book message: {:?}", msg);
        }
    });
    
    tokio::select! {
        _ = client_handle => {
            info!("Client task completed unexpectedly");
        }
        _ = processor_handle => {
            info!("Processor task completed unexpectedly");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal, stopping client");
        }
    }
    
    info!("Application shutting down");
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