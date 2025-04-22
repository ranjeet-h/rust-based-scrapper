use std::env;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use crate::error::{AppError, Result};

#[derive(Clone)]
pub struct Config {
    pub server_addr: SocketAddr,
    pub openrouter_api_key: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Load environment variables from .env file if it exists
        dotenv::dotenv().ok();
        
        // Load OpenRouter API key
        let openrouter_api_key = env::var("OPENROUTER_API_KEY")?;
        
        // Load server configuration with defaults
        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        let port = port.parse::<u16>().map_err(|e| AppError::ConfigError(format!("Invalid port: {}", e)))?;
        let ip = IpAddr::from_str(&host).map_err(|e| AppError::ConfigError(format!("Invalid host address: {}", e)))?;
        
        let server_addr = SocketAddr::new(ip, port);
        
        Ok(Config {
            server_addr,
            openrouter_api_key,
        })
    }
} 