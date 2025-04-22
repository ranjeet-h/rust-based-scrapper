# rust-based-scrapper

This repository contains a web scraping application built with Rust, consisting of a frontend UI, a backend API service, and potentially a separate core scraping library/service.

## Project Structure

The project is organized into three main directories:

```
.
├── backend/         # Axum backend service using Firecrawl and SQLite
├── frontend/        # Egui frontend application (compiles to native and WASM)
└── rust-web-scrapper/ # Another Axum service, potentially core scraping logic
```

### 1. `backend/`

This directory contains an Axum-based backend service responsible for handling scraping requests, storing results, and serving data potentially to the frontend.

**Key Files:**

*   `Cargo.toml`: Defines dependencies like `axum`, `sqlx` (for SQLite), `reqwest`, `scraper`, `firecrawl`, `tokio`, `serde`, `dotenvy`, `tracing`.
*   `.env`: Configuration file for environment variables (e.g., `DATABASE_URL`, `FIRECRAWL_API_KEY`). **Remember to create and populate this file.**
*   `ruscraper.db`: SQLite database file where scraped data is stored.
*   `src/main.rs`:
    *   Entry point for the backend server.
    *   Initializes database connection (SQLite) and creates the `scraped_items` table.
    *   Initializes the Firecrawl client using an API key from `.env`.
    *   Sets up Axum router with the following endpoints:
        *   `POST /scrape`: Accepts a URL, uses Firecrawl to fetch Markdown content, caches the result in the database, and returns the scraped content.
        *   `GET /history`: Returns a list of all previously scraped items from the database.
        *   `GET /history/:id`: Returns a specific scraped item by its ID.
    *   Configures CORS and tracing.
    *   Starts the server, typically on `127.0.0.1:8000`.

### 2. `frontend/`

This directory contains the user interface for the application, built using the `egui` and `eframe` Rust libraries. It can be compiled both as a native desktop application and a WebAssembly (WASM) application to run in a web browser.

**Key Files:**

*   `Cargo.toml`: Defines dependencies like `egui`, `eframe`, `reqwest`/`ehttp` (for making HTTP requests to the backend), `serde`, `chrono`, `egui_commonmark` (for displaying Markdown), `rfd` (native file dialogs), `printpdf` (PDF export). Includes configuration for both native and WASM targets.
*   `index.html`: The HTML host page for the WASM version of the application. Includes a canvas (`the_canvas_id`) for `egui` rendering and a loading indicator. Used with `trunk` for building and serving the WASM app.
*   `assets/`: Contains static assets like icons (`icon-256.png`).
*   `src/main.rs`: Entry point that handles the bootstrapping for both native and WASM environments. Initializes logging and the `eframe` application runner, loading the main application logic from `frontend_app::TemplateApp`.
*   `src/app.rs`: (Assumed) Contains the main application logic and UI definition using `egui`. Likely handles user input (like URLs to scrape), sends requests to the `backend` API, displays results (including Markdown), and manages application state.
*   `src/processing.rs`: (Assumed) May contain helper functions for data processing related to the frontend.
*   `check.sh`, `fill_template.sh`, `fill_template.ps1`: Utility scripts, possibly for development tasks like checking code, filling templates, or build steps.
*   `dist/`: (Typically) Output directory where `trunk` places the built WASM application files.

### 3. `rust-web-scrapper/`

This directory contains another Rust project, also seemingly an Axum-based web service. Its exact purpose relative to the `backend/` service needs further clarification, but it appears to provide core scraping or processing capabilities.

**Key Files:**

*   `Cargo.toml`: Defines dependencies, likely including `axum`, `tokio`, `serde`, potentially `reqwest` or other scraping libraries.
*   `.env`: Configuration file for this specific service.
*   `README.md`: Contains specific documentation for this sub-project.
*   `src/main.rs`: Entry point for this Axum server. Loads configuration (`Config`), sets up application state (including a simple cache), creates routes using `api::routes::create_router`, and starts the server.
*   `src/lib.rs`: Library entry point, defining shared structures like `AppState`.
*   `src/config.rs`: Defines the `Config` struct for loading application configuration.
*   `src/scraper.rs`: (Assumed) Contains logic related to web scraping.
*   `src/llm.rs`: (Assumed) Contains logic related to interacting with Large Language Models (LLMs), possibly for processing scraped content.
*   `src/error.rs`: Defines custom error types for this service.
*   `src/api/`: (Assumed Directory) Contains modules defining the API routes and handlers.

## Setup and Running

*(Detailed setup instructions should be added here based on how the components interact and are built/run. This typically involves steps like)*

1.  **Prerequisites:** Install Rust (`rustup`), potentially `trunk` for the WASM frontend (`cargo install trunk`).
2.  **Backend Setup:**
    *   Navigate to the `backend/` directory.
    *   Create a `.env` file based on `.env.example` (if available) or the required variables (`DATABASE_URL=sqlite:ruscraper.db`, `FIRECRAWL_API_KEY=YOUR_KEY`).
    *   Run `cargo run`.
3.  **rust-web-scrapper Setup:**
    *   Navigate to the `rust-web-scrapper/` directory.
    *   Create a `.env` file if needed.
    *   Run `cargo run`.
4.  **Frontend Setup (WASM):**
    *   Navigate to the `frontend/` directory.
    *   Run `trunk serve --open` to build and serve the WASM app locally, opening it in your browser.
5.  **Frontend Setup (Native):**
    *   Navigate to the `frontend/` directory.
    *   Run `cargo run`.

*(Adjust ports and specific commands as needed)*

## Usage

*(Describe how to use the application, e.g., opening the frontend, entering a URL, viewing history, etc.)*
