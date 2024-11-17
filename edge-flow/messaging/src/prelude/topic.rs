use crate::prelude::{
    config::TopicConfig,
    error::Error,
    models::{Data, Event, Metadata},
    subscriber::{Subscriber, Subscribers},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// TODO: Implement retention policy & ordering attribute

/**
    TODO:
    Implement comprehensive context and metadata population for traceability and observability.

    1. **Context and Metadata Enrichment**: Ensure each published event has enriched metadata that includes:
        - `correlation_id`: Unique ID for tracking the event across services and processes.
        - `trace_id`: Unique identifier for tracing this event's journey within the system.
        - `attributes`: Include additional attributes for system insights and debugging.

    2. **Automatic Metadata Generation**: Enable automatic population of metadata fields by the system for every event.
       The system should capture details like the originating source, event timestamps (creation, processing, delivery),
       and any relevant system-generated identifiers.

    3. **Tracing and Logging**: Each event should be logged in a way that allows tracking its entire lifecycle
       (from creation through processing to delivery). This includes handling and logging delivery attempts and failures.

    4. **Feedback Mechanism**: Implement a mechanism to return metadata to the event producer (via the client of
       the publishing web service). This may involve providing feedback on event status, unique identifiers
       (event_id, correlation_id, trace_id), and any errors encountered during processing.

    5. **Delivery Guarantees Handling**: Ensure consistent handling of delivery guarantees:
        - `AtLeastOnce`: Deliver to as many subscribers as possible, even if some fail.
        - `ExactlyOnce`: Guarantee delivery to each subscriber exactly once or report a failure if any are unreachable.

    These requirements will provide enhanced traceability, debugging support, and improve reliability within
    the event publishing framework.
*/

pub struct Topic<T>
where
    T: Clone + Send + Sync,
{
    name: String,
    config: TopicConfig,
    subscribers: Mutex<Vec<Subscribers<T>>>,
}

impl<T> Topic<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static,
{
    pub fn new(name: String, config: TopicConfig) -> Self {
        Self {
            name,
            config,
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub async fn publish(&self, data: T) -> Result<String, Error> {
        let event_id = uuid::Uuid::new_v4().to_string();

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

        if subscribers.is_empty()
            && matches!(
                self.config.delivery_guarantee,
                crate::prelude::config::DeliveryGuarantee::ExactlyOnce
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
                Ok(_) => match self.config.delivery_guarantee {
                    crate::prelude::config::DeliveryGuarantee::AtLeastOnce => {
                        any_success = true;
                    }
                    crate::prelude::config::DeliveryGuarantee::ExactlyOnce => {
                        any_success = true;
                    }
                },
                Err(err) => {
                    let err_string = err.to_string();

                    match self.config.delivery_guarantee {
                        crate::prelude::config::DeliveryGuarantee::ExactlyOnce => {
                            return Err(Error::Transport(format!(
                                "Failed to deliver message: {}",
                                err_string
                            )));
                        }
                        crate::prelude::config::DeliveryGuarantee::AtLeastOnce => {
                            last_error = Some(err);
                        }
                    }
                }
            }
        }

        match self.config.delivery_guarantee {
            crate::prelude::config::DeliveryGuarantee::AtLeastOnce => {
                if any_success {
                    Ok(event_id)
                } else if let Some(err) = last_error {
                    Err(Error::Transport(format!(
                        "All subscribers failed to receive message: {}",
                        err
                    )))
                } else {
                    Ok(event_id)
                }
            }
            crate::prelude::config::DeliveryGuarantee::ExactlyOnce => {
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

    pub async fn subscribe(&self, subscriber: Subscribers<T>) -> Result<(), Error>
    where
        T: Clone + Send + Sync,
    {
        let mut subscribers = self.subscribers.lock().await;
        subscribers.push(subscriber);
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

impl<T> std::fmt::Debug for Topic<T>
where
    T: Clone + Send + Sync,
{
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
    use crate::prelude::config::{DeliveryGuarantee, SubscriptionConfig};
    use crate::prelude::subscriber::{MessageHandler, QueuedSubscriber};
    use assert_matches::assert_matches;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

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
        async fn handle(&self, msg: Event<TestMessage>) -> Result<(), Error> {
            if self.should_fail {
                return Err(Error::Handler("Simulated failure".to_string()));
            }
            assert_eq!(msg.data.value.value, 42);
            assert_eq!(msg.data.value.text, "test");
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct SuccessfulHandler;

    #[async_trait]
    impl MessageHandler<TestMessage> for SuccessfulHandler {
        async fn handle(&self, msg: Event<TestMessage>) -> Result<(), Error> {
            assert_eq!(msg.data.value.value, 42);
            assert_eq!(msg.data.value.text, "test");
            TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FailingHandler;

    #[async_trait]
    impl MessageHandler<TestMessage> for FailingHandler {
        async fn handle(&self, _msg: Event<TestMessage>) -> Result<(), Error> {
            Err(Error::Handler("Simulated failure".to_string()))
        }
    }

    #[tokio::test]
    async fn test_topic_publish_subscribe() {
        let config = TopicConfig {
            delivery_guarantee: crate::prelude::config::DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);

        let subscriber = QueuedSubscriber::new(
            Box::new(TestHandler {
                counter: Arc::new(AtomicUsize::new(0)),
                should_fail: false,
            }),
            SubscriptionConfig::new()
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1)),
            10,
        );

        subscriber.start_processing().await.unwrap();
        topic
            .subscribe(Subscribers::Queued(subscriber))
            .await
            .unwrap();
        assert_eq!(topic.subscriber_count().await, 1);

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

        let msg_id = topic.publish(message).await.unwrap();
        assert!(!msg_id.is_empty());

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(TEST_COUNTER.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_exactly_once_delivery() {
        let config = TopicConfig {
            delivery_guarantee: DeliveryGuarantee::ExactlyOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);

        let subscriber = QueuedSubscriber::new(
            Box::new(FailingHandler),
            SubscriptionConfig::new()
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1))
                .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce),
            10,
        );

        subscriber.start_processing().await.unwrap();
        topic
            .subscribe(Subscribers::Queued(subscriber))
            .await
            .unwrap();

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
            delivery_guarantee: crate::prelude::config::DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);
        TEST_COUNTER.store(0, Ordering::SeqCst);

        let successful_subscriber = QueuedSubscriber::new(
            Box::new(SuccessfulHandler),
            SubscriptionConfig::new()
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1)),
            10,
        );

        let failing_subscriber = QueuedSubscriber::new(
            Box::new(FailingHandler),
            SubscriptionConfig::new()
                .with_concurrency(1)
                .with_ack_deadline(Duration::from_secs(1)),
            10,
        );

        successful_subscriber.start_processing().await.unwrap();
        failing_subscriber.start_processing().await.unwrap();

        topic
            .subscribe(Subscribers::Queued(successful_subscriber))
            .await
            .unwrap();
        topic
            .subscribe(Subscribers::Queued(failing_subscriber))
            .await
            .unwrap();

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

        let result = topic.publish(message).await;
        assert!(result.is_ok());

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(TEST_COUNTER.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_unsubscribe_all() {
        let config = TopicConfig::default();
        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);
        TEST_COUNTER.store(0, Ordering::SeqCst);

        for _ in 0..3 {
            let subscriber = QueuedSubscriber::new(
                Box::new(SuccessfulHandler),
                SubscriptionConfig::new()
                    .with_concurrency(1)
                    .with_ack_deadline(Duration::from_secs(1)),
                10,
            );

            subscriber.start_processing().await.unwrap();
            topic
                .subscribe(Subscribers::Queued(subscriber))
                .await
                .unwrap();
        }

        assert_eq!(topic.subscriber_count().await, 3);

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
            delivery_guarantee: crate::prelude::config::DeliveryGuarantee::ExactlyOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(60),
        };

        let topic: Topic<TestMessage> = Topic::new("test-topic".to_string(), config);

        let message = TestMessage {
            value: 42,
            text: "test".to_string(),
        };

        let result = topic.publish(message).await;
        assert_matches!(result, Err(Error::Transport(_)));
    }
}
