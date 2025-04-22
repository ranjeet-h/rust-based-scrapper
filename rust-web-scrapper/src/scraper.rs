use reqwest::{Client, ClientBuilder};
use scraper::{Html, Selector};
use std::time::Duration;
use once_cell::sync::Lazy;
use crate::error::Result;

// Create a static client to reuse connections
static CLIENT: Lazy<Client> = Lazy::new(|| {
    ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(10)
        .build()
        .expect("Failed to build HTTP client")
});

// Create static selectors to avoid recompiling them each time
static BODY_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("body").expect("Failed to parse body selector")
});

// Content selectors - most websites use these elements for main content
static CONTENT_SELECTORS: Lazy<Vec<Selector>> = Lazy::new(|| {
    vec![
        // Main content selectors
        Selector::parse("main").expect("Failed to parse main selector"),
        Selector::parse("article").expect("Failed to parse article selector"),
        Selector::parse(".content, .main-content, #content, #main-content").expect("Failed to parse content class selector"),
        
        // Fallback to common containers
        Selector::parse(".post, .entry, .blog-post").expect("Failed to parse post selector"),
        
        // Content areas by semantic HTML5 tags
        Selector::parse("section").expect("Failed to parse section selector"),
    ]
});

// Selector for elements to remove
static NOISE_SELECTORS: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("script, style, nav, header, footer, iframe, noscript, svg, .ads, .advertisement, .banner, .cookie-banner, .cookie-notice, .popup").expect("Failed to parse noise selector")
});

pub async fn fetch_html(url: &str) -> Result<String> {
    let response = CLIENT.get(url).send().await?;
    let html = response.text().await?;
    Ok(html)
}

pub fn extract_body(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    
    document.select(&BODY_SELECTOR)
        .next()
        .map(|element| element.inner_html())
}

pub fn format_html(html: &str) -> String {
    let document = Html::parse_document(html);
    
    // First try to find main content using content selectors
    let mut content = String::new();
    for selector in CONTENT_SELECTORS.iter() {
        if let Some(element) = document.select(selector).next() {
            // Create a new document from this content element
            let mut content_doc = element.inner_html();
            
            // Remove noise elements from this content
            let content_fragment = Html::parse_fragment(&content_doc);
            for noise in content_fragment.select(&NOISE_SELECTORS) {
                if let Some(_) = noise.parent() {
                    // This is a placeholder since we can't modify the DOM directly in scraper
                    content_doc = content_doc.replace(&noise.html(), "");
                }
            }
            
            content = content_doc;
            println!("Found content using selector: {:?}", selector);
            break;
        }
    }
    
    // If no content was found, use the entire body but clean it
    if content.is_empty() {
        if let Some(body) = document.select(&BODY_SELECTOR).next() {
            let mut body_content = body.inner_html();
            
            // Remove noise elements from body
            let body_fragment = Html::parse_fragment(&body_content);
            for noise in body_fragment.select(&NOISE_SELECTORS) {
                if let Some(_) = noise.parent() {
                    body_content = body_content.replace(&noise.html(), "");
                }
            }
            
            content = body_content;
            println!("Using cleaned body content");
        }
    }
    
    // Clean up HTML tags and normalize whitespace
    clean_html_content(&content)
}

fn clean_html_content(html: &str) -> String {
    // Simple HTML tag removal
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_entity = false;
    
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                result.push(' '); // Replace tags with space
                continue;
            }
            '&' => in_entity = true,
            ';' => if in_entity {
                in_entity = false;
                result.push(' '); // Replace entities with space
                continue;
            },
            _ => {}
        }
        
        if !in_tag && !in_entity {
            result.push(c);
        }
    }
    
    // Normalize whitespace
    let mut normalized = String::with_capacity(result.len());
    let mut last_was_whitespace = true;
    
    for c in result.chars() {
        if c.is_whitespace() {
            if !last_was_whitespace {
                normalized.push(' ');
                last_was_whitespace = true;
            }
        } else {
            normalized.push(c);
            last_was_whitespace = false;
        }
    }
    
    normalized
}

pub fn build_prompt(content: &str) -> String {
    // Use a more efficient string format that pre-allocates approximately the right amount of space
    let mut result = String::with_capacity(content.len() + 150);
    result.push_str("The following is the content of a webpage. Please provide a concise summary formatted in Markdown. Focus on the key points, main ideas, and important details. Use headers, bullet points, and other Markdown formatting to make the summary structured and readable:\n\n");
    result.push_str(content);
    result
} 