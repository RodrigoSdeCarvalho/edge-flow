use messaging::service::PubSubService;
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting Edge Flow messaging service...");

    // Create service
    let service = PubSubService::new();

    info!("Default topics registered");

    // Bind to localhost:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Server starting on {}", addr);

    // Run the service
    service.run(addr).await;
}
