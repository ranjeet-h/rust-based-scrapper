use poll_promise::Promise;
use serde::{Deserialize, Serialize};
use std::fmt;
use serde_json;
use egui_commonmark::CommonMarkViewer;
use egui::ComboBox;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlElement, HtmlAnchorElement};

#[cfg(not(target_arch = "wasm32"))]
use printpdf::{Mm, PdfDocument}; // Removed Point

// Define the backend URLs
const FIRECROWL_URL: &str = "http://127.0.0.1:8000"; // Updated Port for Firecrowl (@backend)
const LLM_SCRAPER_URL: &str = "http://127.0.0.1:3000"; // URL for LLM Scraper (@rust-web-scrapper)

// Enum to represent the scraper type
#[derive(Debug, PartialEq, Copy, Clone, serde::Deserialize, serde::Serialize)]
enum ScraperType {
    Firecrowl, // Renamed from Backend
    LLM,       // Renamed from RustWebScraper
}

// Implement Display for ScraperType for the ComboBox
impl fmt::Display for ScraperType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScraperType::Firecrowl => write!(f, "Firecrowl"), // Updated display name
            ScraperType::LLM => write!(f, "LLM"),             // Updated display name
        }
    }
}

// Define structs matching Backend API Responses
#[derive(Serialize, Deserialize, Debug, Clone)]
struct FirecrowlScrapeResponse {
    id: i64,
    url: String,
    content: String, // Markdown content from backend
}

// Define struct matching LLM Scraper API Response
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LlmScrapeResponse {
    url: String,
    #[serde(rename = "summary_markdown")] // Match backend field name
    summary: String,
    // Add other fields if needed, matching the backend's models.rs ScrapeResponse
    // scraped_at: Option<String>, // Using Option<String> for simplicity if DateTime parsing is complex
    // word_count: Option<usize>,
    // status: Option<String>,
}

// Define structs to match the LLM backend's ApiResponse wrapper
#[derive(Deserialize, Debug, Clone)]
struct LlmApiResponse<T> {
    data: Option<T>,
    meta: LlmResponseMeta,
}

#[derive(Deserialize, Debug, Clone)]
struct LlmResponseMeta {
    status: String,
    status_code: u16,
    timestamp: String, // Keep as String for simplicity
    message: Option<String>,
}

// Enum to hold the active promise, distinguishing its type
enum ActivePromise {
    Firecrowl(Promise<Result<FirecrowlScrapeResponse, FrontendError>>),
    Llm(Promise<Result<LlmApiResponse<LlmScrapeResponse>, FrontendError>>),
}

// Result type for the promise, holding either response type
#[derive(Debug, Clone)]
enum ScrapeResult {
    Firecrowl(FirecrowlScrapeResponse),
    Llm(LlmScrapeResponse),
}

// Simplified representation for history
#[derive(Serialize, Deserialize, Debug, Clone)]
struct HistoryItem {
    url: String,
    markdown: String,
}

// Custom Error type for Frontend operations
#[derive(Debug)]
enum FrontendError {
    Http(reqwest::Error), // Keep for now, although ehttp is primary now
    EHttp(String),        // Add variant for ehttp errors
    JsonParse(serde_json::Error),
    ApiError(String), // Errors reported by the backend API
    Other(String),
}

impl fmt::Display for FrontendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrontendError::Http(e) => write!(f, "HTTP request failed: {}", e),
            FrontendError::EHttp(e) => write!(f, "HTTP request failed: {}", e),
            FrontendError::JsonParse(e) => write!(f, "Failed to parse JSON response: {}", e),
            FrontendError::ApiError(msg) => write!(f, "API Error: {}", msg),
            FrontendError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

// Convert reqwest errors (keep for now)
impl From<reqwest::Error> for FrontendError {
    fn from(err: reqwest::Error) -> Self {
        FrontendError::Http(err)
    }
}

// Convert ehttp errors
impl From<ehttp::Error> for FrontendError {
    fn from(err: ehttp::Error) -> Self {
        FrontendError::EHttp(err.to_string())
    }
}

