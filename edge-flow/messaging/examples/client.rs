use messaging::client::PubSubClient;
use messaging::topics::LogMessage;
use tracing::info;

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

    println!("Subscribed to logs");

    // Publish some test messages
    let log = LogMessage {
        service: "test".to_string(),
        level: "INFO".to_string(),
        message: "Test message".to_string(),
        timestamp: chrono::Utc::now(),
    };

    let msg_id = publisher
        .publish("logs", log)
        .await
        .expect("Failed to publish");
    info!("Published message with ID: {}", msg_id);

    // Keep the program running to see messages
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
}
