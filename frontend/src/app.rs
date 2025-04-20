use poll_promise::Promise;
use serde::{Deserialize, Serialize};
use std::fmt; // Import fmt for custom error Display
use serde_json;
use egui_commonmark::CommonMarkViewer;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlElement, HtmlAnchorElement};

#[cfg(not(target_arch = "wasm32"))]
use printpdf::{Mm, PdfDocument, Point}; // Imports for native PDF generation

// Define the backend URL
const BACKEND_URL: &str = "http://127.0.0.1:3001";

// Define structs matching Backend API
#[derive(Serialize, Deserialize, Debug, Clone)] // Need Clone for history
struct BackendScrapeResponse {
    id: i64,
    url: String,
    content: String, // Markdown content from backend
}

// Simplified representation for history (can be expanded later)
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HistoryItem {
    url: String,
    markdown: String,
}

// Custom Error type for Frontend operations
#[derive(Debug)]
enum FrontendError {
    Http(reqwest::Error),
    JsonParse(serde_json::Error), // If using serde_json for response parsing
    ApiError(String),          // Errors reported by the backend API
    Other(String),
}

impl fmt::Display for FrontendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrontendError::Http(e) => write!(f, "HTTP request failed: {}", e),
            FrontendError::JsonParse(e) => write!(f, "Failed to parse JSON response: {}", e),
            FrontendError::ApiError(msg) => write!(f, "API Error: {}", msg),
            FrontendError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

// Convert reqwest errors
impl From<reqwest::Error> for FrontendError {
    fn from(err: reqwest::Error) -> Self {
        FrontendError::Http(err)
    }
}

