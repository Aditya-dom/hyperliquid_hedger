use crate::events::types::*;
use crossbeam_channel::{Sender, Receiver, bounded, unbounded};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info, warn, debug};

pub struct EventBus {
    // High-priority channel for critical events (risk, errors)
    high_priority_tx: Sender<SystemEvent>,
    high_priority_rx: Receiver<SystemEvent>,
    
    // Normal priority channel for regular events
    normal_priority_tx: Sender<SystemEvent>,
    normal_priority_rx: Receiver<SystemEvent>,
    
    // Low priority channel for market data (high volume)
    low_priority_tx: Sender<SystemEvent>,
    low_priority_rx: Receiver<SystemEvent>,
    
    // Subscriber management
    subscribers: Arc<DashMap<String, Vec<Sender<SystemEvent>>>>,
    
    // Event filtering
    filters: Arc<RwLock<Vec<Box<dyn EventFilter + Send + Sync>>>>,
    
    // Performance metrics
    events_processed: Arc<AtomicU64>,
    events_dropped: Arc<AtomicU64>,
    
    // Configuration
    config: EventBusConfig,
}

#[derive(Debug, Clone)]
pub struct EventBusConfig {
    pub high_priority_buffer_size: usize,
    pub normal_priority_buffer_size: usize,
    pub market_data_buffer_size: usize,
    pub max_subscribers_per_topic: usize,
    pub enable_metrics: bool,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            high_priority_buffer_size: 1000,
            normal_priority_buffer_size: 10000,
            market_data_buffer_size: 100000, // High volume for market data
            max_subscribers_per_topic: 100,
            enable_metrics: true,
            batch_size: 100,
            batch_timeout_ms: 10,
        }
    }
}

pub trait EventFilter {
    fn should_process(&self, event: &SystemEvent) -> bool;
    fn name(&self) -> &str;
}

pub struct TopicFilter {
    pub name: String,
    pub allowed_topics: Vec<String>,
}

