use crate::model::hl_msgs::PriceLevel;
use std::collections::HashMap;
use std::collections::VecDeque;

pub struct TobCache {
    mp: HashMap<String, (PriceLevel, PriceLevel)>,
    tobs: VecDeque<String>,
    capacity: usize,
}

impl TobCache {
    pub fn new() -> Self {
        let mp: HashMap<String, (PriceLevel, PriceLevel)> = HashMap::new();
        let tobs: VecDeque<String> = VecDeque::with_capacity(100);
        Self {
            mp,
            tobs,
            capacity: 100,
        }
    }
    
    pub fn update(&mut self, message_id: String, tob: (PriceLevel, PriceLevel)) -> TobCacheResult {
        if self.mp.contains_key(&message_id) {
            return TobCacheResult::Duplicate;
        }
                
        if self.tobs.len() >= self.capacity {
            return self.evict_and_add(message_id, tob);
        }
        self.mp.insert(message_id.clone(), tob);
        self.tobs.push_back(message_id);
        TobCacheResult::Added
    }
    
    fn evict_and_add(&mut self, message_id: String, levels: (PriceLevel, PriceLevel)) -> TobCacheResult {
        
        let evicted_id = self.tobs.pop_front().unwrap();  
        self.mp.remove(&evicted_id);
        
        self.mp.insert(message_id.clone(), levels);
        self.tobs.push_back(message_id);
        
        TobCacheResult::AddedWithEviction(evicted_id)
    }
    
    
    pub fn get(&self, message_id: &str) -> Option<&(PriceLevel, PriceLevel)> {
        self.mp.get(message_id)
    }
    
    pub fn len(&self) -> usize {
        self.tobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tobs.is_empty()
    }
}

pub enum TobCacheResult {
    Added,
    Duplicate,
    AddedWithEviction(String),
}