// Convert serde_json errors (if using it directly)
impl From<serde_json::Error> for FrontendError {
    fn from(err: serde_json::Error) -> Self {
        FrontendError::JsonParse(err)
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // State for the web scraper app
    input_url: String,
    #[serde(skip)] // Avoid serializing potentially large markdown data or promises
    markdown_content: Option<String>, // Displayed markdown (from selected history or latest scrape)
    #[serde(skip)]
    error_message: Option<String>,
    #[serde(skip)]
    // Update promise type to reflect backend call
    scrape_promise: Option<Promise<Result<BackendScrapeResponse, FrontendError>>>,
    #[serde(skip)] // Don't persist history for now
    // Use HistoryItem struct for clarity
    scrape_history: Vec<HistoryItem>,
    #[serde(skip)]
    selected_history_index: Option<usize>, // Index of the currently viewed history item
    #[serde(skip)]
    is_displaying_result: bool, // Added flag: true if showing result, false if ready for input
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            input_url: "".to_owned(),
            markdown_content: None,
            error_message: None,
            scrape_promise: None,
            scrape_history: Vec::new(),
            selected_history_index: None,
            is_displaying_result: false, // Default to ready for input
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // TODO: Implement loading history from backend here?
        // For now, just default state.
        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Only save non-skipped fields (input_url)
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Handle Promise Resolution --- New Logic ---
        if let Some(promise) = &self.scrape_promise {
            if let Some(result) = promise.ready() {
                match result {
                    // Success case: Backend returned a valid response
                    Ok(response) => {
                        let history_item = HistoryItem {
                            url: response.url.clone(),
                            markdown: response.content.clone(),
                        };
                        self.markdown_content = Some(response.content.clone()); // Update display
                        self.error_message = None;
                        self.is_displaying_result = true; // Set flag on success

                        // Add to history, preventing exact duplicates
                        if self.scrape_history.last().map_or(true, |last| last.url != history_item.url) {
                            self.scrape_history.push(history_item);
                        }
                        // Automatically select the latest history item
                        self.selected_history_index = Some(self.scrape_history.len() - 1);
                    }
                    // Failure case: Promise returned an error (HTTP, JSON, API, etc.)
                    Err(e) => {
                        log::error!("Scraping failed: {}", e);
                        self.error_message = Some(format!("{}", e)); // Use Display impl of FrontendError
                        self.markdown_content = None; // Clear content on error
                        self.selected_history_index = None; // De-select history on error
                        self.is_displaying_result = false; // Clear flag on error
                    }
                }
                // Promise is finished, remove it
                self.scrape_promise = None;
            }
        }
        // --- End Handle Promise Resolution ---

        // Determine if currently loading
        let is_loading = self.scrape_promise.is_some() && self.scrape_promise.as_ref().map_or(false, |p| p.ready().is_none());

        // --- Top Panel (unchanged) ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.heading("Ruscraper");
            });
        });

        // --- Left Panel for History - Adjusted for HistoryItem ---
        egui::SidePanel::left("history_panel")
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("History");
                ui.add_space(10.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.scrape_history.is_empty() {
                        ui.label("(No history yet)");
                    } else {
                        // Iterate in reverse to show newest first
                        for i in (0..self.scrape_history.len()).rev() {
                            let item = &self.scrape_history[i];
                            let display_url = item.url.splitn(4, '/').nth(2).unwrap_or(&item.url).to_string();
                            let label_text = format!("{}: {}", i + 1, display_url);
                            let is_selected = self.selected_history_index == Some(i);

                            ui.horizontal(|ui| {
                                if ui.selectable_label(is_selected, label_text).clicked() {
                                    self.selected_history_index = Some(i);
                                    self.markdown_content = Some(item.markdown.clone());
                                    self.error_message = None;
                                    self.input_url = item.url.clone(); // Update input field when selecting history
                                }
                                // TODO: Add backend calls for Export/Delete later
                                ui.add_enabled(!is_loading, egui::Button::new("MD").small())
                                    .on_hover_text("Export Markdown (NYI)");
                                ui.add_enabled(!is_loading, egui::Button::new("PDF").small())
                                    .on_hover_text("Export PDF (NYI)");
                                ui.add_enabled(!is_loading, egui::Button::new("🗑").small())
                                    .on_hover_text("Delete History Item (NYI)");
                            });
                        }
                    }
                });
            });

        // --- Bottom Panel for Input, Controls, and Errors --- Updated Scrape Logic ---
        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(5.0);
                let panel_frame = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 5));
                panel_frame.show(ui, |ui| {
                    if let Some(err) = &self.error_message {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        ui.add_space(2.0);
                    }

                    ui.horizontal(|ui| {
                        let available_width = ui.available_width();
                        let desired_input_width = (available_width - 100.0f32).max(0.0f32);
                        // let input_height = 35.0f32; // Removed, seems unused now

                        // Use add_enabled_ui to create a conditionally enabled scope
                        let mut inner_response: Option<egui::Response> = None;
                        // Input field enabled only when not loading and not displaying result
                        let outer_response = ui.add_enabled_ui(!is_loading && !self.is_displaying_result, |ui| {
                            // Add the widget directly with desired width and height
                             let response = ui.add(
                                 egui::TextEdit::singleline(&mut self.input_url)
                                     .desired_width(desired_input_width)
                                     .min_size(egui::vec2(0.0, 35.0)) // Set minimum height to match button
                                     .hint_text("Enter URL to scrape...")
                             );
                             inner_response = Some(response); // Store the inner response
                        });

                        // Use the inner response if the UI was enabled, otherwise use the outer one (which might indicate disabled state)
                        let url_input_response = inner_response.unwrap_or(outer_response.response);

                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            if self.is_displaying_result {
                                // Show "New +" button when displaying a result
                                let new_button = egui::Button::new("➕ New")
                                    .min_size(egui::vec2(100.0, 35.0));
                                if ui.add(new_button).clicked() {
                                    // Reset state for a new scrape
                                    self.input_url.clear(); // Ensure input is clear
                                    self.markdown_content = None;
                                    self.error_message = None;
                                    self.selected_history_index = None;
                                    self.is_displaying_result = false;
                                }
                            } else {
                                // Show "Send" button when ready for input
                                let scrape_button = egui::Button::new(if is_loading { "..." } else { "Send" })
                                    .min_size(egui::vec2(100.0, 35.0));
                                let button_response = ui.add_enabled(!is_loading && !self.input_url.trim().is_empty(), scrape_button)
                                    .on_hover_text("Call backend to fetch and process URL");

                                let enter_pressed = url_input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                                if button_response.clicked() || enter_pressed {
                                    if !self.input_url.trim().is_empty() {
                                        self.error_message = None;
                                        self.markdown_content = None;
                                        self.selected_history_index = None;
                                        // No need to set is_displaying_result to false here, happens on promise error
                                        let url_to_fetch = self.input_url.clone();
                                        log::info!("Sending scrape request to backend for URL: {}", url_to_fetch);

                                        // --- Create and store the promise --- NEW LOGIC ---
                                        let promise = {
                                            let request_url = format!("{}/scrape", BACKEND_URL);
                                            let _client = reqwest::Client::new(); // Prefix with underscore as it's only used in spawn_async
                                            let request_payload = serde_json::json!({ "url": url_to_fetch });

                                            #[cfg(not(target_arch = "wasm32"))]
                                            {
                                                Promise::spawn_thread("backend_scrape", move || {
                                                    // Use blocking client for simplicity in spawn_thread
                                                    let blocking_client = reqwest::blocking::Client::new();
                                                    let resp = blocking_client.post(&request_url)
                                                        .json(&request_payload)
                                                        .send()?;

                                                    if resp.status().is_success() {
                                                        let parsed = resp.json::<BackendScrapeResponse>()?;
                                                        Ok(parsed)
                                                    } else {
                                                        let err_text = resp.text()?;
                                                        log::error!("Backend API error: {}", err_text);
                                                        Err(FrontendError::ApiError(err_text))
                                                    }
                                                })
                                            }
                                            #[cfg(target_arch = "wasm32")]
                                            {
                                                Promise::spawn_async(async move {
                                                    // Create the client inside the async block for wasm
                                                    let client = reqwest::Client::new(); 
                                                    let resp = client.post(&request_url)
                                                        .json(&request_payload)
                                                        .send()
                                                        .await?;

                                                    if resp.status().is_success() {
                                                        let parsed = resp.json::<BackendScrapeResponse>().await?;
                                                        Ok(parsed)
                                                    } else {
                                                         let err_text = resp.text().await?;
                                                         log::error!("Backend API error: {}", err_text);
                                                         Err(FrontendError::ApiError(err_text))
                                                    }
                                                })
                                            }
                                        };
                                        self.scrape_promise = Some(promise);
                                        // self.input_url.clear(); // Already cleared outside this block
                                    }
                                }
                            }
                        });
                    });
                    ui.add_space(5.0);

                     // --- Footer Row (unchanged for now) ---
                    ui.horizontal(|ui| {
                        // Right-align theme/footer elements
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(10.0);
                            egui::widgets::global_theme_preference_buttons(ui);
                            let is_web = cfg!(target_arch = "wasm32");
                            if !is_web {
                                if ui.button("Quit").clicked() {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            }
                        });
                    });
                });
                ui.add_space(5.0);
            });

        // --- Central Panel for the Markdown Output - Adjusted for HistoryItem ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Scraped Content");
                // Add Export buttons to the right if displaying a result
                if self.is_displaying_result {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0); // Spacing before buttons
                        
                        // PDF Export Button (Now Enabled)
                        // let pdf_button = egui::Button::new("📄 PDF");
                        // if ui.add(pdf_button).on_hover_text("Export as PDF (Basic)").clicked() {
                        //      if let Some(content) = &self.markdown_content {
                        //         save_pdf_file("scraped_content.pdf", content);
                        //     } else {
                        //         log::warn!("No content available to export as PDF.");
                        //     }
                        // }
                        
                        // Markdown Export Button
                        let md_button = egui::Button::new("Ⓜ️ MD");
                        if ui.add(md_button).on_hover_text("Export as Markdown").clicked() {
                            if let Some(content) = &self.markdown_content {
                                save_markdown_file("scraped_content.md", content);
                            } else {
                                log::warn!("No markdown content available to export.");
                            }
                        }
                    });
                }
            });

            ui.add_space(5.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                 egui::ScrollArea::vertical()
                     .stick_to_bottom(true)
                     .auto_shrink([false, false])
                     .show(ui, |ui| {
                        if is_loading {
                            ui.add(egui::Spinner::new());
                            ui.label("Fetching content...");
                        } else {
                            let display_text = self.markdown_content.as_deref()
                                .unwrap_or("Scraped content will appear here...\n\nEnter a URL below and click Send to fetch from backend.");

                            // Use CommonMarkViewer to render the markdown
                            CommonMarkViewer::new()
                                .show(ui, &mut egui_commonmark::CommonMarkCache::default(), display_text);
                        }
                     });
            });
        });
    }
}

