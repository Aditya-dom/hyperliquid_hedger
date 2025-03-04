use crate::{model::hl_msgs::TobMsg, utils::ws_utils::{ConnectionTimers, HypeStreamRequest, L2BookSubscription, SubscriptionType, WSState, WebSocketError}};
use futures::StreamExt;
use tokio::{sync::mpsc, time::{sleep, Instant}};
use tracing::{error, info, warn};
use yawc::frame::{FrameView, OpCode};
use std::borrow::Cow;
use std::time::Duration;

use super::ws_client::WebsocketClient;


pub struct HypeClient<'a, 's> {
    pub ws: WebsocketClient<'a>,
    pub msg_tx: mpsc::Sender<TobMsg>,
    pub timers: ConnectionTimers,
    pub client_no: u64,
    pub symbol: &'s str,
}

impl<'a, 's> HypeClient<'a, 's,> {
    pub async fn new(url: &'a str, symbol: &'s str, msg_tx: mpsc::Sender<TobMsg>, client_no: u64) -> anyhow::Result<Self>{
        let ws = WebsocketClient::new(url).await?;
        let timers = ConnectionTimers::default();
        Ok(Self {ws, msg_tx, timers, client_no, symbol})
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
        self.ws.send(HypeClient::subscribe_payload("l2Book", self.symbol)).await?;
        Ok(())
    }

    pub async fn handle_msg(&mut self, frame: FrameView) -> anyhow::Result<WSState> {
        match frame.opcode {
            OpCode::Text => {
                        if let Ok(text) = std::str::from_utf8(&frame.payload) {
                            // debug!("Raw WS message: {}", text);
                            if text.contains(r#""channel":"pong""#) {
                                info!("Received pong from HyperLiquid");
                                self.timers.last_alert = Instant::now();
                                return Ok(WSState::Continue);
                            }
                            if let Ok(tob_msg) = serde_json::from_str::<TobMsg>(text) {
                                if let Err(e) = self.msg_tx.send(tob_msg).await {
                                    warn!("Failed to send message to manager: {}", e);
                                }
                                return Ok(WSState::Continue);
                            }
                            warn!("Received unrecognized text message: {}", text);
                            return Ok(WSState::Continue);
                        }
                        warn!("Received invalid UTF-8 in text frame");
                        return Ok(WSState::Continue);
                    }
           OpCode::Binary | OpCode::Continuation | OpCode::Ping | OpCode::Pong  => {
                        return Ok(WSState::Continue);
                    }
            OpCode::Close => {
                        warn!("Received close frame from server, client={}", self.client_no);
                        return Ok(WSState::Closed);
                    }
        }
    }

    pub async fn consume(&mut self) -> anyhow::Result<(), WebSocketError> {
        loop {
            tokio::select! {
                frame = self.ws.client.next() => {
                    match frame {
                        Some(frame) => {
                            match self.handle_msg(frame).await? {
                                WSState::Continue => continue,
                                WSState::Closed => return Ok(()),
                                WSState::Err(e) => return Err(WebSocketError::Error(e)),
                            }
                        },
                        _ => {
                            continue;
                        }
                    }
                },
                _ = tokio::signal::ctrl_c() => {
                    return Err(WebSocketError::Terminated);
                }
                
                _ = self.timers.ping_timer.tick() => {
                    if let Err(e) = self.ws.send_ping().await {
                        error!("Failed to send ping: {}", e);
                        return Err(WebSocketError::Error(e));
                    }
                },

                _ = self.timers.stale_timer.tick() => {
                    let elapsed = self.timers.last_alert.elapsed();
                    if elapsed > Duration::from_secs(70) {
                        return Err(WebSocketError::Timeout);
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
            if matches!(
                self.consume().await,
                Err(WebSocketError::Terminated)
            ) {
                break;
            }
            sleep(Duration::from_millis(50)).await;
            self.reconnect().await?;
        }
        Ok(())
    }
}