// Convert serde_json errors
impl From<serde_json::Error> for FrontendError {
    fn from(err: serde_json::Error) -> Self {
        FrontendError::JsonParse(err)
    }
}

/// Main application state
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TemplateApp {
    input_url: String,
    #[serde(skip)]
    markdown_content: Option<String>,
    #[serde(skip)]
    error_message: Option<String>,
    #[serde(skip)]
    scrape_promise: Option<ActivePromise>,
    #[serde(skip)]
    scrape_history: Vec<HistoryItem>,
    #[serde(skip)]
    selected_history_index: Option<usize>,
    #[serde(skip)]
    is_displaying_result: bool,
    #[serde(skip)]
    selected_scraper: ScraperType,
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
            is_displaying_result: false,
            selected_scraper: ScraperType::Firecrowl, // Default to Firecrowl
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Handle Promise Resolution (Revised Logic) ---
        let mut promise_finished = false;
        if let Some(active_promise) = &self.scrape_promise {
            match active_promise {
                ActivePromise::Firecrowl(promise) => {
                    if let Some(result_ref) = promise.ready() {
                        match result_ref {
                            Ok(response_ref) => {
                                // --- Success Case (Firecrowl) ---
                                let history_item = HistoryItem {
                                    url: response_ref.url.clone(),
                                    markdown: response_ref.content.clone(),
                                };
                                self.markdown_content = Some(response_ref.content.clone());
                                self.error_message = None;
                                self.is_displaying_result = true;
                                if self.scrape_history.last().map_or(true, |last| last.url != history_item.url) {
                                    self.scrape_history.push(history_item);
                                }
                                self.selected_history_index = Some(self.scrape_history.len() - 1);
                                // --- End Success Case ---
                            }
                            Err(error_ref) => {
                                // --- Error Case ---
                                log::error!("Scraping failed (Firecrowl): {}", error_ref);
                                self.error_message = Some(format!("{}", error_ref));
                                self.markdown_content = None;
                                self.selected_history_index = None;
                                self.is_displaying_result = false;
                                // --- End Error Case ---
                            }
                        }
                        promise_finished = true;
                    }
                }
                ActivePromise::Llm(promise) => {
                    if let Some(result_ref) = promise.ready() {
                         match result_ref {
                            Ok(api_resp_ref) => {
                                // Extract the inner LlmScrapeResponse
                                match &api_resp_ref.data {
                                    Some(llm_resp_ref) => {
                                        // --- Success Case (LLM) ---
                                        let history_item = HistoryItem {
                                            url: llm_resp_ref.url.clone(),
                                            markdown: llm_resp_ref.summary.clone(), // Use summary field
                                        };
                                        self.markdown_content = Some(llm_resp_ref.summary.clone());
                                        self.error_message = None;
                                        self.is_displaying_result = true;
                                        if self.scrape_history.last().map_or(true, |last| last.url != history_item.url) {
                                            self.scrape_history.push(history_item);
                                        }
                                        self.selected_history_index = Some(self.scrape_history.len() - 1);
                                        // --- End Success Case ---
                                    }
                                    None => {
                                        // --- Error Case (API OK, but no data) ---
                                        let err_msg = format!("LLM API Response successful but data field is None. Meta: {:?}", api_resp_ref.meta);
                                        log::error!("{}", err_msg);
                                        self.error_message = Some("API returned success but no data".to_string());
                                        self.markdown_content = None;
                                        self.selected_history_index = None;
                                        self.is_displaying_result = false;
                                        // --- End Error Case ---
                                    }
                                }
                            }
                            Err(error_ref) => {
                                // --- Error Case ---
                                log::error!("Scraping failed (LLM): {}", error_ref);
                                self.error_message = Some(format!("{}", error_ref));
                                self.markdown_content = None;
                                self.selected_history_index = None;
                                self.is_displaying_result = false;
                                // --- End Error Case ---
                            }
                        }
                        promise_finished = true;
                    }
                }
            }
        }

        // Clear the promise state if it finished in this frame
        if promise_finished {
            self.scrape_promise = None;
        }
        // --- End Handle Promise Resolution ---

        // Determine if currently loading by checking the inner promise
        let is_loading = self.scrape_promise.as_ref().map_or(false, |active_promise| {
            match active_promise {
                ActivePromise::Firecrowl(promise) => promise.ready().is_none(),
                ActivePromise::Llm(promise) => promise.ready().is_none(),
            }
        });

        // --- Top Panel ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.heading("Ruscraper");
            });
        });

        // --- Left Panel (History) ---
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
                                    self.input_url = item.url.clone();
                                }
                                // NYI Buttons
                                ui.add_enabled(false, egui::Button::new("MD").small()).on_hover_text("Export Markdown (NYI)");
                                ui.add_enabled(false, egui::Button::new("PDF").small()).on_hover_text("Export PDF (NYI)");
                                ui.add_enabled(false, egui::Button::new("🗑").small()).on_hover_text("Delete History Item (NYI)");
                            });
                        }
                    }
                });
            });

        // --- Bottom Panel (Input/Controls/Error) ---
        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(5.0);
                let panel_frame = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 5));
                panel_frame.show(ui, |ui| {
                    // Display Error Message
                    if let Some(err) = &self.error_message {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        ui.add_space(2.0);
                    }

                    // Log flag value for debugging
                    log::info!("Bottom panel redraw: is_displaying_result = {}", self.is_displaying_result);

                    // Show EITHER the "New" button OR the input row
                    if self.is_displaying_result {
                        // Wrap "New +" button in a horizontal layout for consistent padding
                        ui.horizontal(|ui| {
                            let new_button = egui::Button::new("➕ New").min_size(egui::vec2(100.0, 35.0));
                            if ui.add(new_button).clicked() {
                                // Reset state for a new scrape
                                self.input_url.clear();
                                self.markdown_content = None;
                                self.error_message = None;
                                self.selected_history_index = None;
                                self.is_displaying_result = false;
                            }
                        });
                    } else {
                        // Show input elements when ready for new scrape or loading
                        ui.horizontal(|ui| {
                            let available_width = ui.available_width();
                            let button_width = 100.0;
                            let combo_width = 120.0;
                            let spacing = ui.spacing().item_spacing.x * 2.0;
                            let desired_input_width = (available_width - button_width - combo_width - spacing).max(50.0);
                            let widget_height = 35.0;

                            // --- URL Input ---
                            let mut trigger_scrape = false;
                            let url_input_enabled = !is_loading; // Only disable if actively loading
                            let url_input_response = ui.add_enabled(
                                url_input_enabled,
                                egui::TextEdit::singleline(&mut self.input_url)
                                    .desired_width(desired_input_width)
                                    .min_size(egui::vec2(0.0, widget_height))
                                    .hint_text("Enter URL to scrape..."),
                            );
                            if url_input_enabled && url_input_response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                                trigger_scrape = true;
                            }

                            // --- Scraper ComboBox ---
                            let combo_enabled = !is_loading;
                            ui.add_enabled_ui(combo_enabled, |ui| {
                                // Allocate space and center the ComboBox
                                let (id, rect) = ui.allocate_space(egui::vec2(combo_width, widget_height));
                                // Reverted back to allocate_ui_at_rect to fix compile error
                                ui.allocate_ui_at_rect(rect, |ui| {
                                    ui.centered_and_justified(|ui| {
                                        ComboBox::from_id_salt(id)
                                            .selected_text(format!("{}", self.selected_scraper))
                                            .width(rect.width())
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut self.selected_scraper, ScraperType::Firecrowl, ScraperType::Firecrowl.to_string());
                                                ui.selectable_value(&mut self.selected_scraper, ScraperType::LLM, ScraperType::LLM.to_string());
                                            });
                                    });
                                });
                            });

                            // --- Scrape Button ---
                            let scrape_button_enabled = !is_loading && !self.input_url.trim().is_empty();
                            let button_text = if is_loading { "..." } else { "Scrape" };
                            let scrape_button = egui::Button::new(button_text).min_size(egui::vec2(button_width, widget_height));
                            if ui.add_enabled(scrape_button_enabled, scrape_button).clicked() {
                                trigger_scrape = true;
                            }

                            // --- Trigger Scrape Action ---
                            if trigger_scrape && scrape_button_enabled { // Ensure button *was* enabled
                                if !self.input_url.trim().is_empty() {
                                    log::info!("Scrape triggered for URL: {}", self.input_url);
                                    self.error_message = None;
                                    self.markdown_content = None;
                                    self.selected_history_index = None;

                                    // --- Create and Spawn Promise ---
                                    let active_promise_enum = match self.selected_scraper {
                                        ScraperType::Firecrowl => {
                                            let base_url = FIRECROWL_URL;
                                            let request_url = format!("{}/scrape", base_url);
                                            log::info!("Requesting Firecrowl POST scrape to: {}", request_url);
                                            let request_body = serde_json::json!({ "url": self.input_url });
                                            let headers = ehttp::Headers::new(&[("Content-Type", "application/json")]);
                                            let mut request = ehttp::Request::post(request_url, request_body.to_string().into_bytes());
                                            request.headers = headers;

                                            let promise = spawn_scrape_promise::<FirecrowlScrapeResponse>(ctx, request);
                                            // Wrap in enum variant
                                            ActivePromise::Firecrowl(promise)
                                        }
                                        ScraperType::LLM => {
                                            let base_url = LLM_SCRAPER_URL;
                                            let request_url = format!("{}/api/scrape", base_url);
                                            log::info!("Requesting LLM POST scrape to: {}", request_url);
                                            let request_body = serde_json::json!({ "url": self.input_url });
                                            let headers = ehttp::Headers::new(&[("Content-Type", "application/json")]);
                                            let mut request = ehttp::Request::post(request_url, request_body.to_string().into_bytes());
                                            request.headers = headers;

                                            let promise = spawn_scrape_promise::<LlmApiResponse<LlmScrapeResponse>>(ctx, request);
                                            // Wrap in enum variant
                                            ActivePromise::Llm(promise)
                                        }
                                    };
                                    self.scrape_promise = Some(active_promise_enum);
                                    // --- End Promise Creation ---
                                } else {
                                    // This case should be prevented by button enablement, but handle defensively
                                    self.error_message = Some("Please enter a URL.".to_string());
                                }
                            }
                        }); // End horizontal layout for input row
                    } // End if/else for is_displaying_result

                    // --- Footer Row ---
                    ui.horizontal(|ui| {
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
                }); // End panel_frame.show
                ui.add_space(5.0);
            }); // End bottom panel show

        // --- Central Panel (Markdown Output) ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Scraped Content");
                if self.is_displaying_result {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        // Placeholder Export Buttons
                        if ui.button("Ⓜ️ MD").on_hover_text("Export as Markdown (NYI)").clicked() {
                             if let Some(content) = &self.markdown_content {
                                 save_markdown_file("scraped_content.md", content);
                             }
                        }
                        if ui.button("📄 PDF").on_hover_text("Export as PDF (NYI)").clicked() {
                            if let Some(content) = &self.markdown_content {
                                save_pdf_file("scraped_content.pdf", content);
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
                                .unwrap_or("Scraped content will appear here...

Enter a URL below and click Scrape.");
                            CommonMarkViewer::new()
                                .show(ui, &mut egui_commonmark::CommonMarkCache::default(), display_text);
                        }
                    });
            });
        });
    } // End update fn
} // End impl eframe::App