// ---- Helper Function for Saving Files ----

fn save_markdown_file(filename: &str, content: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native: Use rfd to show a save file dialog
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(filename)
            .add_filter("Markdown", &["md"])
            .save_file() else {
            log::info!("User cancelled save dialog.");
            return;
        };
        
        match std::fs::write(&path, content) {
            Ok(_) => log::info!("Markdown saved to: {:?}", path),
            Err(e) => log::error!("Failed to save markdown file: {}", e),
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        // Web: Trigger a download using data URL
        trigger_download(filename, content);
    }
}

// ---- Helper Function for Saving Basic PDF ----

fn save_pdf_file(filename: &str, content: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Native: Use rfd for save dialog and printpdf for basic PDF
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(filename)
            .add_filter("PDF Document", &["pdf"])
            .save_file() else {
            log::info!("User cancelled save dialog.");
            return;
        };

        match create_basic_pdf(content) {
            Ok(pdf_bytes) => {
                match std::fs::write(&path, pdf_bytes) {
                    Ok(_) => log::info!("PDF saved to: {:?}", path),
                    Err(e) => log::error!("Failed to write PDF file: {}", e),
                }
            }
            Err(e) => {
                 log::error!("Failed to generate basic PDF: {}", e);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        // Web: Trigger download of the raw text with .pdf extension
        // (This is NOT a real PDF, just the text content)
        log::warn!("WASM PDF export is basic: downloading raw text with .pdf extension.");
        trigger_download(filename, content);
    }
}

// ---- Helper to create a very basic PDF with printpdf (Native Only) ----
#[cfg(not(target_arch = "wasm32"))]
fn create_basic_pdf(content: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let (doc, page1, layer1) = PdfDocument::new("Scraped Content", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    // Use a basic built-in font
    let font = doc.add_builtin_font(printpdf::BuiltinFont::Helvetica)?;
    let font_size = 10.0;
    let line_height = 12.0; // Slightly larger than font size
    let margin_top = 280.0;
    let margin_bottom = 15.0;
    let mut y_position = margin_top;

    current_layer.set_font(&font, font_size);

    for line in content.lines() {
        // Stop if we are too close to the bottom margin
        if y_position < margin_bottom {
            log::warn!("PDF content truncated due to reaching page bottom.");
            break;
        }
        // Add the current line
        current_layer.use_text(line.to_string(), font_size, Mm(10.0), Mm(y_position), &font);
        // Move down for the next line
        y_position -= line_height;
    }

    let pdf_bytes = doc.save_to_bytes()?;
    Ok(pdf_bytes)
}

// ---- Helper function for WASM download ----
#[cfg(target_arch = "wasm32")]
fn trigger_download(filename: &str, content: &str) {
    use base64::{engine::general_purpose, Engine as _};

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().expect("document should have a body");

    // Create an anchor element
    let link = document
        .create_element("a")
        .expect("Failed to create anchor element")
        .dyn_into::<HtmlAnchorElement>()
        .expect("Failed to cast to HtmlAnchorElement");

    // Create a data URL
    // Using base64 encoding for potentially large/complex markdown
    let base64_content = general_purpose::STANDARD.encode(content);
    // Adjust mime type based on filename? Or keep generic?
    // Forcing text/plain for wasm pdf download as it's not a real pdf.
    let mime_type = if filename.ends_with(".pdf") {
         "text/plain" 
    } else {
         "text/markdown"
    }; // Simplification for WASM
    let href = format!("data:{};charset=utf-8;base64,{}", mime_type, base64_content);
    
    link.set_href(&href);
    link.set_download(filename);

    // Append to body, click, and remove
    let style = link.style();
    style.set_property("display", "none").expect("Failed to set style");
    body.append_child(&link).expect("Failed to append link");
    link.click();
    body.remove_child(&link).expect("Failed to remove link");
    log::info!("Triggered download for {}", filename);
}

// fn powered_by_egui_and_eframe(ui: &mut egui::Ui) { ... } // Keep if desired
