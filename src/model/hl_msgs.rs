use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TobMsg {
    pub channel: String,
    pub data: OrderBookData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookData {
    pub coin: String,
    pub time: u64,
    pub levels: Vec<Vec<PriceLevel>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PriceLevel {
    pub px: String, 
    pub sz: String, 
    pub n: u32,  
}


impl OrderBookData {
    pub fn top_of_book(&self) -> Option<(PriceLevel, PriceLevel)> {
        let best_bid = self.levels.get(0)?.get(0)?;
        let best_ask = self.levels.get(1)?.get(0)?;

        Some((best_bid.clone(), best_ask.clone()))
    }

    pub fn generate_id(&self) -> String {
        let tob_string = format!("{:?}", self.top_of_book());
        self.time.to_string() + &tob_string
    }
}
