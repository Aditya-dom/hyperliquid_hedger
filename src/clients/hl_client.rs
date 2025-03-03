use crate::{model::hl_msgs::TobMsg, utils::ws_utils::ConnectionTimers};
use tokio::sync::mpsc;

use super::ws_client::WebsocketClient;


pub struct HypeClient<'a> {
    pub ws: WebsocketClient<'a>,
    pub msg_tx: mpsc::Sender<TobMsg>,
    pub timers: ConnectionTimers,
}

impl<'a> HypeClient<'a> {
    pub async fn new(url: &'a str, msg_tx: mpsc::Sender<TobMsg> ) -> anyhow::Result<Self>{
        let ws = WebsocketClient::new(url).await?;
        let timers = ConnectionTimers::default();
        Ok(Self {ws, msg_tx, timers})
    }

    pub async fn subscribe_payload(){}
    pub async fn subscribe() {}
    pub async fn handle_msg(){}
    pub async fn consume() {}
    pub async fn reconnect(){}
    pub async fn run() {}
}