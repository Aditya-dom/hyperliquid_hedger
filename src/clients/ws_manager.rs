
use tokio::sync::{mpsc::Sender, mpsc::Receiver};
use crate::{datastructures::tob_cache::TobCache, model::hl_msgs::TobMsg};
use super::hl_client::HypeClient;

pub struct WsManager<'a, 's>{
    pub clients: Vec<HypeClient<'a, 's>>,
    pub msg_rx : Receiver<TobMsg>,
    pub top_cache: TobCache,
}

impl<'a, 's> WsManager<'a, 's> {
    pub async fn new(no_streams: u64, url : &'a str, symbol: &'s str, msg_tx :  Sender<TobMsg>) -> anyhow::Result<()>{
        let mut client_vec: Vec<HypeClient> = Vec::new();
        for client_no in 0..no_streams {
            let client = HypeClient::new(url, symbol, msg_tx.clone(), client_no).await?;
            client_vec.push(client)
        }
        Ok(())
    }
}