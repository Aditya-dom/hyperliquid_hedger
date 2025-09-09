use crate::api::types::ApiConfig;
use crate::strategies::market_making::MarketMakingConfig;
use crate::trading::types::RiskLimits;
use anyhow::Result;
use crossbeam_channel::{Sender, Receiver, unbounded};
use dashmap::DashMap;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tracing::{error, info, warn, debug};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub bot_id: String,
    pub version: String,
    pub environment: Environment,
    pub api_config: ApiConfig,
    pub strategies: HashMap<String, StrategyConfig>,
    pub risk_config: RiskConfig,
    pub ui_config: UiConfig,
    pub logging_config: LoggingConfig,
    pub performance_config: PerformanceConfig,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub name: String,
    pub enabled: bool,
    pub symbol: String,
    pub strategy_type: StrategyType,
    pub config: serde_json::Value,
    pub risk_limits: RiskLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyType {
    MarketMaking,
    Arbitrage,
    Momentum,
    MeanReversion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub global_risk_limits: RiskLimits,
    pub position_limits: HashMap<String, PositionLimitConfig>,
    pub exposure_limits: HashMap<String, ExposureLimitConfig>,
    pub volatility_limits: HashMap<String, VolatilityLimitConfig>,
    pub circuit_breakers: Vec<CircuitBreakerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionLimitConfig {
    pub max_long: Decimal,
    pub max_short: Decimal,
    pub max_net: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureLimitConfig {
    pub max_notional: Decimal,
    pub max_leverage: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityLimitConfig {
    pub max_spread_bps: u32,
    pub max_price_change_bps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub id: String,
    pub symbol: String,
    pub trigger_type: String,
    pub threshold: Decimal,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub window_width: f32,
    pub window_height: f32,
    pub theme: String,
    pub refresh_rate_ms: u64,
    pub show_debug_info: bool,
    pub panels: HashMap<String, PanelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub enabled: bool,
    pub position: (f32, f32),
    pub size: (f32, f32),
    pub settings: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: Option<String>,
    pub max_file_size_mb: u64,
    pub max_files: u32,
    pub enable_console: bool,
    pub enable_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_memory_mb: u64,
    pub gc_interval_seconds: u64,
    pub metrics_interval_seconds: u64,
    pub enable_profiling: bool,
    pub thread_pool_size: usize,
}

pub struct ConfigManager {
    pub config: Arc<RwLock<BotConfig>>,
    pub config_events_tx: Sender<ConfigEvent>,
    pub watchers: Arc<DashMap<String, ConfigWatcher>>,
    pub file_path: Option<String>,
    pub auto_save: bool,
    pub save_interval: Duration,
}

#[derive(Debug, Clone)]
pub enum ConfigEvent {
    ConfigLoaded {
        config_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ConfigSaved {
        config_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ConfigChanged {
        section: String,
        key: String,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ConfigError {
        error: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

#[derive(Debug, Clone)]
pub struct ConfigWatcher {
    pub id: String,
    pub section: String,
    pub callback: String, // In a real implementation, this would be a function pointer
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            bot_id: Uuid::new_v4().to_string(),
            version: "1.0.0".to_string(),
            environment: Environment::Development,
            api_config: ApiConfig::default(),
            strategies: HashMap::new(),
            risk_config: RiskConfig::default(),
            ui_config: UiConfig::default(),
            logging_config: LoggingConfig::default(),
            performance_config: PerformanceConfig::default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            global_risk_limits: RiskLimits::default(),
            position_limits: HashMap::new(),
            exposure_limits: HashMap::new(),
            volatility_limits: HashMap::new(),
            circuit_breakers: Vec::new(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            window_width: 1200.0,
            window_height: 800.0,
            theme: "dark".to_string(),
            refresh_rate_ms: 100,
            show_debug_info: false,
            panels: HashMap::new(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_path: Some("logs/bot.log".to_string()),
            max_file_size_mb: 100,
            max_files: 10,
            enable_console: true,
            enable_file: true,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 1024,
            gc_interval_seconds: 300,
            metrics_interval_seconds: 60,
            enable_profiling: false,
            thread_pool_size: 4,
        }
    }
}

impl ConfigManager {
    pub fn new() -> (Self, Receiver<ConfigEvent>) {
        let (tx, rx) = unbounded();
        
        let manager = Self {
            config: Arc::new(RwLock::new(BotConfig::default())),
            config_events_tx: tx,
            watchers: Arc::new(DashMap::new()),
            file_path: None,
            auto_save: true,
            save_interval: Duration::from_secs(30),
        };
        
        (manager, rx)
    }

    pub fn with_file_path(mut self, path: String) -> Self {
        self.file_path = Some(path);
        self
    }

    pub fn with_auto_save(mut self, auto_save: bool, interval: Duration) -> Self {
        self.auto_save = auto_save;
        self.save_interval = interval;
        self
    }

    pub async fn load_from_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        if !path.as_ref().exists() {
            return Err(format!("Config file does not exist: {}", path_str));
        }

        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        let config: BotConfig = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;

        {
            let mut current_config = self.config.write();
            *current_config = config;
        }

        let _ = self.config_events_tx.send(ConfigEvent::ConfigLoaded {
            config_id: self.get_config_id(),
            timestamp: chrono::Utc::now(),
        });

        info!("Config loaded from file: {}", path_str);
        Ok(())
    }

    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        let config = self.config.read().clone();
        let content = toml::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(path.as_ref(), content)
            .map_err(|e| format!("Failed to write config file: {}", e))?;

        let _ = self.config_events_tx.send(ConfigEvent::ConfigSaved {
            config_id: self.get_config_id(),
            timestamp: chrono::Utc::now(),
        });

        info!("Config saved to file: {}", path_str);
        Ok(())
    }

    pub fn get_config(&self) -> BotConfig {
        self.config.read().clone()
    }

    pub fn update_config<F>(&self, updater: F) -> Result<(), String>
    where
        F: FnOnce(&mut BotConfig),
    {
        let mut config = self.config.write();
        updater(&mut config);
        config.updated_at = chrono::Utc::now();

        let _ = self.config_events_tx.send(ConfigEvent::ConfigChanged {
            section: "global".to_string(),
            key: "updated_at".to_string(),
            old_value: serde_json::Value::Null,
            new_value: serde_json::Value::String(config.updated_at.to_rfc3339()),
            timestamp: chrono::Utc::now(),
        });

        Ok(())
    }

    pub fn get_strategy_config(&self, name: &str) -> Option<StrategyConfig> {
        self.config.read().strategies.get(name).cloned()
    }

    pub fn update_strategy_config(&self, name: &str, config: StrategyConfig) -> Result<(), String> {
        let mut bot_config = self.config.write();
        let old_config = bot_config.strategies.get(name).cloned();
        bot_config.strategies.insert(name.to_string(), config.clone());
        bot_config.updated_at = chrono::Utc::now();

        let _ = self.config_events_tx.send(ConfigEvent::ConfigChanged {
            section: "strategies".to_string(),
            key: name.to_string(),
            old_value: old_config.map(|c| serde_json::to_value(c).unwrap_or_default()).unwrap_or_default(),
            new_value: serde_json::to_value(config).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        });

        Ok(())
    }

    pub fn get_risk_config(&self) -> RiskConfig {
        self.config.read().risk_config.clone()
    }

    pub fn update_risk_config(&self, config: RiskConfig) -> Result<(), String> {
        let mut bot_config = self.config.write();
        let old_config = bot_config.risk_config.clone();
        bot_config.risk_config = config.clone();
        bot_config.updated_at = chrono::Utc::now();

        let _ = self.config_events_tx.send(ConfigEvent::ConfigChanged {
            section: "risk".to_string(),
            key: "risk_config".to_string(),
            old_value: serde_json::to_value(old_config).unwrap_or_default(),
            new_value: serde_json::to_value(config).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        });

        Ok(())
    }

    pub fn get_api_config(&self) -> ApiConfig {
        self.config.read().api_config.clone()
    }

    pub fn update_api_config(&self, config: ApiConfig) -> Result<(), String> {
        let mut bot_config = self.config.write();
        let old_config = bot_config.api_config.clone();
        bot_config.api_config = config.clone();
        bot_config.updated_at = chrono::Utc::now();

        let _ = self.config_events_tx.send(ConfigEvent::ConfigChanged {
            section: "api".to_string(),
            key: "api_config".to_string(),
            old_value: serde_json::to_value(old_config).unwrap_or_default(),
            new_value: serde_json::to_value(config).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        });

        Ok(())
    }

    pub fn add_config_watcher(&self, watcher: ConfigWatcher) {
        self.watchers.insert(watcher.id.clone(), watcher);
        info!("Added config watcher");
    }

    pub fn remove_config_watcher(&self, id: &str) {
        self.watchers.remove(id);
        info!("Removed config watcher: {}", id);
    }

    pub fn get_config_id(&self) -> String {
        self.config.read().bot_id.clone()
    }

    pub fn start_auto_save(&self) {
        if !self.auto_save {
            return;
        }

        let config = Arc::clone(&self.config);
        let config_events_tx = self.config_events_tx.clone();
        let file_path = self.file_path.clone();
        let save_interval = self.save_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(save_interval);
            
            loop {
                interval.tick().await;
                
                if let Some(path) = &file_path {
                    match Self::save_config_to_file(&config, path).await {
                        Ok(_) => {
                            debug!("Auto-saved config to {}", path);
                        }
                        Err(e) => {
                            error!("Failed to auto-save config: {}", e);
                            let _ = config_events_tx.send(ConfigEvent::ConfigError {
                                error: format!("Auto-save failed: {}", e),
                                timestamp: chrono::Utc::now(),
                            });
                        }
                    }
                }
            }
        });
    }

    async fn save_config_to_file(config: &Arc<RwLock<BotConfig>>, path: &str) -> Result<(), String> {
        let config = config.read().clone();
        let content = toml::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write config file: {}", e))?;

        Ok(())
    }

    pub fn validate_config(&self) -> Result<(), String> {
        let config = self.config.read();
        
        // Validate API config
        if config.api_config.base_url.is_empty() {
            return Err("API base URL cannot be empty".to_string());
        }

        // Validate strategies
        for (name, strategy) in &config.strategies {
            if strategy.name.is_empty() {
                return Err(format!("Strategy name cannot be empty for strategy: {}", name));
            }
            if strategy.symbol.is_empty() {
                return Err(format!("Strategy symbol cannot be empty for strategy: {}", name));
            }
        }

        // Validate risk config
        if config.risk_config.global_risk_limits.max_position_size <= Decimal::ZERO {
            return Err("Global max position size must be positive".to_string());
        }

        if config.risk_config.global_risk_limits.max_daily_loss <= Decimal::ZERO {
            return Err("Global max daily loss must be positive".to_string());
        }

        Ok(())
    }

    pub fn create_default_market_making_strategy(&self, symbol: String) -> Result<(), String> {
        let strategy_config = StrategyConfig {
            name: format!("market_making_{}", symbol),
            enabled: true,
            symbol: symbol.clone(),
            strategy_type: StrategyType::MarketMaking,
            config: serde_json::to_value(MarketMakingConfig::default())
                .map_err(|e| format!("Failed to serialize market making config: {}", e))?,
            risk_limits: RiskLimits::default(),
        };

        let name = strategy_config.name.clone();
        self.update_strategy_config(&name, strategy_config)
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new().0
    }
}

impl Clone for ConfigManager {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            config_events_tx: self.config_events_tx.clone(),
            watchers: Arc::clone(&self.watchers),
            file_path: self.file_path.clone(),
            auto_save: self.auto_save,
            save_interval: self.save_interval,
        }
    }
}
