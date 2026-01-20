//! # Ivoclar Offline
//!
//! A library for scraping JavaScript-rendered web pages and serving them offline.
//!
//! This crate provides two main capabilities:
//!
//! 1. **Scraping** - Download web pages with all their assets (CSS, JS, images, fonts)
//!    using Selenium WebDriver to capture dynamically rendered content.
//!
//! 2. **Serving** - Serve the downloaded pages locally for offline viewing.
//!
//! ## Example
//!
//! ```no_run
//! use ivoclar_offline::{Scraper, serve};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Scrape pages
//!     let scraper = Scraper::new("https://example.com").await?;
//!     scraper.scrape_page("path/to/page").await?;
//!     let total = scraper.finish().await?;
//!     println!("Downloaded {total} assets");
//!
//!     // Serve offline
//!     serve(8080).await?;
//!     Ok(())
//! }
//! ```

use std::{
    collections::HashSet,
    error::Error,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::Router;
use thirtyfour::{By, DesiredCapabilities, WebDriver};
use tokio::fs;
use tower_http::services::ServeDir;

/// Shared set of already-downloaded URLs to avoid duplicates across pages.
type DownloadedUrls = Arc<Mutex<HashSet<String>>>;

/// A web scraper that captures JavaScript-rendered pages and their assets.
///
/// Uses Selenium WebDriver to load pages in a real browser, wait for JavaScript
/// to hydrate the content, then captures all network requests via the Performance API.
pub struct Scraper {
    driver: WebDriver,
    client: reqwest::Client,
    downloaded: DownloadedUrls,
    website: String,
}

impl Scraper {
    /// Creates a new scraper connected to geckodriver.
    ///
    /// Requires geckodriver to be running on `http://127.0.0.1:4444`.
    ///
    /// # Arguments
    ///
    /// * `website` - The base URL of the website to scrape (e.g., "https://example.com")
    ///
    /// # Errors
    ///
    /// Returns an error if the WebDriver connection fails.
    pub async fn new(website: &str) -> Result<Self, Box<dyn Error>> {
        let caps = DesiredCapabilities::firefox();
        let driver = WebDriver::new("http://127.0.0.1:4444", caps).await?;
        let client = reqwest::Client::new();
        let downloaded = Arc::new(Mutex::new(HashSet::new()));

        Ok(Self {
            driver,
            client,
            downloaded,
            website: website.to_string(),
        })
    }

    /// Scrapes a page and all its assets.
    ///
    /// # Arguments
    ///
    /// * `path` - The relative path to scrape (e.g., "en_us/ids")
    ///
    /// # Returns
    ///
    /// The number of new assets downloaded for this page.
    pub async fn scrape_page(&self, path: &str) -> Result<usize, Box<dyn Error>> {
        println!("Scraping: {}/{path}", self.website);
        self.driver.goto(format!("{}/{path}", self.website)).await?;

        // Wait for dynamic content to load
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Collect all resource URLs
        let urls = collect_resource_urls(&self.driver).await?;
        println!("  Found {} resources", urls.len());

        // Download all assets
        let mut downloaded_count = 0;
        for url in &urls {
            if download_asset(&self.client, url, &self.website, &self.downloaded).await {
                downloaded_count += 1;
            }
        }
        println!("  Downloaded {downloaded_count} new assets");

        // Save the HTML
        fs::create_dir_all(format!("page/{path}")).await?;
        let source = self.driver.source().await?;
        fs::write(format!("page/{path}/index.html"), source.as_bytes()).await?;
        println!("  Saved HTML");

        Ok(downloaded_count)
    }

    /// Returns the total number of unique assets downloaded.
    pub fn total_assets(&self) -> usize {
        self.downloaded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Closes the browser and returns the total number of assets downloaded.
    pub async fn finish(self) -> Result<usize, Box<dyn Error>> {
        let total = self.total_assets();
        self.driver.quit().await?;
        Ok(total)
    }
}

/// Starts a static file server to serve the downloaded pages.
///
/// Serves files from the `page/` directory.
///
/// # Arguments
///
/// * `port` - The port to listen on
pub async fn serve(port: u16) -> Result<(), Box<dyn Error>> {
    let app = Router::new().fallback_service(ServeDir::new("page"));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Serving offline pages at http://{addr}");
    println!("Open http://{addr}/en_us/ids/ in your browser");
    println!("Press Ctrl+C to stop");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Normalizes a URL path for local storage.
///
/// Handles:
/// - Absolute URLs (extracts path)
/// - Protocol-relative URLs (//example.com/path)
/// - Root-relative URLs (/path)
/// - Query strings (strips them for cleaner paths)
pub fn normalize_url_path(url: &str, website: &str) -> Option<String> {
    let url = url.split('?').next().unwrap_or(url);
    let url = url.split('#').next().unwrap_or(url);

    if url.is_empty() || url.starts_with("data:") || url.starts_with("blob:") {
        return None;
    }

    let path = if url.starts_with("http://") || url.starts_with("https://") {
        url.strip_prefix(website)
            .map(|p| p.trim_start_matches('/').to_string())
    } else if let Some(stripped) = url.strip_prefix("//") {
        stripped
            .find('/')
            .map(|idx| stripped[idx..].trim_start_matches('/').to_string())
    } else {
        Some(url.trim_start_matches('/').to_string())
    };

    path.filter(|p| !p.is_empty())
}

/// Downloads an asset and saves it locally.
async fn download_asset(
    client: &reqwest::Client,
    url: &str,
    website: &str,
    downloaded: &DownloadedUrls,
) -> bool {
    let Some(local_path) = normalize_url_path(url, website) else {
        return false;
    };

    // Check if already downloaded
    {
        let urls = downloaded.lock().unwrap_or_else(|e| e.into_inner());
        if urls.contains(&local_path) {
            return false;
        }
    }

    // Build full URL for download
    let full_url = if url.starts_with("http") {
        url.to_string()
    } else if let Some(stripped) = url.strip_prefix("//") {
        format!("https://{stripped}")
    } else {
        format!("{website}/{}", url.trim_start_matches('/'))
    };

    // Download and save
    if let Ok(response) = client.get(&full_url).send().await
        && let Ok(bytes) = response.bytes().await
    {
        let file_path = PathBuf::from(format!("page/{local_path}"));
        if let Some(parent) = file_path.parent()
            && fs::create_dir_all(parent).await.is_ok()
            && fs::write(&file_path, &bytes).await.is_ok()
        {
            let mut urls = downloaded.lock().unwrap_or_else(|e| e.into_inner());
            urls.insert(local_path);
            return true;
        }
    }

    false
}

/// Collects all resource URLs from the page using the Performance API and DOM inspection.
async fn collect_resource_urls(driver: &WebDriver) -> Result<Vec<String>, Box<dyn Error>> {
    // Get all resources from Performance API
    let perf_script = r#"
        return performance.getEntriesByType('resource')
            .map(r => r.name)
            .filter(url => !url.startsWith('data:') && !url.startsWith('blob:'));
    "#;

    let perf_urls: Vec<String> =
        serde_json::from_value(driver.execute(perf_script, vec![]).await?.json().clone())
            .unwrap_or_default();

    let mut urls: HashSet<String> = perf_urls.into_iter().collect();

    // Collect from DOM elements
    let selectors = [
        ("img", "src"),
        ("img", "data-src"),
        ("script", "src"),
        ("link[rel='stylesheet']", "href"),
        ("link[rel='preload']", "href"),
        ("link[rel='icon']", "href"),
        ("video", "src"),
        ("video", "poster"),
        ("audio", "src"),
        ("source", "src"),
        ("source", "srcset"),
    ];

    for (selector, attr) in selectors {
        for elem in driver.find_all(By::Css(selector)).await.unwrap_or_default() {
            if let Ok(Some(value)) = elem.attr(attr).await {
                if attr == "srcset" {
                    for part in value.split(',') {
                        if let Some(url) = part.split_whitespace().next() {
                            urls.insert(url.to_string());
                        }
                    }
                } else {
                    urls.insert(value);
                }
            }
        }
    }

    // Get background images from inline styles
    let bg_script = r#"
        const urls = [];
        document.querySelectorAll('[style*="background"]').forEach(el => {
            const match = el.style.cssText.match(/url\(['"]?([^'")\s]+)['"]?\)/g);
            if (match) {
                match.forEach(m => {
                    const url = m.replace(/url\(['"]?|['"]?\)/g, '');
                    if (url && !url.startsWith('data:')) urls.push(url);
                });
            }
        });
        return urls;
    "#;

    if let Ok(bg_urls) = driver.execute(bg_script, vec![]).await
        && let Ok(bg_list) = serde_json::from_value::<Vec<String>>(bg_urls.json().clone())
    {
        urls.extend(bg_list);
    }

    Ok(urls.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEBSITE: &str = "https://www.ivoclar.com";

    #[test]
    fn test_normalize_relative_path() {
        let result = normalize_url_path("images/logo.png", WEBSITE);
        assert_eq!(result, Some("images/logo.png".to_string()));
    }

    #[test]
    fn test_normalize_root_relative_path() {
        let result = normalize_url_path("/assets/style.css", WEBSITE);
        assert_eq!(result, Some("assets/style.css".to_string()));
    }

    #[test]
    fn test_normalize_absolute_url_same_domain() {
        let result = normalize_url_path("https://www.ivoclar.com/js/app.js", WEBSITE);
        assert_eq!(result, Some("js/app.js".to_string()));
    }

    #[test]
    fn test_normalize_absolute_url_different_domain() {
        let result = normalize_url_path("https://cdn.example.com/lib.js", WEBSITE);
        assert_eq!(result, None);
    }

    #[test]
    fn test_normalize_protocol_relative_url() {
        let result = normalize_url_path("//fonts.googleapis.com/css?family=Roboto", WEBSITE);
        assert_eq!(result, Some("css".to_string()));
    }

    #[test]
    fn test_normalize_strips_query_string() {
        let result = normalize_url_path("/api/image.png?v=123&size=large", WEBSITE);
        assert_eq!(result, Some("api/image.png".to_string()));
    }

    #[test]
    fn test_normalize_strips_hash() {
        let result = normalize_url_path("/page/section#anchor", WEBSITE);
        assert_eq!(result, Some("page/section".to_string()));
    }

    #[test]
    fn test_normalize_rejects_data_url() {
        let result = normalize_url_path("data:image/png;base64,ABC123", WEBSITE);
        assert_eq!(result, None);
    }

    #[test]
    fn test_normalize_rejects_blob_url() {
        let result = normalize_url_path("blob:https://example.com/uuid", WEBSITE);
        assert_eq!(result, None);
    }

    #[test]
    fn test_normalize_rejects_empty() {
        let result = normalize_url_path("", WEBSITE);
        assert_eq!(result, None);
    }
}