impl EventFilter for TopicFilter {
    fn should_process(&self, event: &SystemEvent) -> bool {
        let topic = match event {
            SystemEvent::MarketData { symbol, .. } => format!("market_data.{}", symbol),
            SystemEvent::Order(_) => "orders".to_string(),
            SystemEvent::Position(_) => "positions".to_string(),
            SystemEvent::Strategy { strategy_name, .. } => format!("strategy.{}", strategy_name),
            SystemEvent::Connection { connection_id, .. } => format!("connection.{}", connection_id),
            SystemEvent::Risk { symbol, .. } => format!("risk.{}", symbol),
            SystemEvent::System { .. } => "system".to_string(),
        };
        
        self.allowed_topics.iter().any(|allowed| {
            topic.starts_with(allowed) || allowed == "*"
        })
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

impl EventBus {
    pub fn new(config: EventBusConfig) -> Self {
        let (high_priority_tx, high_priority_rx) = bounded(config.high_priority_buffer_size);
        let (normal_priority_tx, normal_priority_rx) = bounded(config.normal_priority_buffer_size);
        let (low_priority_tx, low_priority_rx) = bounded(config.market_data_buffer_size);
        
        Self {
            high_priority_tx,
            high_priority_rx,
            normal_priority_tx,
            normal_priority_rx,
            low_priority_tx,
            low_priority_rx,
            subscribers: Arc::new(DashMap::new()),
            filters: Arc::new(RwLock::new(Vec::new())),
            events_processed: Arc::new(AtomicU64::new(0)),
            events_dropped: Arc::new(AtomicU64::new(0)),
            config,
        }
    }
    
    pub fn publish(&self, event: SystemEvent) -> Result<(), String> {
        // Apply filters
        {
            let filters = self.filters.read();
            for filter in filters.iter() {
                if !filter.should_process(&event) {
                    debug!("Event filtered by {}: {:?}", filter.name(), event);
                    return Ok(());
                }
            }
        }
        
        // Route to appropriate priority channel
        let result = match event.priority() {
            EventPriority::Critical | EventPriority::High => {
                self.high_priority_tx.try_send(event)
            }
            EventPriority::Normal => {
                self.normal_priority_tx.try_send(event)
            }
            EventPriority::Low => {
                self.low_priority_tx.try_send(event)
            }
        };
        
        match result {
            Ok(_) => {
                if self.config.enable_metrics {
                    self.events_processed.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            }
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                if self.config.enable_metrics {
                    self.events_dropped.fetch_add(1, Ordering::Relaxed);
                }
                warn!("Event bus channel full, dropping event");
                Err("Event bus channel full".to_string())
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                error!("Event bus disconnected");
                Err("Event bus disconnected".to_string())
            }
        }
    }
    
    pub fn subscribe(&self, topic: &str) -> Receiver<SystemEvent> {
        let (tx, rx) = unbounded();
        
        let mut subscribers = self.subscribers.entry(topic.to_string()).or_insert_with(Vec::new);
        
        if subscribers.len() >= self.config.max_subscribers_per_topic {
            warn!("Max subscribers reached for topic: {}", topic);
            return rx; // Return empty receiver
        }
        
        subscribers.push(tx);
        info!("New subscriber for topic: {}", topic);
        rx
    }
    
    pub fn add_filter(&self, filter: Box<dyn EventFilter + Send + Sync>) {
        let mut filters = self.filters.write();
        filters.push(filter);
        info!("Added event filter");
    }
    
    pub fn start_processing(&self) {
        let subscribers = Arc::clone(&self.subscribers);
        let events_processed = Arc::clone(&self.events_processed);
        let batch_size = self.config.batch_size;
        let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);
        
        // High priority processor
        let high_rx = self.high_priority_rx.clone();
        let high_subscribers = Arc::clone(&subscribers);
        let high_events_processed = Arc::clone(&events_processed);
        
        thread::spawn(move || {
            info!("High priority event processor started");
            let mut batch = Vec::with_capacity(batch_size);
            let mut last_batch_time = Instant::now();
            
            loop {
                // Try to collect a batch of events
                while batch.len() < batch_size && last_batch_time.elapsed() < batch_timeout {
                    match high_rx.try_recv() {
                        Ok(event) => batch.push(event),
                        Err(crossbeam_channel::TryRecvError::Empty) => {
                            thread::sleep(Duration::from_micros(100));
                            break;
                        }
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            warn!("High priority channel disconnected");
                            return;
                        }
                    }
                }
                
                // Process batch if we have events or timeout
                if !batch.is_empty() || last_batch_time.elapsed() >= batch_timeout {
                    Self::process_event_batch(&batch, &high_subscribers);
                    high_events_processed.fetch_add(batch.len() as u64, Ordering::Relaxed);
                    batch.clear();
                    last_batch_time = Instant::now();
                }
                
                // If no events in batch, block for one event
                if batch.is_empty() {
                    match high_rx.recv_timeout(batch_timeout) {
                        Ok(event) => batch.push(event),
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            warn!("High priority channel disconnected");
                            return;
                        }
                    }
                }
            }
        });
        
        // Normal priority processor
        let normal_rx = self.normal_priority_rx.clone();
        let normal_subscribers = Arc::clone(&subscribers);
        let normal_events_processed = Arc::clone(&events_processed);
        
        thread::spawn(move || {
            info!("Normal priority event processor started");
            for event in normal_rx {
                Self::distribute_event(&event, &normal_subscribers);
                normal_events_processed.fetch_add(1, Ordering::Relaxed);
            }
            warn!("Normal priority event processor stopped");
        });
        
        // Low priority processor (market data)
        let low_rx = self.low_priority_rx.clone();
        let low_subscribers = Arc::clone(&subscribers);
        let low_events_processed = Arc::clone(&events_processed);
        
