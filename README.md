# Ivoclar Offline Demo

A Rust web scraping utility that downloads Ivoclar website pages with JavaScript-rendered content and all assets for complete offline viewing. Includes a built-in server to view pages offline.

## Overview

Many modern websites render content dynamically via JavaScript, making traditional `wget` scraping insufficient. This tool uses Selenium WebDriver to:

1. Load pages in a real Firefox browser
2. Wait for JavaScript to hydrate the content
3. Hook all network requests via the Performance API
4. Download all assets (CSS, JS, images, fonts, etc.)
5. Serve everything locally for offline viewing

## Features

- **JavaScript hydration** - Captures dynamically loaded content that static scrapers miss
- **Complete asset capture** - Downloads CSS, JavaScript, images, fonts, and media files
- **Performance API hooking** - Intercepts all network requests made by the browser
- **DOM inspection** - Also scans `<img>`, `<script>`, `<link>`, `<video>`, `<audio>` tags
- **Background image extraction** - Captures inline CSS background images
- **Deduplication** - Avoids re-downloading assets across multiple pages
- **URL normalization** - Handles absolute, relative, and protocol-relative URLs
- **Directory preservation** - Maintains the original URL structure locally
- **Built-in server** - Serve downloaded pages without external tools

## Requirements

- [Rust](https://rustup.rs/) (1.85+ for Edition 2024)
- [geckodriver](https://github.com/mozilla/geckodriver) - Firefox WebDriver (for scraping)
- [Firefox](https://www.mozilla.org/firefox/) browser (for scraping)

## Usage

```bash
# Scrape pages (requires geckodriver running)
cargo run --bin scrape

# Serve downloaded pages offline
cargo run --bin serve
cargo run --bin serve -- 3000  # custom port
```

### As a Library

```rust
use ivoclar_offline::{Scraper, serve};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Scrape pages
    let scraper = Scraper::new("https://example.com").await?;
    scraper.scrape_page("path/to/page").await?;
    let total = scraper.finish().await?;

    // Serve offline
    serve(8080).await?;
    Ok(())
}
```

## Quick Start

### Option 1: Use Pre-built Archive

```bash
unzip page.zip
cargo run --bin serve
# Open http://127.0.0.1:8080/en_us/ids/
```

### Option 2: Scrape Fresh Content

1. **Start geckodriver:**
   ```bash
   geckodriver
   ```

2. **Scrape the pages** (in another terminal):
   ```bash
   cargo run --bin scrape
   ```

   Output:
   ```
   Scraping: https://www.ivoclar.com/en_us/ids
     Found 89 resources
     Downloaded 37 new assets
     Saved HTML
   Scraping: https://www.ivoclar.com/en_us/ids/workflows
     Found 70 resources
     Downloaded 3 new assets
     Saved HTML
   ...
   Complete! Downloaded 40 unique assets.
   Run `cargo run --bin serve` to view offline.
   ```

3. **Serve the offline pages:**
   ```bash
   cargo run --bin serve
   ```

4. **View the result:**
   Open http://127.0.0.1:8080/en_us/ids/ in your browser.

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Rust App  │────▶│ geckodriver │────▶│   Firefox   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                                       │
       │                                       ▼
       │                              ┌─────────────────┐
       │                              │ ivoclar.com     │
       │◀──────────────────────────── │ (JS hydration)  │
       │   Performance API            └─────────────────┘
       │   (all network requests)
       ▼
┌─────────────────┐
│   page/         │
│   ├── en_us/ids/index.html
│   ├── css/*.css
│   ├── js/*.js
│   ├── images/*
│   └── fonts/*
└─────────────────┘
```

## How It Works

1. **WebDriver Connection** - Connects to geckodriver running on port 4444
2. **Page Navigation** - Opens each target URL in Firefox
3. **JS Hydration** - Waits 2 seconds for dynamic content to load
4. **Performance API** - Queries `performance.getEntriesByType('resource')` to get all network requests
5. **DOM Scanning** - Additionally scans HTML for asset references in tags
6. **Asset Download** - Downloads all unique assets via HTTP
7. **Local Storage** - Saves everything preserving the URL path structure

## Asset Types Captured

| Source | Assets |
|--------|--------|
| Performance API | All network requests (CSS, JS, fonts, images, XHR responses) |
| `<img>` tags | `src`, `data-src`, `srcset` attributes |
| `<script>` tags | External JavaScript files |
| `<link>` tags | Stylesheets, preloads, icons |
| `<video>`/`<audio>` | Media sources and posters |
| Inline styles | Background images via `url()` |

## Project Structure

```
.
├── Cargo.toml          # Package manifest with lib + 2 binaries
├── src/
│   ├── lib.rs          # Core library (Scraper, serve, utilities)
│   └── bin/
│       ├── scrape.rs   # Scraping binary
│       └── serve.rs    # Server binary
├── Makefile            # Convenience commands
├── page.zip            # Pre-scraped content archive
└── README.md
```

## License

MIT
