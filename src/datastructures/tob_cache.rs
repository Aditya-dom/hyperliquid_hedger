pub struct TobCache {
}

impl TobCache {
    pub fn new() -> Self{
        Self {}
    }
}


pub enum TobCacheResult {
    Added,
    Duplicate,
    AddedWithEviction(String),  
}