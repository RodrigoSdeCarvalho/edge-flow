use messaging::{service::PubSubService, DeliveryGuarantee, TopicConfig};
use std::net::SocketAddr;
use std::time::Duration;
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

    // Create some default topics
    let config = TopicConfig {
        delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
        ordering_attribute: None,
        message_retention: Duration::from_secs(3600), // 1 hour
    };

    // Get topic registry from service and register some default topics
    service
        .topic_registry
        .register::<String>("logs", config.clone())
        .await;
    service
        .topic_registry
        .register::<String>("metrics", config.clone())
        .await;
    service
        .topic_registry
        .register::<String>("alerts", config.clone())
        .await;

    info!("Default topics registered");

    // Bind to localhost:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Server starting on {}", addr);

    // Run the service
    service.run(addr).await;
}
