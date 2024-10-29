use crate::prelude::{Error, Topic};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub struct BatchPublisherConfig {
    /// Maximum number of messages to include in a batch
    pub max_batch_size: usize,
    /// Maximum time to wait before publishing a partial batch
    pub flush_interval: Duration,
    /// Maximum number of batches that can be queued for publishing
    pub max_queued_batches: usize,
}

impl Default for BatchPublisherConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            flush_interval: Duration::from_secs(5),
            max_queued_batches: 10,
        }
    }
}

pub struct BatchPublisher<T>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
{
    topic: Arc<Topic<T>>,
    config: BatchPublisherConfig,
    current_batch: Arc<Mutex<Vec<T>>>,
    is_running: Arc<RwLock<bool>>,
    _phantom: PhantomData<T>,
}

impl<T> BatchPublisher<T>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(topic: Arc<Topic<T>>, config: BatchPublisherConfig) -> Self {
        let max_batch_size = config.max_batch_size;
        Self {
            topic,
            config,
            current_batch: Arc::new(Mutex::new(Vec::with_capacity(max_batch_size))),
            is_running: Arc::new(RwLock::new(false)),
            _phantom: PhantomData,
        }
    }

    pub async fn start(&self) -> Result<(), Error> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        *is_running = true;
        drop(is_running);

        let current_batch = self.current_batch.clone();
        let topic = self.topic.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = interval(config.flush_interval);

            while *is_running.read().await {
                interval.tick().await;

                let mut batch = current_batch.lock().await;
                if !batch.is_empty() {
                    match Self::publish_batch(&topic, batch.drain(..).collect()).await {
                        Ok(message_ids) => {
                            debug!(
                                "Successfully published batch of {} messages",
                                message_ids.len()
                            );
                        }
                        Err(e) => {
                            error!("Failed to publish batch: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Error> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        *is_running = false;
        drop(is_running);

        self.flush().await?;

        Ok(())
    }

    pub async fn publish(&self, message: T) -> Result<(), Error> {
        let mut batch = self.current_batch.lock().await;
        batch.push(message);

        if batch.len() >= self.config.max_batch_size {
            let messages = batch.drain(..).collect();
            drop(batch);
            Self::publish_batch(&self.topic, messages).await?;
        }

        Ok(())
    }

    pub async fn flush(&self) -> Result<Vec<String>, Error> {
        let mut batch = self.current_batch.lock().await;
        if batch.is_empty() {
            return Ok(Vec::new());
        }

        let messages = batch.drain(..).collect();
        drop(batch);
        Self::publish_batch(&self.topic, messages).await
    }

    async fn publish_batch(topic: &Topic<T>, messages: Vec<T>) -> Result<Vec<String>, Error> {
        let mut message_ids = Vec::with_capacity(messages.len());

        for message in messages {
            match topic.publish(message).await {
                Ok(id) => message_ids.push(id),
                Err(e) => {
                    error!("Failed to publish message in batch: {}", e);
                    return Err(e);
                }
            }
        }

        Ok(message_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::TopicConfig;
    use std::time::Duration;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestMessage {
        value: i32,
    }

    #[tokio::test]
    async fn test_batch_publisher() {
        let topic = Arc::new(Topic::new("test".to_string(), TopicConfig::default()));

        let config = BatchPublisherConfig {
            max_batch_size: 2,
            flush_interval: Duration::from_millis(100),
            max_queued_batches: 5,
        };

        let publisher = BatchPublisher::new(topic, config);
        publisher.start().await.unwrap();

        // Publish some messages
        for i in 0..3 {
            publisher.publish(TestMessage { value: i }).await.unwrap();
        }

        // Wait for flush interval
        tokio::time::sleep(Duration::from_millis(200)).await;

        publisher.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_manual_flush() {
        let topic = Arc::new(Topic::new("test".to_string(), TopicConfig::default()));

        let config = BatchPublisherConfig {
            max_batch_size: 10,
            flush_interval: Duration::from_secs(60),
            max_queued_batches: 5,
        };

        let publisher = BatchPublisher::new(topic, config);
        publisher.start().await.unwrap();

        // Publish some messages
        for i in 0..3 {
            publisher.publish(TestMessage { value: i }).await.unwrap();
        }

        // Manually flush
        let message_ids = publisher.flush().await.unwrap();
        assert_eq!(message_ids.len(), 3);

        publisher.stop().await.unwrap();
    }
}
