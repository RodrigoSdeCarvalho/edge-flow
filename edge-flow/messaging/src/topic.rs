use crate::config::TopicConfig;
use crate::error::Error;
use crate::models::{Data, Event, Metadata};
use crate::subscriber::Subscriber;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error};

pub struct Topic<T> {
    name: String,
    config: TopicConfig,
    _phantom: PhantomData<T>,
    subscribers: Arc<Mutex<Vec<Box<dyn Subscriber<T> + Send + Sync>>>>,
}

impl<T> Topic<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    pub fn new(name: String, config: TopicConfig) -> Self {
        Self {
            name,
            config,
            _phantom: PhantomData,
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn publish(&self, data: T) -> Result<String, Error> {
        let event_id = uuid::Uuid::new_v4().to_string();
        debug!(
            "Publishing message with id {} to topic {}",
            event_id, self.name
        );

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

        // If no subscribers and ExactlyOnce, consider it a failure
        if subscribers.is_empty()
            && matches!(
                self.config.delivery_guarantee,
                crate::config::DeliveryGuarantee::ExactlyOnce
            )
        {
            return Err(Error::Transport(
                "No subscribers available for ExactlyOnce delivery".to_string(),
            ));
        }

        let mut any_success = false;
        let mut last_error = None;

        for subscriber in subscribers.iter() {
            match subscriber.receive(event.clone()).await {
                Ok(_) => {
                    match self.config.delivery_guarantee {
                        crate::config::DeliveryGuarantee::AtLeastOnce => {
                            any_success = true;
                        }
                        crate::config::DeliveryGuarantee::ExactlyOnce => {
                            // Keep track but continue checking other subscribers
                            any_success = true;
                        }
                    }
                }
                Err(err) => {
                    let err_string = err.to_string();
                    error!("Error delivering message to subscriber: {}", err_string);

                    match self.config.delivery_guarantee {
                        crate::config::DeliveryGuarantee::ExactlyOnce => {
                            // For ExactlyOnce, fail immediately on first error
                            return Err(Error::Transport(format!(
                                "Failed to deliver message: {}",
                                err_string
                            )));
                        }
                        crate::config::DeliveryGuarantee::AtLeastOnce => {
                            last_error = Some(err);
                        }
                    }
                }
            }
        }

        match self.config.delivery_guarantee {
            crate::config::DeliveryGuarantee::AtLeastOnce => {
                if any_success {
                    Ok(event_id)
                } else if let Some(err) = last_error {
                    Err(Error::Transport(format!(
                        "All subscribers failed to receive message: {}",
                        err
                    )))
                } else {
                    // No subscribers case for AtLeastOnce
                    Ok(event_id)
                }
            }
            crate::config::DeliveryGuarantee::ExactlyOnce => {
                if any_success {
                    Ok(event_id)
                } else {
                    Err(Error::Transport(
                        "No successful deliveries for ExactlyOnce guarantee".to_string(),
                    ))
                }
            }
        }
    }

    pub async fn subscribe<S>(&self, subscriber: S) -> Result<(), Error>
    where
        S: Subscriber<T> + Send + Sync + 'static,
    {
        let mut subscribers = self.subscribers.lock().await;
        subscribers.push(Box::new(subscriber));
        Ok(())
    }

    pub async fn unsubscribe_all(&self) -> Result<(), Error> {
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

    #[cfg(test)]
    pub(crate) async fn subscriber_count(&self) -> usize {
        self.subscribers.lock().await.len()
    }
}

impl<T> std::fmt::Debug for Topic<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Topic")
            .field("name", &self.name)
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DeliveryGuarantee, SubscriptionConfig};
    use crate::models::{Context, Event};
    use crate::subscriber::{
        BatchMessageHandler, BatchSubscriber, MessageHandler, QueuedSubscriber,
    };
    use assert_matches::assert_matches;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestMessage {
        value: i32,
        text: String,
    }

    struct TestHandler {
        counter: Arc<AtomicUsize>,
        should_fail: bool,
    }

