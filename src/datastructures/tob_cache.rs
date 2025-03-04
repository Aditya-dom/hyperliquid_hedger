use crate::model::hl_msgs::TobMsg;

pub struct TobCache {
}

impl TobCache {
    pub fn new() -> Self{
        Self {}
    }

    pub fn update(&self, message_id: String, msg: TobMsg) -> TobCacheResult{
        TobCacheResult::Added
    }
    pub fn evict(&self){}
}


pub enum TobCacheResult {
    Added,
    Duplicate,
    AddedWithEviction(String),  
}