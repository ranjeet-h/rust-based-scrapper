#[cfg(target_arch = "wasm32")]
use gloo_timers::future::TimeoutFuture;

#[cfg(not(target_arch = "wasm32"))]
use std::{thread, time::Duration};

/// Represents errors that can occur during the fetching or conversion process.
#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    #[error("Network request failed: {0}")]
    FetchError(String), // In a real app, this might be reqwest::Error or similar
    #[error("Failed to convert content: {0}")]
    ConversionError(String),
    #[error("Mock error: {0}")]
    MockError(String),
}

/// Simulates fetching content from a URL and converting it to Markdown.
///
/// In a real application, this would involve:
/// 1. Making an HTTP GET request to the `url`.
/// 2. Parsing the HTML response.
/// 3. Converting the HTML to Markdown.
///
/// This mock version just simulates a delay and returns predefined content or an error.
pub async fn fetch_and_convert(url: String) -> Result<String, ProcessingError> {
    log::info!("Processing request for URL: {}", url);

    // Simulate network delay
    #[cfg(target_arch = "wasm32")]
    {
        TimeoutFuture::new(1_500).await; // Simulate 1.5 seconds loading time
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        thread::sleep(Duration::from_millis(1500));
    }

    // --- Mock Logic ---
    // Simulate potential errors based on URL or randomly
    if url.contains("error") {
        log::warn!("Simulating a mock error for URL: {}", url);
        Err(ProcessingError::MockError("Simulated failure to process URL.".to_string()))
    } else if url.trim().is_empty() {
         Err(ProcessingError::MockError("URL cannot be empty.".to_string()))
    }
     else {
        log::info!("Successfully processed URL: {}", url);
        // Return mock markdown content
        Ok(format!(
            "# Mock Result for: `{}`

This is simulated Markdown content.

- Fetched data would go here.
- Conversion logic would be applied.

*Timestamp:* `{:?}`",
            url,
            chrono::Utc::now() // Add a timestamp to show it's dynamic
        ))
    }
    // --- End Mock Logic ---
} 