//! Scrapes Ivoclar IDS pages for offline viewing.
//!
//! Requires geckodriver to be running on port 4444.
//!
//! # Usage
//!
//! ```bash
//! geckodriver &
//! cargo run --bin scrape
//! ```

use std::error::Error;

use ivoclar_offline::Scraper;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let website = "https://www.ivoclar.com";
    let scraper = Scraper::new(website).await?;

    // Scrape the main IDS page and subpages
    scraper.scrape_page("en_us/ids").await?;
    scraper.scrape_page("en_us/ids/workflows").await?;
    scraper.scrape_page("en_us/ids/product-highlights").await?;

    let total = scraper.finish().await?;
    println!("\nComplete! Downloaded {total} unique assets.");
    println!("Run `cargo run --bin serve` to view offline.");

    Ok(())
}
