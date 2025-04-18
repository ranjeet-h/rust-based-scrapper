use crate::processing::{fetch_and_convert, ProcessingError};
use poll_promise::Promise;

#[cfg(not(target_arch = "wasm32"))]
use futures::executor::block_on;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // State for the web scraper app
    input_url: String,
    #[serde(skip)] // Avoid serializing potentially large markdown data or promises
    markdown_content: Option<String>,
    #[serde(skip)]
    error_message: Option<String>,
    #[serde(skip)]
    scrape_promise: Option<Promise<Result<String, ProcessingError>>>,
    #[serde(skip)] // Don't persist history for now
    scrape_history: Vec<(String, String)>, // Store (URL, Title/Summary)
    #[serde(skip)]
    selected_history_index: Option<usize>, // Index of the currently viewed history item
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            input_url: "".to_owned(),
            markdown_content: None,
            error_message: None,
            scrape_promise: None,
            scrape_history: Vec::new(),   // Initialize history
            selected_history_index: None, // Nothing selected initially
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        /* <-- Temporarily disable loading state --> 
        if let Some(storage) = cc.storage {
            // Try to load previous state
            if let Some(state) = eframe::get_value(storage, eframe::APP_KEY) {
                return state; // Return loaded state
            }
        }
        */

        // If loading failed or was disabled, create a default state
        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Handle Promise Resolution ---
        if let Some(promise) = &self.scrape_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(markdown) => {
                        let url = self.input_url.clone(); // Assuming input_url holds the URL for the finished promise
                        self.markdown_content = Some(markdown.clone());
                        self.error_message = None;
                        // Add to history (use URL as title for now)
                        // Prevent duplicates if the exact same URL was just scraped
                        if self
                            .scrape_history
                            .last()
                            .map_or(true, |(last_url, _)| last_url != &url)
                        {
                            // Store the full markdown for now to allow switching back
                            // Consider storing only a summary later for performance
                            self.scrape_history.push((url.clone(), markdown.clone()));
                        }
                        // Automatically select the latest history item
                        self.selected_history_index = Some(self.scrape_history.len() - 1);
                    }
                    Err(e) => {
                        log::error!("Scraping failed: {}", e);
                        self.error_message = Some(format!("{}", e));
                        self.markdown_content = None; // Clear content on error
                        self.selected_history_index = None; // De-select history on error
                    }
                }
                // Promise is finished, remove it
                self.scrape_promise = None;
            }
        }
        // --- End Handle Promise Resolution ---

        // Determine if currently loading
        let is_loading = self.scrape_promise.is_some()
            && self
                .scrape_promise
                .as_ref()
                .map_or(false, |p| p.ready().is_none());

        // --- Top Panel for Header ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.heading("Ruscraper");
                // ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                //     // Quit Button for non-web
                //     let is_web = cfg!(target_arch = "wasm32");
                //     if !is_web {
                //         if ui.button("Quit").clicked() {
                //             ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                //         }
                //     }
                //     // Theme switcher
                //     // egui::widgets::global_theme_preference_buttons(ui);
                //     ui.add_space(16.0);
                // });
            });
        });
        // --- End Top Panel ---

        // --- Left Panel for History ---
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
                            let (url, _content) = &self.scrape_history[i];
                            // Use URL as the label for now
                            let display_url = url.splitn(4, '/').nth(2).unwrap_or(url).to_string(); // Show domain or full url
                            let label_text = format!("{}: {}", i + 1, display_url);

                            // Highlight the selected item
                            let is_selected = self.selected_history_index == Some(i);

                            ui.horizontal(|ui| {
                                // Selectable Label takes up most space
                                if ui.selectable_label(is_selected, label_text).clicked() {
                                    self.selected_history_index = Some(i);
                                    // Update main content view when history item is clicked
                                    self.markdown_content = Some(self.scrape_history[i].1.clone());
                                    self.error_message = None; // Clear error when viewing history
                                    self.input_url = self.scrape_history[i].0.clone();
                                    // Optionally update input field
                                }

                                // Action buttons for the history item (only if selected for now? or always? Let's show always)
                                ui.add_enabled(!is_loading, egui::Button::new("MD").small())
                                    .on_hover_text("Export Markdown")
                                    .clicked()
                                    .then(|| {
                                        log::info!("Export MD clicked for index {}", i);
                                        // TODO: Implement MD export for scrape_history[i]
                                    });
                                ui.add_enabled(!is_loading, egui::Button::new("PDF").small())
                                    .on_hover_text("Export PDF")
                                    .clicked()
                                    .then(|| {
                                        log::info!("Export PDF clicked for index {}", i);
                                        self.error_message =
                                            Some("PDF export not implemented yet.".to_string());
                                        // TODO: Implement PDF export for scrape_history[i]
                                    });
                                ui.add_enabled(!is_loading, egui::Button::new("🗑").small())
                                    .on_hover_text("Delete History Item")
                                    .clicked()
                                    .then(|| {
                                        log::warn!("Delete clicked for index {}", i);
                                        // TODO: Implement deletion logic (need mutable borrow or delayed action)
                                        // self.scrape_history.remove(i); // Cannot borrow mutably here
                                        self.error_message =
                                            Some("Delete not implemented yet.".to_string());
                                    });
                            }); // End horizontal layout for item + buttons
                        }
                    }
                });
            });
        // --- End Left Panel ---

        // --- Bottom Panel for Input, Controls, and Errors ---
        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false) // Typically fixed height
            .show(ctx, |ui| {
                // Add some padding/margin around the panel contents
                ui.add_space(5.0);
                let panel_frame = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 5));
                panel_frame.show(ui, |ui| {
                    // Display Error Messages just above the input
                    if let Some(err) = &self.error_message {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        ui.add_space(2.0);
                    }

                    // Input Row: URL field and Scrape button
                    ui.horizontal(|ui| {
                        // --- URL Input takes remaining space ---
                        let available_width = ui.available_width();
                        let desired_input_width = (available_width - 100.0).max(0.0); // Ensure non-negative width
                        let input_height = 35.0;
                        
                        ui.add_enabled_ui(!is_loading, |ui| {
                            let mut text_edit = egui::TextEdit::singleline(&mut self.input_url)
                                .hint_text("Enter URL to scrape...");
                            // Use add_sized within the enabled_ui closure
                            let url_input_response = ui.add_sized([desired_input_width, input_height], text_edit);
                            // Store response outside the closure if needed later (e.g., for focus check)
                            ui.memory_mut(|mem| mem.data.insert_temp("url_input_response_id".into(), url_input_response));
                        });
                        // Retrieve the response
                        let url_input_response: egui::Response = ui.memory_mut(|mem| mem.data.get_temp("url_input_response_id".into()).unwrap());

                        // --- End URL Input ---
                        
                        // --- Button on the right first ---
                        let mut button_clicked = false; // Initialize placeholder
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            // Scrape Button (takes its natural width)
                            let scrape_button =
                                egui::Button::new(if is_loading { "..." } else { "Send" })
                                    .min_size(egui::vec2(100.0, 35.0)) // Set minimum height, natural width
                                    .corner_radius(egui::epaint::CornerRadius::same(
                                        ui.style().visuals.widgets.inactive.corner_radius.ne,
                                    ));

                            // Add the button and capture if it was clicked
                            button_clicked = ui
                                .add_enabled(
                                    !is_loading && !self.input_url.trim().is_empty(),
                                    scrape_button,
                                )
                                .on_hover_text("Fetch content and convert to Markdown")
                                .clicked();
                        });
                        // --- End Button Layout ---

                        // Check if enter was pressed in the text input OR the button was clicked
                        let enter_pressed = url_input_response.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        if button_clicked || enter_pressed {
                            // Set loading state implicitly by creating the promise
                            self.error_message = None;
                            self.markdown_content = None; // Clear display while loading
                            self.selected_history_index = None; // De-select history when starting new scrape
                            let url_to_fetch = self.input_url.clone();
                            log::info!("Spawning promise to fetch URL: {}", url_to_fetch); // Use info level log
                                                                                           // Create and store the promise
                            #[cfg(target_arch = "wasm32")]
                            {
                                self.scrape_promise = Some(Promise::spawn_async(async move {
                                    fetch_and_convert(url_to_fetch).await
                                }));
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                self.scrape_promise =
                                    Some(Promise::spawn_thread("scrape", move || {
                                        // #[cfg(feature = "tokio")] // Example if using tokio runtime directly - Removed
                                        // let _rt = tokio::runtime::Runtime::new().unwrap(); // Ensure runtime exists if needed by block_on - Removed
                                        // Use futures::executor::block_on for simplicity if no async runtime needed directly
                                        block_on(fetch_and_convert(url_to_fetch))
                                    }));
                            }
                        }
                        // Loading indicator - now handled by button text/disabled state
                        // if is_loading { ui.add(egui::Spinner::new()); }
                    });
                    ui.add_space(5.0);

                    // Combined Row for Export Buttons and Theme/Footer
                    ui.horizontal(|ui| {
                        // Enable buttons only if there's content *from the selected history* and not loading
                        let enable_export = self.selected_history_index.is_some() && !is_loading;

                        // Remove the global export buttons, actions are now per-history item
                        // if ui.add_enabled(enable_export, egui::Button::new("Export Markdown (.md)")).clicked() { ... }
                        // if ui.add_enabled(enable_export, egui::Button::new("Export PDF")).clicked() { ... }

                        // Right-align theme/footer elements
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Footer with egui/eframe attribution
                            // powered_by_egui_and_eframe(ui);
                            ui.add_space(10.0);
                            // Theme switcher
                            egui::widgets::global_theme_preference_buttons(ui);
                            // Quit Button for non-web
                            let is_web = cfg!(target_arch = "wasm32");
                            if !is_web {
                                if ui.button("Quit").clicked() {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            }
                        });
                    });
                }); // End inner frame
                ui.add_space(5.0); // Add some padding below the panel contents
            });
        // --- End Bottom Panel ---

        // --- Central Panel for the Markdown Output ---
        egui::CentralPanel::default().show(ctx, |ui| {
            // Add a subtle frame/background
            egui::Frame::group(ui.style()).show(ui, |ui| {
                 egui::ScrollArea::vertical()
                     .stick_to_bottom(true)
                     .auto_shrink([false, false]) // Fill available space
                     .show(ui, |ui| {
                     // Display the markdown content based on selection, or a placeholder
                     let display_text = if let Some(index) = self.selected_history_index {
                         self.scrape_history[index].1.as_str() // Show content from history
                     } else if self.markdown_content.is_some() {
                         // This case might happen briefly after scrape before selection updates
                         self.markdown_content.as_deref().unwrap_or("")
                     } else {
                         "Scraped content will appear here...\n\nEnter a URL below and click scrape."
                     };

                     // Use a Label or TextEdit for display. TextEdit allows selection.
                     // For pure display, Label might be slightly more performant.
                     // Let's use read-only TextEdit for consistency and text selection.
                     let mut display_mut = display_text.to_string(); // Need mutable for TextEdit
                     ui.add_sized(ui.available_size(), // Fill the central panel
                         egui::TextEdit::multiline(&mut display_mut)
                             .interactive(false) // Make it read-only
                             .frame(false) // No frame inside the scroll area frame
                             .code_editor() // Use a monospace font potentially
                     );
                 });
            });
        });
        // --- End Central Panel ---
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