// --- Helper function to spawn the scrape promise ---
// Returns a promise for the direct deserialized type T
fn spawn_scrape_promise<T: 'static + Send>(
    _ctx: &egui::Context, // Use underscore for unused parameter
    request: ehttp::Request,
) -> Promise<Result<T, FrontendError>> // Return Result<T, FrontendError>
where
    T: for<'de> Deserialize<'de> + Clone,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        let request_clone = request.clone();
        Promise::spawn_thread("ehttp_fetch_native", move || {
            futures::executor::block_on(async {
                ehttp::fetch_async(request_clone)
                    .await
                    .map_err(FrontendError::from)
                    .and_then(|response| {
                        if response.ok {
                            let body_bytes_for_log = response.bytes.clone();
                            if let Ok(text) = std::str::from_utf8(&body_bytes_for_log) {
                                log::info!("Attempting to parse JSON response: {}", text);
                            } else {
                                log::warn!("Received non-UTF8 response body before parsing.");
                            }

                            // Attempt to parse directly into T
                            serde_json::from_slice::<T>(&response.bytes)
                                .map_err(|e| {
                                    log::error!("JSON parsing failed: {:?}. Raw response logged above.", e);
                                    FrontendError::JsonParse(e)
                                })
                        } else {
                            let err_msg = format!(
                                "API request failed with status {}: {}",
                                response.status, response.status_text
                            );
                            log::error!("{}", err_msg);
                            Err(FrontendError::ApiError(err_msg))
                        }
                    })
            })
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ctx_clone = _ctx.clone();
         Promise::spawn_async(async move {
            ehttp::fetch_async(request)
                .await
                .map_err(FrontendError::from)
                .and_then(|response| {
                    if response.ok {
                        let body_bytes_for_log = response.bytes.clone();
                        if let Ok(text) = std::str::from_utf8(&body_bytes_for_log) {
                            log::info!("Attempting to parse JSON response: {}", text);
                        } else {
                            log::warn!("Received non-UTF8 response body before parsing.");
                        }

                        // Attempt to parse directly into T
                         serde_json::from_slice::<T>(&response.bytes)
                            .map_err(|e| {
                                log::error!("JSON parsing failed: {:?}. Raw response logged above.", e);
                                FrontendError::JsonParse(e)
                            })
                    } else {
                        let err_msg = format!(
                            "API request failed with status {}: {}",
                            response.status, response.status_text
                        );
                        log::error!("{}", err_msg);
                        Err(FrontendError::ApiError(err_msg))
                    }
                })
        })
    }
}


