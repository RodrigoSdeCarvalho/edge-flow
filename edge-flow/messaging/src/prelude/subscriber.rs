// TODO: Use retry policy

// TODO: Make Subscriber customizable

// TODO: Fix same event being processed multiple times

// TODO: Fix unending workers when subscriber is dropped

// TODO: Prevent multiple `start_processing` creating multiple workers each time

// TODO: Add notify for empty queue and optional timeout on shutdown

use crate::prelude::{
    config::{DeliveryGuarantee, SubscriptionConfig},
    error::Error,
    models::Event,
    queue::MessageQueue,
};
use async_trait::async_trait;
use derive_more::derive::From;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::timeout;

#[async_trait]
pub trait Subscriber<T>: Send + Sync
where
    T: Clone + Send + Sync,
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error>;
}

#[derive(From)]
pub enum Subscribers<T>
where
    T: Clone + Send + Sync,
{
    Queued(QueuedSubscriber<T>),
}

#[async_trait]
impl<T> Subscriber<T> for Subscribers<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error> {
        match self {
            Self::Queued(inner) => Subscriber::receive(inner, event).await,
        }
    }
}

#[async_trait]
pub trait MessageHandler<T>: Send + Sync
where
    T: Clone + Send + Sync + 'static,
{
    async fn handle(&self, msg: Event<T>) -> Result<(), Error>;
}

pub struct QueuedSubscriber<T>
where
    T: Clone + Send + Sync,
{
    queue: MessageQueue<T>,
    handler: Arc<dyn MessageHandler<T>>,
    config: SubscriptionConfig,
    workers_active: Arc<AtomicBool>,
    accepting_messages: Arc<AtomicBool>,
}

impl<T> QueuedSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(
        handler: Box<dyn MessageHandler<T>>,
        config: SubscriptionConfig,
        queue_size: usize,
    ) -> Self {
        Self {
            queue: MessageQueue::new(queue_size),
            handler: Arc::from(handler),
            config,
            workers_active: Arc::new(AtomicBool::new(false)),
            accepting_messages: Arc::new(AtomicBool::new(true)),
        }
    }

    pub async fn start_processing(&self) -> Result<(), Error> {
        if self.workers_active.load(Ordering::SeqCst) {
            return Ok(());
        }

        let queue = self.queue.clone();
        let handler = self.handler.clone();
        let max_concurrency = self.config.max_concurrency;
        let ack_deadline = self.config.ack_deadline;
        let workers_active = self.workers_active.clone();

        self.workers_active.store(true, Ordering::SeqCst);
        self.accepting_messages.store(true, Ordering::SeqCst);

        for _ in 0..max_concurrency {
            let queue = queue.clone();
            let handler = handler.clone();
            let active = workers_active.clone();

            tokio::spawn(async move {
                while active.load(Ordering::SeqCst) {
                    if let Some(event) =
                        queue.dequeue_with_timeout(Duration::from_millis(100)).await
                    {
                        match timeout(ack_deadline, handler.handle(event)).await {
                            Ok(Ok(_)) => {}
                            Ok(Err(_)) => {
                                // Handle retry logic here if needed
                            }
                            Err(_) => {
                                // Handle timeout retry logic here if needed
                            }
                        }
                    }
                }
            });
        }

        Ok(())
    }

    pub async fn shutdown(&self, timeout_duration: Duration) {
        self.accepting_messages.store(false, Ordering::SeqCst);

        let _ = timeout(timeout_duration, async {
            while !self.queue.is_empty().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;

        self.workers_active.store(false, Ordering::SeqCst);

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[async_trait]
impl<T> Subscriber<T> for QueuedSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error> {
        if !self.accepting_messages.load(Ordering::SeqCst) {
            return Err(Error::Shutdown("Subscriber is shutting down".to_string()));
        }

        let is_exactly_once = matches!(
            self.config.delivery_guarantee,
            DeliveryGuarantee::ExactlyOnce
        );

        if is_exactly_once {
            match timeout(self.config.ack_deadline, self.handler.handle(event)).await {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(Error::Handler(e.to_string())),
                Err(_) => Err(Error::Timeout),
            }
        } else {
            self.queue.enqueue(event).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::models::{Data, Metadata};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct TestHandler {
        counter: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl MessageHandler<i32> for TestHandler {
        async fn handle(&self, _: Event<i32>) -> Result<(), Error> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct ErrorHandler;

    #[async_trait]
    impl MessageHandler<i32> for ErrorHandler {
        async fn handle(&self, _: Event<i32>) -> Result<(), Error> {
            Err(Error::Handler("Test error".to_string()))
        }
    }

    fn create_test_event() -> Event<i32> {
        Event {
            data: Data {
                value: 42,
                metadata: Metadata {
                    source: "test".to_string(),
                    correlation_id: None,
                    trace_id: None,
                    attributes: HashMap::new(),
                },
                timestamp: Utc::now(),
            },
            event_type: "test".to_string(),
            event_id: "test1".to_string(),
            event_time: Utc::now(),
            ordering_key: None,
        }
    }

    #[tokio::test]
    async fn test_queued_subscriber() {
        let counter = Arc::new(AtomicUsize::new(0));
        let handler = TestHandler {
            counter: counter.clone(),
        };

        let config = SubscriptionConfig::new()
            .with_concurrency(2)
            .with_ack_deadline(Duration::from_secs(1))
            .with_delivery_guarantee(DeliveryGuarantee::AtLeastOnce);

        let subscriber = QueuedSubscriber::new(Box::new(handler), config, 10);
        subscriber.start_processing().await.unwrap();

        subscriber.receive(create_test_event()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        subscriber.shutdown(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    async fn test_handler_error() {
        let config = SubscriptionConfig::new()
            .with_concurrency(1)
            .with_ack_deadline(Duration::from_secs(1))
            .with_delivery_guarantee(DeliveryGuarantee::AtLeastOnce);

        let subscriber = QueuedSubscriber::new(Box::new(ErrorHandler), config, 10);
        subscriber.start_processing().await.unwrap();

        subscriber.receive(create_test_event()).await.unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        subscriber.shutdown(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    async fn test_exactly_once_delivery() {
        let counter = Arc::new(AtomicUsize::new(0));
        let handler = TestHandler {
            counter: counter.clone(),
        };

        let config = SubscriptionConfig::new()
            .with_concurrency(1)
            .with_ack_deadline(Duration::from_secs(1))
            .with_delivery_guarantee(DeliveryGuarantee::ExactlyOnce);

        let subscriber = QueuedSubscriber::new(Box::new(handler), config, 10);

        subscriber.receive(create_test_event()).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        subscriber.shutdown(Duration::from_secs(1)).await;
    }
}
