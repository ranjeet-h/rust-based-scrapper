use axum::{
    extract::{Path, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use firecrawl::{
    scrape::{ScrapeFormats, ScrapeOptions},
    FirecrawlApp,
    FirecrawlError,
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, instrument}; // Import instrument
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Shared application state
struct AppState {
    db: SqlitePool,
    firecrawl_app: FirecrawlApp,
}

// Data structures
#[derive(Serialize, Deserialize, sqlx::FromRow)]
struct ScrapedItem {
    id: i64,
    url: String,
    content: String, // Will now store Markdown content
    created_at: String, // Using TEXT for simplicity, consider DATETIME
}

#[derive(Deserialize, Debug)]
struct ScrapeRequest {
    url: String,
}

#[derive(Serialize)]
struct ScrapeResponse {
    id: i64,
    url: String,
    content: String, // Send back Markdown content
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

// Custom Error Type
enum AppError {
    Sqlx(sqlx::Error),
    Firecrawl(FirecrawlError),
    Internal(String),
    NotFound(String),
}

// Implement IntoResponse for AppError to convert errors into HTTP responses
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Sqlx(e) => {
                error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Database operation failed: {}", e),
                )
            }
            AppError::Firecrawl(e) => {
                error!("Firecrawl error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Scraping service failed: {}", e),
                )
            }
            AppError::Internal(msg) => {
                error!("Internal server error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        let body = Json(ErrorResponse {
            message: error_message,
        });

        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Item not found in database".to_string()),
            _ => AppError::Sqlx(err),
        }
    }
}

impl From<FirecrawlError> for AppError {
    fn from(err: FirecrawlError) -> Self {
        AppError::Firecrawl(err)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().expect("Failed to load .env file");

    // Initialize tracing (logging)
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Initializing database connection...");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Create SQLite connection pool
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create database pool");

    info!("Running database migrations (creating table if needed)...");
    // Create table if it doesn't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS scraped_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to run database migrations");

    info!("Database initialized successfully.");

    info!("Initializing Firecrawl client...");
    let firecrawl_api_key = env::var("FIRECRAWL_API_KEY").expect("FIRECRAWL_API_KEY must be set");
    if firecrawl_api_key == "YOUR_FIRECRAWL_API_KEY" {
        error!("Placeholder FIRECRAWL_API_KEY found. Please set it in .env");
        panic!("FIRECRAWL_API_KEY not configured");
    }
    let firecrawl_app = FirecrawlApp::new(firecrawl_api_key)?;
    info!("Firecrawl client initialized.");

    // Create shared state
    let shared_state = Arc::new(AppState {
        db: pool,
        firecrawl_app,
    });

    // Configure CORS
    let cors = CorsLayer::new()
        // Allow requests from any origin - adjust in production!
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    // Build application routes
    let app = Router::new()
        .route("/scrape", post(scrape_handler))
        .route("/history", get(get_history_handler))
        .route("/history/:id", get(get_item_handler))
        .with_state(shared_state)
        .layer(cors) // Apply CORS middleware
        .layer(tower_http::trace::TraceLayer::new_for_http()); // Apply tracing

    // Define the server address
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001)); // Use port 3001 for the backend
    info!("Server listening on {}", addr);

    // Run the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// --- API Handlers ---

#[instrument(skip(state))] // Instrument the handler, skipping the state
async fn scrape_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScrapeRequest>,
) -> Result<Json<ScrapeResponse>, AppError> {
    info!("Received scrape request for URL: {}", payload.url);

    // 1. Check if URL already exists in DB
    let existing_item: Option<ScrapedItem> = sqlx::query_as("SELECT * FROM scraped_items WHERE url = ?1")
        .bind(&payload.url)
        .fetch_optional(&state.db)
        .await?;

    if let Some(item) = existing_item {
        info!("URL {} found in database (ID: {}). Returning cached Markdown.", item.url, item.id);
        return Ok(Json(ScrapeResponse {
            id: item.id,
            url: item.url,
            content: item.content, // Return stored Markdown
        }));
    }

    // 2. If not exists, scrape the URL using Firecrawl
    info!("URL {} not found in DB. Scraping with Firecrawl...", payload.url);

    let scrape_options = ScrapeOptions {
        formats: Some(vec![ScrapeFormats::Markdown]), // Request only Markdown
        ..Default::default()
    };

    let scrape_result = state
        .firecrawl_app
        .scrape_url(&payload.url, Some(scrape_options))
        .await?; // Use `?` to propagate FirecrawlError

    // Extract Markdown content
    let markdown_content = scrape_result
        .markdown
        .ok_or_else(|| AppError::Internal("Firecrawl did not return Markdown content".to_string()))?;

    info!(
        "Successfully scraped {} using Firecrawl ({} bytes of Markdown)",
        payload.url,
        markdown_content.len()
    );

    // 3. Insert Markdown content into database
    let result = sqlx::query(
        "INSERT INTO scraped_items (url, content) VALUES (?1, ?2)"
    )
    .bind(&payload.url)
    .bind(&markdown_content) // Store Markdown content
    .execute(&state.db)
    .await?;

    let new_id = result.last_insert_rowid();
    info!("Successfully inserted Markdown for URL {} with ID {}", payload.url, new_id);

    // Return the newly scraped Markdown content
    Ok(Json(ScrapeResponse {
        id: new_id,
        url: payload.url,
        content: markdown_content,
    }))
}

#[instrument(skip(state))]
async fn get_history_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ScrapedItem>>, AppError> {
    info!("Fetching scrape history");
    let items = sqlx::query_as::<_, ScrapedItem>("SELECT id, url, content, created_at FROM scraped_items ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await?;
    info!("Found {} items in history", items.len());
    Ok(Json(items))
}

#[instrument(skip(state))]
async fn get_item_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<ScrapedItem>, AppError> {
    info!("Fetching scraped item with ID: {}", id);
    let item = sqlx::query_as::<_, ScrapedItem>("SELECT id, url, content, created_at FROM scraped_items WHERE id = ?1")
        .bind(id)
        .fetch_one(&state.db) // Use fetch_one to get a specific item or error if not found
        .await?; // Automatically converts RowNotFound to AppError::NotFound via From trait
    info!("Found item with ID: {}", item.id);
    Ok(Json(item))
} 