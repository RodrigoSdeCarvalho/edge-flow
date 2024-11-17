use async_trait::async_trait;
use messaging::prelude::{
    config::SubscriptionConfig,
    models::Event,
    subscriber::{MessageHandler, QueuedSubscriber, Subscribers},
    DeliveryGuarantee, Error, Topic, TopicConfig,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemperatureReading {
    sensor_id: String,
    temperature: f64,
    unit: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

struct TemperatureHandler;

#[async_trait]
impl MessageHandler<TemperatureReading> for TemperatureHandler {
    async fn handle(&self, msg: Event<TemperatureReading>) -> Result<(), Error> {
        println!("Received message: {:?}", msg);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let topic_config = TopicConfig {
        delivery_guarantee: DeliveryGuarantee::ExactlyOnce,
        ordering_attribute: None,
        message_retention: Duration::from_secs(3600),
    };

    let topic: Topic<TemperatureReading> =
        Topic::new("temperature-readings".to_string(), topic_config);

    let subscriber = QueuedSubscriber::new(
        Box::new(TemperatureHandler),
        SubscriptionConfig::new()
            .with_concurrency(2)
            .with_ack_deadline(Duration::from_secs(5))
            .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce),
        1000,
    );
    subscriber.start_processing().await?;

    topic.subscribe(Subscribers::from(subscriber)).await?;

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

    for reading in readings {
        match topic.publish(reading).await {
            Ok(msg_id) => {
                println!("Published message with ID: {}", msg_id);
            }
            Err(e) => {
                println!("Error publishing message: {:?}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    tokio::time::sleep(Duration::from_secs(5)).await;

    Ok(())
}