    #[async_trait]
    impl MessageHandler<TestMessage> for TestHandler {
        async fn handle(&self, _ctx: &Context, msg: Event<TestMessage>) -> Result<(), Error> {
            if self.should_fail {
                return Err(Error::Handler("Simulated failure".to_string()));
            }
            assert_eq!(msg.data.value.value, 42);
            assert_eq!(msg.data.value.text, "test");
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct TestBatchHandler {
        counter: Arc<AtomicUsize>,
        should_fail: bool,
    }

    #[async_trait]
    impl BatchMessageHandler<TestMessage> for TestBatchHandler {
        async fn handle_batch(
            &self,
            _ctx: &Context,
            msgs: Vec<Event<TestMessage>>,
        ) -> Result<(), Error> {
            if self.should_fail {
                return Err(Error::Handler("Simulated failure".to_string()));
            }
            self.counter.fetch_add(msgs.len(), Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_topic_publish_subscribe() {
        let config = TopicConfig {
            delivery_guarantee: crate::config::DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);
        let counter = Arc::new(AtomicUsize::new(0));

        let handler = Arc::new(TestHandler {
            counter: counter.clone(),
            should_fail: false,
        });

        let subscriber = QueuedSubscriber::new(
            handler.clone(), // Clone here
            SubscriptionConfig::new(handler)
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1)),
            10, // queue size
        );

        subscriber.start_processing().await.unwrap();
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
            delivery_guarantee: DeliveryGuarantee::ExactlyOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);

        let handler = Arc::new(TestHandler {
            counter: Arc::new(AtomicUsize::new(0)),
            should_fail: true,
        });

        let subscriber = QueuedSubscriber::new(
            handler.clone(),
            SubscriptionConfig::new(handler)
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1))
                .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce), // Add this
            10,
        );

        subscriber.start_processing().await.unwrap();
        topic.subscribe(subscriber).await.unwrap();

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

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
        let counter = Arc::new(AtomicUsize::new(0));

        // Add a successful subscriber
        let successful_handler = Arc::new(TestHandler {
            counter: counter.clone(),
            should_fail: false,
        });

        let successful_subscriber = QueuedSubscriber::new(
            successful_handler.clone(),
            SubscriptionConfig::new(successful_handler)
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1)),
            10,
        );

        // Add a failing subscriber
        let failing_handler = Arc::new(TestHandler {
            counter: Arc::new(AtomicUsize::new(0)),
            should_fail: true,
        });

        let failing_subscriber = QueuedSubscriber::new(
            failing_handler.clone(),
            SubscriptionConfig::new(failing_handler)
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1)),
            10,
        );

        successful_subscriber.start_processing().await.unwrap();
        failing_subscriber.start_processing().await.unwrap();

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
    async fn test_batch_processing() {
        let config = TopicConfig::default();
        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);
        let counter = Arc::new(AtomicUsize::new(0));

        let handler = Arc::new(TestBatchHandler {
            counter: counter.clone(),
            should_fail: false,
        });

        let batch_subscriber = BatchSubscriber::new(
            handler,
            2,                         // batch size
            Duration::from_millis(50), // batch timeout
            10,                        // queue size
        );

        batch_subscriber.start_processing().await.unwrap();
        topic.subscribe(batch_subscriber).await.unwrap();

        // Publish multiple messages
        for i in 0..3 {
            let message = TestMessage {
                value: 42,
                text: format!("test{}", i),
            };
            topic.publish(message).await.unwrap();
        }

        // Give time for batch processing
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_unsubscribe_all() {
        let config = TopicConfig::default();
        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);
        let counter = Arc::new(AtomicUsize::new(0));

        // Add multiple subscribers
        for _ in 0..3 {
            // Changed i to _ to fix unused variable warning
            let handler = Arc::new(TestHandler {
                counter: counter.clone(),
                should_fail: false,
            });

            let subscriber = QueuedSubscriber::new(
                handler.clone(), // Clone here
                SubscriptionConfig::new(handler)
                    .with_concurrency(1)
                    .with_ack_deadline(Duration::from_secs(1)),
                10,
            );

            subscriber.start_processing().await.unwrap();
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
        assert_eq!(
            topic.get_config().delivery_guarantee as u8,
            config.delivery_guarantee as u8
        );
        assert_eq!(
            topic.get_config().message_retention,
            config.message_retention
        );
    }

    #[tokio::test]
    async fn test_exactly_once_empty() {
        let config = TopicConfig {
            delivery_guarantee: crate::config::DeliveryGuarantee::ExactlyOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

        // Should fail because there are no subscribers for ExactlyOnce
        let result = topic.publish(message).await;
        assert_matches!(result, Err(Error::Transport(_)));
    }
}