// ---- Helper Functions for Saving Files ----
// (These remain outside the impl eframe::App block)

fn save_markdown_file(filename: &str, content: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
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
        trigger_download(filename, content);
    }
}

fn save_pdf_file(filename: &str, content: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
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
        log::warn!("WASM PDF export is basic: downloading raw text with .pdf extension.");
        trigger_download(filename, content);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn create_basic_pdf(content: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let (doc, page1, layer1) = PdfDocument::new("Scraped Content", Mm(210.0), Mm(297.0), "Layer 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);
    let font = doc.add_builtin_font(printpdf::BuiltinFont::Helvetica)?;
    let font_size = 10.0;
    let line_height = 12.0;
    let margin_top = 280.0;
    let margin_bottom = 15.0;
    let mut y_position = margin_top;
    current_layer.set_font(&font, font_size);
    for line in content.lines() {
        if y_position < margin_bottom {
            log::warn!("PDF content truncated due to reaching page bottom.");
            break;
        }
        current_layer.use_text(line.to_string(), font_size, Mm(10.0), Mm(y_position), &font);
        y_position -= line_height;
    }
    let pdf_bytes = doc.save_to_bytes()?;
    Ok(pdf_bytes)
}

#[cfg(target_arch = "wasm32")]
fn trigger_download(filename: &str, content: &str) {
    use base64::{engine::general_purpose, Engine as _};

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().expect("document should have a body");

    let link = document
        .create_element("a")
        .expect("Failed to create anchor element")
        .dyn_into::<HtmlAnchorElement>()
        .expect("Failed to cast to HtmlAnchorElement");

    let base64_content = general_purpose::STANDARD.encode(content);
    let mime_type = if filename.ends_with(".pdf") { "text/plain" } else { "text/markdown" };
    let href = format!("data:{};charset=utf-8;base64,{}", mime_type, base64_content);

    link.set_href(&href);
    link.set_download(filename);

    let style = link.style();
    style.set_property("display", "none").expect("Failed to set style");
    body.append_child(&link).expect("Failed to append link");
    link.click();
    body.remove_child(&link).expect("Failed to remove link");
    log::info!("Triggered download for {}", filename);
}
