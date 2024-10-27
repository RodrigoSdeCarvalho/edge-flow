use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Utc;
use serde::{ Serialize, Deserialize };
use tracing::{ debug, error, info };

use crate::error::Error;
use crate::models::{ Event, Data, Metadata };
use crate::config::TopicConfig;
use crate::subscriber::Subscriber;

pub struct Topic<T> {
    name: String,
    config: TopicConfig,
    _phantom: PhantomData<T>,
    subscribers: Arc<Mutex<Vec<Box<dyn Subscriber<T> + Send + Sync>>>>,
}

impl<T> Topic<T> where T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static {
    pub fn new(name: String, config: TopicConfig) -> Self {
        info!("Creating new topic: {}", name);
        Self {
            name,
            config,
            _phantom: PhantomData,
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn publish(&self, data: T) -> Result<String, Error> {
        let event_id = uuid::Uuid::new_v4().to_string();
        debug!("Publishing message with id {} to topic {}", event_id, self.name);

        let event = Event {
            data: Data {
                value: data,
                timestamp: Utc::now(),
                metadata: Metadata {
                    source: self.name.clone(),
                    correlation_id: None,
                    trace_id: None,
                    attributes: std::collections::HashMap::new(),
                },
            },
            event_type: std::any::type_name::<T>().to_string(),
            event_id: event_id.clone(),
            event_time: Utc::now(),
            ordering_key: None,
        };

        let subscribers = self.subscribers.lock().await;
        let mut errors = Vec::new();

        for subscriber in subscribers.iter() {
            if let Err(err) = subscriber.receive(event.clone()).await {
                let err_string = err.to_string(); // Convert to string before moving
                error!("Error delivering message to subscriber: {}", err_string);
                errors.push(err);

                match self.config.delivery_guarantee {
                    crate::config::DeliveryGuarantee::AtLeastOnce => {
                        // Log but continue with other subscribers
                        continue;
                    }
                    crate::config::DeliveryGuarantee::ExactlyOnce => {
                        return Err(
                            Error::Transport(format!("Failed to deliver message: {}", err_string))
                        );
                    }
                }
            }
        }

        // For AtLeastOnce, only return error if all subscribers failed
        if !errors.is_empty() && errors.len() == subscribers.len() {
            return Err(Error::Transport("All subscribers failed to receive message".to_string()));
        }

        Ok(event_id)
    }

    pub async fn subscribe<S>(&self, subscriber: S) -> Result<(), Error>
        where S: Subscriber<T> + Send + Sync + 'static
    {
        debug!("Adding new subscriber to topic {}", self.name);
        let mut subscribers = self.subscribers.lock().await;
        subscribers.push(Box::new(subscriber));
        Ok(())
    }

    pub async fn unsubscribe_all(&self) -> Result<(), Error> {
        debug!("Removing all subscribers from topic {}", self.name);
        let mut subscribers = self.subscribers.lock().await;
        subscribers.clear();
        Ok(())
    }

    pub fn get_config(&self) -> &TopicConfig {
        &self.config
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    // Add this method to peek at the number of subscribers (useful for testing)
    #[cfg(test)]
    pub(crate) async fn subscriber_count(&self) -> usize {
        self.subscribers.lock().await.len()
    }
}

impl<T> std::fmt::Debug for Topic<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Topic").field("name", &self.name).field("config", &self.config).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subscriber::FunctionSubscriber;
    use std::sync::atomic::{ AtomicI32, Ordering };
    use std::sync::Arc;
    use std::time::Duration;
    use assert_matches::assert_matches;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestMessage {
        value: i32,
        text: String,
    }

    #[tokio::test]
    async fn test_topic_publish_subscribe() {
        let config = TopicConfig {
            delivery_guarantee: crate::config::DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = counter.clone();

        let subscriber = FunctionSubscriber::new(move |event: Event<TestMessage>| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            assert_eq!(event.data.value.value, 42);
            assert_eq!(event.data.value.text, "test");
            Ok(())
        });

        topic.subscribe(subscriber).await.unwrap();
        assert_eq!(topic.subscriber_count().await, 1);

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

        let msg_id = topic.publish(message).await.unwrap();
        assert!(!msg_id.is_empty());

        // Give some time for async processing
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_exactly_once_delivery() {
        let config = TopicConfig {
            delivery_guarantee: crate::config::DeliveryGuarantee::ExactlyOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);

        // Add a failing subscriber
        let failing_subscriber = FunctionSubscriber::new(|_| {
            Err(Error::Handler("Simulated failure".to_string()))
        });

        topic.subscribe(failing_subscriber).await.unwrap();

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

        // Publishing should fail because the subscriber failed
        let result = topic.publish(message).await;
        assert_matches!(result, Err(Error::Transport(_)));
    }

    #[tokio::test]
    async fn test_at_least_once_delivery() {
        let config = TopicConfig {
            delivery_guarantee: crate::config::DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);
        let counter = Arc::new(AtomicI32::new(0));

        // Add a successful subscriber
        let counter_clone = counter.clone();
        let successful_subscriber = FunctionSubscriber::new(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        // Add a failing subscriber
        let failing_subscriber = FunctionSubscriber::new(|_| {
            Err(Error::Handler("Simulated failure".to_string()))
        });

        topic.subscribe(successful_subscriber).await.unwrap();
        topic.subscribe(failing_subscriber).await.unwrap();

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

        // Publishing should succeed because at least one subscriber succeeded
        let result = topic.publish(message).await;
        assert!(result.is_ok());

        // Give some time for async processing
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_unsubscribe_all() {
        let config = TopicConfig::default();
        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);

        // Add multiple subscribers
        for _ in 0..3 {
            let subscriber = FunctionSubscriber::new(|_: Event<TestMessage>| Ok(()));
            topic.subscribe(subscriber).await.unwrap();
        }

        assert_eq!(topic.subscriber_count().await, 3);

        // Unsubscribe all
        topic.unsubscribe_all().await.unwrap();
        assert_eq!(topic.subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn test_topic_metadata() {
        let config = TopicConfig::default();
        let topic_name = "test-metadata-topic".to_string();
        let topic: Topic<TestMessage> = Topic::new(topic_name.clone(), config.clone());

        assert_eq!(topic.get_name(), topic_name);
        assert_eq!(topic.get_config().delivery_guarantee as u8, config.delivery_guarantee as u8);
        assert_eq!(topic.get_config().message_retention, config.message_retention);
    }
}
