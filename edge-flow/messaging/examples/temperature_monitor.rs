use async_trait::async_trait;
use messaging::prelude::{
    config::SubscriptionConfig,
    models::{Context, Event},
    subscriber::{MessageHandler, QueuedSubscriber},
    DeliveryGuarantee, Error, Topic, TopicConfig,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

// Define our message type
#[derive(Clone, Serialize, Deserialize)]
struct TemperatureReading {
    sensor_id: String,
    temperature: f64,
    unit: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

// Define handlers for temperature readings
struct TemperatureHandler {
    name: String,
}

#[async_trait]
impl MessageHandler<TemperatureReading> for TemperatureHandler {
    async fn handle(&self, _: &Context, msg: Event<TemperatureReading>) -> Result<(), Error> {
        info!(
            "Handler '{}' received temperature reading: {:.1}°{} from sensor {} at {} (message ID: {})", 
            self.name,
            msg.data.value.temperature,
            msg.data.value.unit,
            msg.data.value.sensor_id,
            msg.data.value.timestamp,
            msg.event_id
        );

        // Simulate some processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    // Create a topic with exactly-once delivery guarantee
    let topic_config = TopicConfig {
        delivery_guarantee: DeliveryGuarantee::ExactlyOnce,
        ordering_attribute: None,
        message_retention: Duration::from_secs(3600), // 1 hour
    };

    let topic: Topic<TemperatureReading> =
        Topic::new("temperature-readings".to_string(), topic_config);

    // Create two handlers with different configurations
    let handler1 = Arc::new(TemperatureHandler {
        name: "Main Monitor".to_string(),
    });

    let handler2 = Arc::new(TemperatureHandler {
        name: "Backup Monitor".to_string(),
    });

    // Create queued subscribers with different configurations
    let subscriber1 = QueuedSubscriber::new(
        handler1.clone(),
        SubscriptionConfig::new()
            .with_concurrency(2)
            .with_ack_deadline(Duration::from_secs(5))
            .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce),
        1000, // queue size
    );

    let subscriber2 = QueuedSubscriber::new(
        handler2.clone(),
        SubscriptionConfig::new()
            .with_concurrency(1)
            .with_ack_deadline(Duration::from_secs(5))
            .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce),
        1000, // queue size
    );

    // Start processing for both subscribers
    subscriber1.start_processing().await?;
    subscriber2.start_processing().await?;

    // Subscribe both to the topic
    topic.subscribe(subscriber1).await?;
    topic.subscribe(subscriber2).await?;

    info!("Starting temperature monitoring...");

    // Simulate publishing some temperature readings
    let readings = vec![
        TemperatureReading {
            sensor_id: "SENSOR001".to_string(),
            temperature: 22.5,
            unit: "C".to_string(),
            timestamp: chrono::Utc::now(),
        },
        TemperatureReading {
            sensor_id: "SENSOR002".to_string(),
            temperature: 23.1,
            unit: "C".to_string(),
            timestamp: chrono::Utc::now(),
        },
        TemperatureReading {
            sensor_id: "SENSOR003".to_string(),
            temperature: 21.8,
            unit: "C".to_string(),
            timestamp: chrono::Utc::now(),
        },
    ];

    // Publish readings with some delay between them
    for reading in readings {
        match topic.publish(reading).await {
            Ok(msg_id) => {
                info!("Published message with ID: {}", msg_id);
            }
            Err(e) => {
                error!("Failed to publish message: {}", e);
            }
        }

        // Wait a bit between messages
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Keep the program running to see the processing
    info!("Waiting for message processing...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    info!("Shutting down...");
    Ok(())
}
