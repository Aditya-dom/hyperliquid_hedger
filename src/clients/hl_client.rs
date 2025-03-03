use crate::{model::hl_msgs::TobMsg, utils::ws_utils::{ConnectionTimers, HypeStreamRequest, L2BookSubscription, SubscriptionType, WSState}};
use tokio::{sync::mpsc, time::Instant};
use tracing::{error, info};
use std::borrow::Cow;

use super::ws_client::WebsocketClient;


pub struct HypeClient<'a> {
    pub ws: WebsocketClient<'a>,
    pub msg_tx: mpsc::Sender<TobMsg>,
    pub timers: ConnectionTimers,
    pub client_no: u64
}

impl<'a> HypeClient<'a> {
    pub async fn new(url: &'a str, msg_tx: mpsc::Sender<TobMsg>, client_no: u64) -> anyhow::Result<Self>{
        let ws = WebsocketClient::new(url).await?;
        let timers = ConnectionTimers::default();
        Ok(Self {ws, msg_tx, timers, client_no})
    }

    pub fn subscribe_payload<'h>(type_field: &'h str, coin: &'h str) -> HypeStreamRequest<'h> {
        // could use pattern matching for subscription type to make it more extendable
        HypeStreamRequest {
            method: "subscribe",
            subscription: SubscriptionType::L2Book(L2BookSubscription {
                type_field: Cow::Borrowed(type_field), 
                coin: Cow::Borrowed(coin)
            })
        }
    }

    pub async fn subscribe(&mut self) -> anyhow::Result<()> {
        self.ws.send(HypeClient::subscribe_payload("l2Book", "BTC-USD")).await?;
        Ok(())
    }

    pub async fn handle_msg(&mut self, frame: yawc::frame::FrameView) -> anyhow::Result<WSState> {
        match frame.opcode() {
            yawc::frame::OpCode::Text => {
                if let Ok(text) = std::str::from_utf8(frame.payload()) {
                    if text.contains(r#""channel":"pong""#) {
                        println!("Received pong from HyperLiquid");
                        self.timers.last_alert = Instant::now();
                        return Ok(WSState::Continue);
                    }
                    
                    if let Ok(tob_msg) = serde_json::from_str::<TobMsg>(text) {
                        if let Err(e) = self.msg_tx.send(tob_msg).await {
                            eprintln!("Failed to send message to manager: {}", e);
                        }
                        return Ok(WSState::Continue);
                    }
                    
                    println!("Received unrecognized text message: {}", text);
                    return Ok(WSState::Continue);
                }
                eprintln!("Received invalid UTF-8 in text frame");
                return Ok(WSState::Continue);
            }

            yawc::frame::OpCode::Binary => {
                println!("Received binary data of {} bytes", frame.payload().len());
                return Ok(WSState::Continue);
            }
            yawc::frame::OpCode::Close => {
                println!("Received close frame from server");
                return Ok(WSState::Closed);
            }
            _ => {
                println!("Received unsupported frame type");
                return Ok(WSState::Continue);
            }
        }
    }
    pub async fn consume(&mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                // Process incoming WebSocket messages
                frame = self.ws.client.next() => {
                    match frame {
                        Some(Ok(frame)) => {
                            match self.handle_msg(frame).await? {
                                WSState::Continue => continue,
                                WSState::Closed => return Ok(()),
                                WSState::Err(e) => return Err(e),
                            }
                        },
                        Some(Err(e)) => {
                            eprintln!("WebSocket error: {}", e);
                            return Err(anyhow::anyhow!("WebSocket error: {}", e));
                        },
                        None => {
                            println!("WebSocket connection closed by server");
                            return Ok(());
                        }
                    }
                },
                
                // Send heartbeat ping at regular intervals
                _ = self.timers.ping_timer.tick() => {
                    if let Err(e) = self.send_ping().await {
                        eprintln!("Failed to send ping: {}", e);
                        return Err(e);
                    }
                },
                
                // Check for connection health
                _ = self.timers.no_message_timeout.tick() => {
                    // If we haven't received a pong in too long, reconnect
                    let elapsed = self.timers.last_pong.elapsed();
                    if elapsed > self.timers.max_idle_time {
                        return Err(anyhow::anyhow!("Connection timed out - no pong received in {:?}", elapsed));
                    }
                }
            }
        }
    }
    
    pub async fn reconnect(&mut self) -> anyhow::Result<()> {
        info!("Attempting to reconnect to HyperLiquid, client={}", self.client_no);
        let _ = self.ws.close().await;
        self.ws = WebsocketClient::new(self.ws.url).await?;
        self.timers = ConnectionTimers::default();
        self.subscribe().await?;
        info!("Successfully reconnected to HyperLiquid, client={}", self.client_no);
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Starting HyperLiquid client: {}", self.client_no);
        self.subscribe().await?;
        info!("Client: {}, connected to HyperLiquid ", self.client_no);
        
        loop {
            if let Err(e) = self.consume().await {
                error!("Connection error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                
                match self.reconnect().await {
                    Ok(_) => println!("Reconnection successful"),
                    Err(e) => {
                        error!("Failed to reconnect: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            } else {
                info!("Connection closed gracefully, reconnecting...");
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                if let Err(e) = self.reconnect().await {
                    error!("Failed to reconnect: {}", e);
                }
            }
        }
    }
}