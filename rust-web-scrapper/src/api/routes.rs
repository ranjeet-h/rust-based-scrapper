use axum::{
    routing::post,
    Router,
    extract::{Json, State},
    response::IntoResponse,
};
use tower_http::cors::{CorsLayer, Any};
use chrono::Utc;
use std::time::Duration;

use crate::error::{Result, AppError};
use crate::api::models::{ScrapeRequest, ScrapeResponse};
use crate::api::response;
use crate::scraper::{fetch_html, extract_body, format_html, build_prompt};
use crate::llm::call_openrouter;
use crate::{AppState, CachedResponse};

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .route("/api/scrape", post(scrape_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(app_state)
}

async fn scrape_handler(
    State(state): State<AppState>,
    Json(req): Json<ScrapeRequest>,
) -> impl IntoResponse {
    println!("Processing request for URL: {}", req.url);
    let start_time = std::time::Instant::now();
    
    // Set an overall timeout for the entire handler
    let result = tokio::time::timeout(
        Duration::from_secs(90), // Overall handler timeout of 90 seconds
        process_scrape_request(&state, &req)
    ).await;
    
    let elapsed = start_time.elapsed();
    println!("Request Processing took: {:?}", elapsed);
    
    match result {
        Ok(result) => match result {
            Ok(response_data) => {
                println!("Successfully processed URL: {}", req.url);
                response::success(response_data)
            },
            Err(err) => {
                let (status, msg) = match &err {
                    AppError::FetchError(msg) => {
                        println!("Fetch error: {}", msg);
                        (axum::http::StatusCode::BAD_REQUEST, msg.clone())
                    },
                    AppError::ParseError(msg) => {
                        println!("Parse error: {}", msg);
                        (axum::http::StatusCode::UNPROCESSABLE_ENTITY, msg.clone())
                    },
                    AppError::LlmError(msg) => {
                        println!("LLM error: {}", msg);
                        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
                    },
                    AppError::ConfigError(msg) => {
                        println!("Config error: {}", msg);
                        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
                    },
                };
                
                response::error(status, msg)
            }
        },
        Err(_) => {
            println!("Request timed out after {:?}", elapsed);
            response::error(
                axum::http::StatusCode::REQUEST_TIMEOUT, 
                "Request processing timed out".to_string()
            )
        },
    }
}

async fn process_scrape_request(state: &AppState, req: &ScrapeRequest) -> Result<ScrapeResponse> {
    // Check cache first
    let cache_key = &req.url;
    
    // Try to get from cache with lock scope to minimize lock contention
    {
        let cache = state.cache.lock().unwrap();
        if let Some(cached) = cache.get(cache_key) {
            // Only use cache if it's less than 24 hours old
            let cache_age = Utc::now() - cached.timestamp;
            if cache_age < chrono::Duration::hours(24) {
                println!("Cache hit for URL: {}", req.url);
                return Ok(ScrapeResponse {
                    url: req.url.clone(),
                    summary: cached.summary.clone(),
                    scraped_at: Utc::now(),
                    word_count: cached.word_count,
                    status: "success (cached)".to_string(),
                });
            }
        }
    }

    println!("Fetching HTML for URL: {}", req.url);
    let fetch_start = std::time::Instant::now();
    
    // Fetch with even shorter timeout - 5 seconds
    let html_result = tokio::time::timeout(
        Duration::from_secs(5), 
        fetch_html(&req.url)
    ).await;
    
    let html = match html_result {
        Ok(result) => {
            match result {
                Ok(html) => {
                    println!("HTML fetch successful in {:?}", fetch_start.elapsed());
                    html
                },
                Err(e) => {
                    println!("HTML fetch error: {}", e);
                    return Err(AppError::FetchError(format!("Failed to fetch HTML: {}", e)));
                }
            }
        },
        Err(_) => {
            println!("HTML fetch timed out after 5 seconds");
            return Err(AppError::FetchError("HTML fetch timed out after 5 seconds".to_string()));
        }
    };
    
    println!("🔍 Extracting and formatting HTML content");
    let raw_body = extract_body(&html)
        .ok_or_else(|| {
            println!("No <body> tag found in HTML");
            crate::error::AppError::ParseError("No <body> tag found in the HTML".to_string())
        })?;
    
    let formatted = format_html(&raw_body);
    println!("Content size: {} chars (using full content)", formatted.len());
    
    let prompt = build_prompt(&formatted);
    println!("Built prompt with length: {} chars", prompt.len());

    // Calculate word count
    let word_count = formatted.split_whitespace().count();
    println!("Word count: {}", word_count);

    println!("Calling LLM API...");
    let llm_start = std::time::Instant::now();
    
    // Try with an even shorter timeout for the LLM
    let summary_result = call_openrouter(
        &state.config.openrouter_api_key, 
        &prompt, 
        Some(&req.url), 
        None
    ).await;
    
    let summary = match summary_result {
        Ok(summary) => {
            println!("LLM API call successful in {:?}", llm_start.elapsed());
            summary
        },
        Err(e) => {
            println!("LLM API error: {}", e);
            return Err(AppError::LlmError(format!("LLM API error: {}", e)));
        }
    };

    println!("Formatting summary...");
    // Ensure proper Markdown formatting
    let formatted_summary = ensure_markdown_formatting(&summary);
    
    // No truncation happening now, so use the summary directly
    let final_summary = formatted_summary;

    println!("Storing result in cache");
    // Store in cache
    {
        let mut cache = state.cache.lock().unwrap();
        cache.insert(req.url.clone(), CachedResponse {
            summary: final_summary.clone(),
            word_count,
            timestamp: Utc::now(),
        });
    }

    println!("Request completed successfully for URL: {}", req.url);
    Ok(ScrapeResponse {
        url: req.url.clone(),
        summary: final_summary,
        scraped_at: Utc::now(),
        word_count,
        status: "success".to_string(),
    })
}

/// Ensures the text is properly formatted as Markdown.
/// This function does some basic validation and formatting to improve the Markdown structure.
fn ensure_markdown_formatting(text: &str) -> String {
    let text = text.trim();
    
    // Make sure it has a title/heading
    if !text.starts_with('#') {
        // Try to find a good title from the first line or use "Website Summary"
        let first_line = text.lines().next().unwrap_or("Website Summary");
        let title = if first_line.len() > 50 {
            "Website Summary"
        } else {
            first_line
        };
        
        return format!("# {}\n\n{}", title, text);
    }
    
    // Text already has Markdown formatting, return as is
    text.to_string()
} 