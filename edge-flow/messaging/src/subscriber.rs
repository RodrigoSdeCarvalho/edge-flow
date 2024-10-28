use crate::config::{DeliveryGuarantee, SubscriptionConfig};
use crate::error::Error;
use crate::models::{Context, Event};
use crate::queue::MessageQueue;
use async_trait::async_trait;
use chrono::Utc;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, warn};

#[async_trait]
pub trait MessageHandler<T>: Send + Sync
where
    T: Clone + Send + Sync + 'static,
{
    async fn handle(&self, ctx: &Context, msg: Event<T>) -> Result<(), Error>;
}

#[async_trait]
pub trait BatchMessageHandler<T>: Send + Sync
where
    T: Clone + Send + Sync + 'static,
{
    async fn handle_batch(&self, ctx: &Context, msgs: Vec<Event<T>>) -> Result<(), Error>;
}

#[async_trait]
pub trait Subscriber<T>: Send + Sync
where
    T: Clone + Send + Sync + 'static,
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error>;
}

pub struct FunctionSubscriber<T, F>
where
    T: Clone + Send + Sync + 'static,
    F: Fn(Event<T>) -> Result<(), Error> + Send + Sync,
{
    handler: F,
    _phantom: PhantomData<T>,
}

impl<T, F> FunctionSubscriber<T, F>
where
    T: Clone + Send + Sync + 'static,
    F: Fn(Event<T>) -> Result<(), Error> + Send + Sync,
{
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<T, F> Subscriber<T> for FunctionSubscriber<T, F>
where
    T: Clone + Send + Sync + 'static,
    F: Fn(Event<T>) -> Result<(), Error> + Send + Sync,
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error> {
        (self.handler)(event)
    }
}

pub struct QueuedSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    queue: Arc<MessageQueue<T>>,
    handler: Arc<dyn MessageHandler<T>>,
    config: SubscriptionConfig<T>,
}

impl<T> QueuedSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(
        handler: Arc<dyn MessageHandler<T>>,
        config: SubscriptionConfig<T>,
        queue_size: usize,
    ) -> Self {
        Self {
            queue: Arc::new(MessageQueue::new(queue_size)),
            handler,
            config,
        }
    }

    pub async fn start_processing(&self) -> Result<(), Error> {
        let queue = self.queue.clone();
        let handler = self.handler.clone();
        let max_concurrency = self.config.max_concurrency;
        let ack_deadline = self.config.ack_deadline;

        for _ in 0..max_concurrency {
            let queue = queue.clone();
            let handler = handler.clone();

            tokio::spawn(async move {
                loop {
                    if let Some(event) =
                        queue.dequeue_with_timeout(Duration::from_millis(100)).await
                    {
                        let ctx = Context {
                            trace_id: event.data.metadata.trace_id.clone(),
                            correlation_id: event.data.metadata.correlation_id.clone(),
                            deadline: Utc::now()
                                + chrono::Duration::from_std(ack_deadline).unwrap(),
                        };

                        match timeout(ack_deadline, handler.handle(&ctx, event.clone())).await {
                            Ok(Ok(_)) => {
                                debug!("Successfully processed message {}", event.event_id);
                            }
                            Ok(Err(e)) => {
                                error!("Error processing message {}: {}", event.event_id, e);
                                // Handle retry logic here if needed
                            }
                            Err(_) => {
                                warn!("Message {} processing timed out", event.event_id);
                                // Handle timeout retry logic here if needed
                            }
                        }
                    }
                }
            });
        }

        Ok(())
    }
}

#[async_trait]
impl<T> Subscriber<T> for QueuedSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error> {
        let is_exactly_once = matches!(
            self.config.delivery_guarantee,
            DeliveryGuarantee::ExactlyOnce
        );

