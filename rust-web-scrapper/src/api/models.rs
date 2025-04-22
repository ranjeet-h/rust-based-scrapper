use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Deserialize)]
pub struct ScrapeRequest {
    pub url: String,
}

#[derive(Serialize)]
pub struct ScrapeResponse {
    pub url: String,
    #[serde(rename = "summary_markdown")]
    pub summary: String,
    pub scraped_at: DateTime<Utc>,
    pub word_count: usize,
    pub status: String,
} 