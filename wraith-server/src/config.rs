//! Configuration for Wraith server.
//!
//! All configuration is read from environment variables.

use std::env;

/// Server configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// HTTP server host
    pub host: String,
    
    /// HTTP server port
    pub port: u16,
    
    /// NATS server URL
    pub nats_url: String,
    
    /// NATS subject for events
    pub nats_subject: String,
    
    /// ClickHouse URL
    pub clickhouse_url: String,
    
    /// ClickHouse database name
    pub clickhouse_database: String,
    
    /// ClickHouse table name
    pub clickhouse_table: String,
    
    /// Log level
    pub log_level: String,
    
    /// Enable JSON logging
    pub log_json: bool,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string()),
            nats_subject: env::var("NATS_SUBJECT").unwrap_or_else(|_| "wraith.events".to_string()),
            clickhouse_url: env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://localhost:8123".to_string()),
            clickhouse_database: env::var("CLICKHOUSE_DATABASE")
                .unwrap_or_else(|_| "wraith".to_string()),
            clickhouse_table: env::var("CLICKHOUSE_TABLE")
                .unwrap_or_else(|_| "events".to_string()),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            log_json: env::var("LOG_JSON")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
    }
    
    /// Get the full server address
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
    
    /// Get the full ClickHouse table path
    pub fn clickhouse_table_path(&self) -> String {
        format!("{}.{}", self.clickhouse_database, self.clickhouse_table)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}
