use std::sync::Arc;
use tokio::net::TcpListener;
use std::collections::HashMap;
use std::sync::Mutex;
use rust_web_scrapper::{
    config::Config,
    api::routes::create_router,
    AppState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::load()?;
    let server_addr = config.server_addr;
    println!("Starting server on {}", server_addr);
    
    // Create application state
    let app_state = AppState {
        config: Arc::new(config),
        cache: Arc::new(Mutex::new(HashMap::new())),
    };
    
    // Build the router with routes
    let app = create_router(app_state);
    
    // Create the listener
    let listener = TcpListener::bind(server_addr).await?;
    
    // Start the server
    println!("Listening on {}", server_addr);
    axum::serve(listener, app).await?;
    
    Ok(())
}
