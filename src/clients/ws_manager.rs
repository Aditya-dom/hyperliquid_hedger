use std::sync::Arc;
use tokio::task::JoinSet;
use parking_lot::Mutex;
use tracing::{error, info, warn};
use crate::{datastructures::tob_cache::{TobCache, TobCacheResult}, model::hl_msgs::TobMsg};
use super::hl_client::HypeClient;

pub struct WsManager {
    pub clients: Vec<Option<HypeClient>>,  
    pub msg_rx: Option<tokio::sync::mpsc::Receiver<TobMsg>>,  
    pub tob_cache: Arc<parking_lot::Mutex<TobCache>>,  
}

impl WsManager {
    pub async fn new(no_streams: u64, url: &str, symbol: &str, msg_tx: tokio::sync::mpsc::Sender<TobMsg>, 
                    msg_rx: tokio::sync::mpsc::Receiver<TobMsg>) -> anyhow::Result<Self> {
        
        let mut clients = Vec::with_capacity(no_streams as usize);
        for client_no in 0..no_streams {
            let client = HypeClient::new(url, symbol, msg_tx.clone(), client_no).await?;
            clients.push(Some(client));
        }

        let tob_cache = Arc::new(parking_lot::Mutex::new(TobCache::new()));

        Ok(Self {
            clients,
            msg_rx: Some(msg_rx),
            tob_cache,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Starting ws_manager with {} redundant connections", self.clients.len());
    
        let mut client_tasks = JoinSet::new();
    
        for client_index in 0..self.clients.len() {
            if let Some(mut client) = self.clients[client_index].take() {
                let client_index = client_index; // Create a copy for the closure
                
                client_tasks.spawn(async move {
                    let result = client.run().await;
                    (client_index, client, result)
                });
            }
        }
        
        let msg_rx = self.msg_rx.take()
            .expect("Message receiver was already taken");
        let tob_cache = self.tob_cache.clone();
        
        tokio::spawn(async move {
            process_messages(msg_rx, tob_cache).await;
        });
        
        while let Some(result) = client_tasks.join_next().await {
            match result {
                Ok((index, client, Ok(()))) => {
                    info!("Client {} completed - shutdown received", index);
                    if index >= self.clients.len() {
                        self.clients.resize_with(index + 1, || None);
                    }
                    
                    self.clients[index] = Some(client);
                },
                Ok((index, _client, Err(e))) => 
                    error!("Client {} failed with error: {}", index, e),
                Err(e) => {
                    error!("Client task join failed with error: {}", e);
                }
            }
        }
        
        info!("All hype clients have stopped");
        Ok(())
    }
    
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping ws_manager");
        for (i, client_opt) in self.clients.iter_mut().enumerate() {
            if let Some(client) = client_opt {
                info!("Closing client {}", i);
                if let Err(e) = client.ws.close().await {
                    warn!("Error closing client {}: {}", i, e);
                }
            }
        }

        info!("All clients closed");
        Ok(())
    }
}

async fn process_messages(mut msg_rx: tokio::sync::mpsc::Receiver<TobMsg>, tob_cache: Arc<Mutex<TobCache>>) {
    info!("Message processor started");
    
    loop {
        let msg = match msg_rx.recv().await {
            Some(msg) => msg,
            _ => {
                info!("Message channel closed, processor shutting down");
                break;
            }
        };
        
        if let Err(e) = process_single_message(&msg, &tob_cache).await {
            error!("Error processing message: {}", e);
            continue;
        }
    }
    
    info!("Message processor has shut down");
}

async fn process_single_message(msg: &TobMsg, tob_cache: &Arc<Mutex<TobCache>>) -> anyhow::Result<()> {
    let message_id = msg.data.generate_id();
    
    let tob = match  msg.data.top_of_book() {
        Some(tob) => tob,
        _ => {
            return Ok(());
        }
    };
    
    let update_result = {
        let mut guard = tob_cache.lock();
        guard.update(message_id.clone(), tob)
    };
    
    match update_result {
        TobCacheResult::Added => {
            if let Some(top) = msg.data.top_of_book() {
                info!("Latest added top of book - Bid: {} @ {}, Ask: {} @ {}", 
                      top.0.px, top.0.sz, top.1.px, top.1.sz);
            }
        },
        TobCacheResult::Duplicate => {
            info!("Duplicate message detected: {}", message_id);
        },
        TobCacheResult::AddedWithEviction(evicted_id) => {
            info!("Evicted message: {}", evicted_id);
            info!("Added new top-of-book state: {}", message_id);
            if let Some(top) = msg.data.top_of_book() {
                info!("Latest top of book - Bid: {} @ {}, Ask: {} @ {}", 
                    top.0.px, top.0.sz, top.1.px, top.1.sz);
            }
        }
    }
    
    Ok(())
}