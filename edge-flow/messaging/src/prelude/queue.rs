use crate::prelude::error::Error;
use crate::prelude::models::Event;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct MessageQueue<T>
where
    T: Clone + Send,
{
    messages: Arc<Mutex<VecDeque<Event<T>>>>,
    capacity: usize,
}

impl<T> MessageQueue<T>
where
    T: Clone + Send,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub async fn enqueue(&self, event: Event<T>) -> Result<(), Error> {
        let mut queue = self.messages.lock().await;
        if queue.len() >= self.capacity {
            return Err(Error::QueueFull("Message queue at capacity".into()));
        }
        queue.push_back(event);
        Ok(())
    }

    pub async fn enqueue_batch(&self, events: &[Event<T>]) -> Result<usize, Error> {
        let mut queue = self.messages.lock().await;
        let space_left = self.capacity - queue.len();

        if space_left == 0 {
            return Err(Error::QueueFull("Message queue at capacity".into()));
        }

        let to_enqueue = events.len().min(space_left);
        for event in events.iter().take(to_enqueue) {
            queue.push_back(event.clone());
        }

        if to_enqueue < events.len() {}

        Ok(to_enqueue)
    }

    pub async fn dequeue(&self) -> Option<Event<T>> {
        let mut queue = self.messages.lock().await;
        queue.pop_front()
    }

    pub async fn dequeue_batch(&self, max_batch_size: usize) -> Vec<Event<T>> {
        let mut queue = self.messages.lock().await;
        let batch_size = max_batch_size.min(queue.len());
        let mut batch = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            if let Some(event) = queue.pop_front() {
                batch.push(event);
            }
        }

        batch
    }

    pub async fn dequeue_batch_with_timeout(
        &self,
        max_batch_size: usize,
        timeout: Duration,
    ) -> Vec<Event<T>> {
        match tokio::time::timeout(timeout, self.dequeue_batch(max_batch_size)).await {
            Ok(batch) => batch,
            Err(_) => Vec::new(),
        }
    }

    pub async fn dequeue_with_timeout(&self, timeout: Duration) -> Option<Event<T>> {
        tokio::time::timeout(timeout, self.dequeue())
            .await
            .ok()
            .flatten()
    }

    pub async fn len(&self) -> usize {
        let queue = self.messages.lock().await;
        queue.len()
    }

    pub async fn is_empty(&self) -> bool {
        let queue = self.messages.lock().await;
        queue.is_empty()
    }

    pub async fn is_full(&self) -> bool {
        let queue = self.messages.lock().await;
        queue.len() >= self.capacity
    }

    pub async fn clear(&self) {
        let mut queue = self.messages.lock().await;
        queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::models::{Data, Metadata};
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_event(value: i32, id: &str) -> Event<i32> {
        Event {
            data: Data {
                value,
                timestamp: Utc::now(),
                metadata: Metadata {
                    source: "test".to_string(),
                    correlation_id: None,
                    trace_id: None,
                    attributes: HashMap::new(),
                },
            },
            event_type: "test".to_string(),
            event_id: id.to_string(),
            event_time: Utc::now(),
            ordering_key: None,
        }
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let queue = MessageQueue::<i32>::new(5);

        // Create test events
        let events: Vec<_> = (0..3)
            .map(|i| create_test_event(i, &format!("test{}", i)))
            .collect();

        // Test batch enqueue
        let enqueued = queue.enqueue_batch(&events).await.unwrap();
        assert_eq!(enqueued, 3);
        assert_eq!(queue.len().await, 3);

        // Test batch dequeue
        let dequeued = queue.dequeue_batch(2).await;
        assert_eq!(dequeued.len(), 2);
        assert_eq!(dequeued[0].data.value, 0);
        assert_eq!(dequeued[1].data.value, 1);
        assert_eq!(queue.len().await, 1);

        // Test batch enqueue with capacity limit
        let more_events: Vec<_> = (3..8)
            .map(|i| create_test_event(i, &format!("test{}", i)))
            .collect();

        let enqueued = queue.enqueue_batch(&more_events).await.unwrap();
        assert_eq!(enqueued, 4); // Only 4 spots left in queue
        assert!(queue.is_full().await);
    }

    #[tokio::test]
    async fn test_batch_timeout() {
        let queue = MessageQueue::<i32>::new(5);
        let timeout = Duration::from_millis(100);

        let batch = queue.dequeue_batch_with_timeout(10, timeout).await;
        assert!(batch.is_empty());
    }

    #[tokio::test]
    async fn test_queue_operations() {
        let queue = MessageQueue::<i32>::new(2);
        let event = create_test_event(42, "test1");

        // Test enqueue
        assert!(queue.enqueue(event.clone()).await.is_ok());
        assert_eq!(queue.len().await, 1);
        assert!(!queue.is_empty().await);
        assert!(!queue.is_full().await);

        // Test dequeue
        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.data.value, 42);
        assert!(queue.is_empty().await);

        // Test capacity
        assert!(queue.enqueue(event.clone()).await.is_ok());
        assert!(queue.enqueue(event.clone()).await.is_ok());
        assert!(queue.enqueue(event.clone()).await.is_err());
        assert!(queue.is_full().await);

        // Test clear
        queue.clear().await;
        assert!(queue.is_empty().await);
    }
}
