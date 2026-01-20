//! Serves downloaded pages for offline viewing.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin serve          # Port 8080
//! cargo run --bin serve -- 3000  # Custom port
//! ```

use std::{env, error::Error};

use ivoclar_offline::serve;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let port = env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    serve(port).await
}