        if is_exactly_once {
            let ctx = Context {
                trace_id: event.data.metadata.trace_id.clone(),
                correlation_id: event.data.metadata.correlation_id.clone(),
                deadline: Utc::now()
                    + chrono::Duration::from_std(self.config.ack_deadline).unwrap(),
            };

            match timeout(
                self.config.ack_deadline,
                self.handler.handle(&ctx, event.clone()),
            )
            .await
            {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(Error::Timeout),
            }
        } else {
            self.queue.enqueue(event).await
        }
    }
}

pub struct BatchSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    queue: Arc<MessageQueue<T>>,
    handler: Arc<dyn BatchMessageHandler<T>>,
    batch_size: usize,
    batch_timeout: Duration,
}

impl<T> BatchSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(
        handler: Arc<dyn BatchMessageHandler<T>>,
        batch_size: usize,
        batch_timeout: Duration,
        queue_size: usize,
    ) -> Self {
        Self {
            queue: Arc::new(MessageQueue::new(queue_size)),
            handler,
            batch_size,
            batch_timeout,
        }
    }

    pub async fn start_processing(&self) -> Result<(), Error> {
        let queue = self.queue.clone();
        let handler = self.handler.clone();
        let batch_size = self.batch_size;
        let batch_timeout = self.batch_timeout;

        tokio::spawn(async move {
            loop {
                let batch = queue
                    .dequeue_batch_with_timeout(batch_size, batch_timeout)
                    .await;

                if !batch.is_empty() {
                    let ctx = Context {
                        trace_id: None,
                        correlation_id: None,
                        deadline: Utc::now() + chrono::Duration::from_std(batch_timeout).unwrap(),
                    };

                    match handler.handle_batch(&ctx, batch.clone()).await {
                        Ok(_) => {
                            debug!("Successfully processed batch of {} messages", batch.len());
                        }
                        Err(e) => {
                            error!("Error processing batch: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

#[async_trait]
impl<T> Subscriber<T> for BatchSubscriber<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error> {
        self.queue.enqueue(event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Data, Metadata};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestHandler(Arc<AtomicUsize>);

    #[async_trait]
    impl<T: Clone + Send + Sync + 'static> MessageHandler<T> for TestHandler {
        async fn handle(&self, _ctx: &Context, _msg: Event<T>) -> Result<(), Error> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct TestBatchHandler(Arc<AtomicUsize>);

    #[async_trait]
    impl<T: Clone + Send + Sync + 'static> BatchMessageHandler<T> for TestBatchHandler {
        async fn handle_batch(&self, _ctx: &Context, msgs: Vec<Event<T>>) -> Result<(), Error> {
            self.0.fetch_add(msgs.len(), Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_queued_subscriber() {
        let counter = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(TestHandler(counter.clone()));

        let config = SubscriptionConfig::new(handler.clone())
            .with_concurrency(2)
            .with_ack_deadline(Duration::from_secs(1));

        let subscriber = QueuedSubscriber::new(handler, config, 10);
        subscriber.start_processing().await.unwrap();

        let event = Event {
            data: Data {
                value: 42,
                timestamp: Utc::now(),
                metadata: Metadata {
                    source: "test".to_string(),
                    correlation_id: None,
                    trace_id: None,
                    attributes: HashMap::new(),
                },
            },
            event_type: "test".to_string(),
            event_id: "test1".to_string(),
            event_time: Utc::now(),
            ordering_key: None,
        };

        subscriber.receive(event).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_batch_subscriber() {
        let counter = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(TestBatchHandler(counter.clone()));

        let subscriber = BatchSubscriber::new(handler, 2, Duration::from_millis(100), 10);
        subscriber.start_processing().await.unwrap();

        for i in 0..3 {
            let event = Event {
                data: Data {
                    value: i,
                    timestamp: Utc::now(),
                    metadata: Metadata {
                        source: "test".to_string(),
                        correlation_id: None,
                        trace_id: None,
                        attributes: HashMap::new(),
                    },
                },
                event_type: "test".to_string(),
                event_id: format!("test{}", i),
                event_time: Utc::now(),
                ordering_key: None,
            };
            subscriber.receive(event).await.unwrap();
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