        thread::spawn(move || {
            info!("Low priority event processor started");
            let mut batch = Vec::with_capacity(batch_size * 2); // Larger batches for market data
            let mut last_batch_time = Instant::now();
            
            loop {
                // Collect larger batches for market data
                while batch.len() < batch_size * 2 && last_batch_time.elapsed() < batch_timeout {
                    match low_rx.try_recv() {
                        Ok(event) => batch.push(event),
                        Err(crossbeam_channel::TryRecvError::Empty) => break,
                        Err(crossbeam_channel::TryRecvError::Disconnected) => {
                            warn!("Low priority channel disconnected");
                            return;
                        }
                    }
                }
                
                if !batch.is_empty() {
                    Self::process_event_batch(&batch, &low_subscribers);
                    low_events_processed.fetch_add(batch.len() as u64, Ordering::Relaxed);
                    batch.clear();
                    last_batch_time = Instant::now();
                }
                
                // Wait for more events if batch is empty
                if batch.is_empty() {
                    match low_rx.recv_timeout(batch_timeout) {
                        Ok(event) => batch.push(event),
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            warn!("Low priority channel disconnected");
                            return;
                        }
                    }
                }
            }
        });
        
        info!("Event bus processing started");
    }
    
    fn distribute_event(event: &SystemEvent, subscribers: &DashMap<String, Vec<Sender<SystemEvent>>>) {
        let topics = Self::get_event_topics(event);
        
        for topic in &topics {
            if let Some(subs) = subscribers.get(topic) {
                let mut disconnected_indices = Vec::new();
                
                for (i, sender) in subs.iter().enumerate() {
                    match sender.try_send(event.clone()) {
                        Ok(_) => {},
                        Err(crossbeam_channel::TrySendError::Full(_)) => {
                            debug!("Subscriber channel full for topic: {}", topic);
                        },
                        Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                            disconnected_indices.push(i);
                        }
                    }
                }
                
                // Clean up disconnected subscribers
                if !disconnected_indices.is_empty() {
                    debug!("Cleaning up {} disconnected subscribers for topic: {}", 
                          disconnected_indices.len(), topic);
                }
            }
        }
    }
    
    fn process_event_batch(batch: &[SystemEvent], subscribers: &DashMap<String, Vec<Sender<SystemEvent>>>) {
        for event in batch {
            Self::distribute_event(event, subscribers);
        }
    }
    
    fn get_event_topics(event: &SystemEvent) -> Vec<String> {
        let mut topics = vec!["*".to_string()]; // Global topic
        
        match event {
            SystemEvent::MarketData { symbol, .. } => {
                topics.push("market_data".to_string());
                topics.push(format!("market_data.{}", symbol));
            },
            SystemEvent::Order(_) => {
                topics.push("orders".to_string());
            },
            SystemEvent::Position(_) => {
                topics.push("positions".to_string());
            },
            SystemEvent::Strategy { strategy_name, .. } => {
                topics.push("strategy".to_string());
                topics.push(format!("strategy.{}", strategy_name));
            },
            SystemEvent::Connection { connection_id, .. } => {
                topics.push("connection".to_string());
                topics.push(format!("connection.{}", connection_id));
            },
            SystemEvent::Risk { symbol, .. } => {
                topics.push("risk".to_string());
                topics.push(format!("risk.{}", symbol));
            },
            SystemEvent::System { .. } => {
                topics.push("system".to_string());
            },
        }
        
        topics
    }
    
    pub fn get_metrics(&self) -> EventBusMetrics {
        EventBusMetrics {
            events_processed: self.events_processed.load(Ordering::Relaxed),
            events_dropped: self.events_dropped.load(Ordering::Relaxed),
            subscriber_count: self.subscribers.len(),
            high_priority_queue_len: self.high_priority_rx.len(),
            normal_priority_queue_len: self.normal_priority_rx.len(),
            low_priority_queue_len: self.low_priority_rx.len(),
        }
    }
    
    pub fn get_publisher(&self) -> EventPublisher {
        EventPublisher {
            high_priority_tx: self.high_priority_tx.clone(),
            normal_priority_tx: self.normal_priority_tx.clone(),
            low_priority_tx: self.low_priority_tx.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventPublisher {
    high_priority_tx: Sender<SystemEvent>,
    normal_priority_tx: Sender<SystemEvent>,
    low_priority_tx: Sender<SystemEvent>,
}

impl EventPublisher {
    pub fn publish(&self, event: SystemEvent) -> Result<(), String> {
        let result = match event.priority() {
            EventPriority::Critical | EventPriority::High => {
                self.high_priority_tx.try_send(event)
            }
            EventPriority::Normal => {
                self.normal_priority_tx.try_send(event)
            }
            EventPriority::Low => {
                self.low_priority_tx.try_send(event)
            }
        };
        
        match result {
            Ok(_) => Ok(()),
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                Err("Channel full".to_string())
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                Err("Channel disconnected".to_string())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventBusMetrics {
    pub events_processed: u64,
    pub events_dropped: u64,
    pub subscriber_count: usize,
    pub high_priority_queue_len: usize,
    pub normal_priority_queue_len: usize,
    pub low_priority_queue_len: usize,
}
