mod monitor;

use clap::Parser;
use std::time::Duration;
use tracing::Level;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Endpoint URLs to monitor (space-separated)
    #[arg(value_name = "URLS", num_args = 1..)]
    endpoints: Vec<String>,

    /// Check interval in seconds
    #[arg(short, long, default_value = "60")]
    interval: u64,

    /// Request timeout in seconds
    #[arg(short, long, default_value = "10")]
    timeout: u64,
}

fn main() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Parse command line arguments
    let args = Args::parse();

    // Create runtime
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    // Create and run monitor
    runtime.block_on(async {
        let mut monitor = monitor::Monitor::new(
            args.endpoints,
            Duration::from_secs(args.interval),
            Duration::from_secs(args.timeout),
        );

        monitor.run().await;
    });
}
