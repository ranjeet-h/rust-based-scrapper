pub mod api;
pub mod config;
pub mod error;
pub mod llm;
pub mod scraper;

use std::sync::Arc;
use config::Config;
use std::collections::HashMap;
use std::sync::Mutex;
use chrono::DateTime;
use chrono::Utc;

/// Application state that will be shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub cache: Arc<Mutex<HashMap<String, CachedResponse>>>,
}

/// Structure to store cached responses
#[derive(Clone)]
pub struct CachedResponse {
    pub summary: String,
    pub word_count: usize,
    pub timestamp: DateTime<Utc>,
} 