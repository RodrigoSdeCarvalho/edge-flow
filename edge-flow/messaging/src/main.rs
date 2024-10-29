use dotenv::dotenv;
use messaging::{prelude::Error, service::PubSubService};
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_thread_ids(true)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_ansi(true)
        .init();

    info!("Starting Edge Flow messaging service...");

    let service = PubSubService::new();

    let port = std::env::var("PORT")
        .map(|p| p.parse::<u16>().expect("Invalid PORT"))
        .unwrap_or(3000);

    let addr: SocketAddr = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()
        .expect("Invalid BIND_ADDRESS");

    let addr = SocketAddr::new(addr.ip(), port);

    info!("Starting server on {}", addr);
    info!("Available topics: logs, metrics, alerts");
    info!("Default topics support the following message types:");
    info!("  - logs: LogMessage (level, message, timestamp, service)");
    info!("  - metrics: MetricMessage (name, value, unit, timestamp, tags)");
    info!("  - alerts: AlertMessage (severity, message, timestamp, source, status)");

    service.run(addr).await;

    Ok(())
}
