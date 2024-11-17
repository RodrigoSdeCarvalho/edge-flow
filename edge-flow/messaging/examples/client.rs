use messaging::{
    client::{HandlerStore, PubSubClient},
    topics::LogMessage,
    Event,
};

#[tokio::main]
async fn main() {
    let client = PubSubClient::new("http://localhost:3000".to_string());
    let publisher = client.publisher();
    let mut store = HandlerStore::new();

    let subscriber = client.subscriber();
    let handle_logs = subscriber
        .subscribe("logs", "handle_logs", |event: Event<LogMessage>| {
            println!("Received event: {:?}", event);
            println!("Message: {:?}", event.event_id);
            Ok(())
        })
        .await
        .expect("Failed to subscribe");
    store.add_handler(handle_logs);

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
    println!("Published message with ID: {}", msg_id);

    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
}
