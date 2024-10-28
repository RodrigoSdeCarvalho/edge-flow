use messaging::client::PubSubClient;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LogMessage {
    level: String,
    message: String,
    timestamp: String,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create client
    let client = PubSubClient::new("http://localhost:3000".to_string());

    // Create publisher for logs
    let publisher = client.publisher::<LogMessage>();

    // Subscribe to logs
    let subscriber = client.subscriber::<LogMessage>();
    subscriber
        .subscribe("logs", |event| {
            info!("Received log: {:?}", event.data.value);
            Ok(())
        })
        .await
        .expect("Failed to subscribe");

    // Publish some test messages
    let log = LogMessage {
        level: "INFO".to_string(),
        message: "Test message".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let msg_id = publisher
        .publish("logs", log)
        .await
        .expect("Failed to publish");
    info!("Published message with ID: {}", msg_id);

    // Keep the program running to see messages
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
}
