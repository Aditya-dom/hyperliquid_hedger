
use tokio::{sync::mpsc::{Receiver, Sender}, task::JoinSet};
use tracing::{error, info, warn};
use crate::{datastructures::tob_cache::{TobCache, TobCacheResult}, model::hl_msgs::TobMsg};
use super::hl_client::HypeClient;

pub struct WsManager<'a, 's>{
    pub clients: Vec<HypeClient<'a, 's>>,
    pub msg_rx : Receiver<TobMsg>,
    pub tob_cache: TobCache,
}

impl<'a, 's> WsManager<'a, 's> {
    pub async fn new(no_streams: u64, url : &'a str, symbol: &'s str, msg_tx: Sender<TobMsg>, msg_rx: Receiver<TobMsg>) -> anyhow::Result<Self>{
        let mut clients: Vec<HypeClient> = Vec::new();
        for client_no in 0..no_streams {
            let client = HypeClient::new(url, symbol, msg_tx.clone(), client_no).await?;
            clients.push(client)
        }

        let tob_cache = TobCache::new();

        Ok( Self {
            clients,
            msg_rx,
            tob_cache,
        })
    }


    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Starting ws_manager with {} redundant connections", self.clients.len());
    
        let mut client_tasks = JoinSet::new();
    
        for client_index in 0..self.clients.len() {
            let mut client = std::mem::replace(&mut self.clients[client_index], 
                                             unsafe { std::mem::zeroed() }); 
            
            client_tasks.spawn(async move {
                let result = client.run().await;
                (client_index, client, result)
            });
        }
        self.clients.clear();
        
        tokio::spawn(async move {
            self.process_messages().await;
        });
        
        while let Some(result) = client_tasks.join_next().await {
            match result {
                Ok((index, client, Ok(()))) => {
                    warn!("Client {} completed unexpectedly", index);
                    // Store the client back in case we want to restart it
                    if index >= self.clients.len() {
                        self.clients.resize_with(index + 1, || unsafe { std::mem::zeroed() });
                    }
                    self.clients[index] = client;
                },
                Ok((index, _client, Err(e))) => 
                    error!("Client {} failed with error: {}", index, e),
                }
                Err(e) => {
                    error!("Client task join error: {}", e);
                }
            }
            info!("All WebSocket clients have stopped");
            Ok(())
        }
    

    async fn process_messages(&mut self) {
        info!("Message processor started");
        
        while let Some(msg) = self.msg_rx.recv().await {
            let message_id = msg.generate_id();
            match self.tob_cache.update(message_id.clone(), msg.clone()) {
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
        }
        
        info!("Message processor shutting down");
    }
    
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Stopping ws_manager");
        
        for (i, client) in self.clients.iter_mut().enumerate() {
            info!("Closing client {}", i);
            if let Err(e) = client.ws.close().await {
                warn!("Error closing client {}: {}", i, e);
            }
        }

        info!("All clients closed");
        Ok(())
    }
}
