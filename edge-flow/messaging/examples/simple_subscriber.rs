use messaging::{subscriber::FunctionSubscriber, DeliveryGuarantee, Error, Topic, TopicConfig};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize)]
struct SimpleMessage {
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Create a topic
    let topic_config = TopicConfig {
        delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
        ordering_attribute: None,
        message_retention: Duration::from_secs(60),
    };

    let topic: Topic<SimpleMessage> = Topic::new("simple-topic".to_string(), topic_config);

    // Create a simple subscriber with a closure
    let subscriber = FunctionSubscriber::new(|event: messaging::Event<SimpleMessage>| {
        println!("Received message: {}", event.data.value.content);
        Ok(())
    });

    // Subscribe
    topic.subscribe(subscriber).await?;

    // Publish a message
    let message = SimpleMessage {
        content: "Hello, World!".to_string(),
    };

    let msg_id = topic.publish(message).await?;
    println!("Published message with ID: {}", msg_id);

    // Wait a moment to see the output
    tokio::time::sleep(Duration::from_secs(1)).await;

    Ok(())
}
