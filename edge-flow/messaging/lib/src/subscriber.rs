use async_trait::async_trait;
use std::marker::PhantomData;
use crate::error::Error;
use crate::models::{ Event, Context };

#[async_trait]
pub trait MessageHandler<T>: Send + Sync {
    async fn handle(&self, ctx: &Context, msg: Event<T>) -> Result<(), Error>;
}

#[async_trait]
pub trait Subscriber<T>: Send + Sync {
    async fn receive(&self, event: Event<T>) -> Result<(), Error>;
}

// Function subscriber implementation
pub struct FunctionSubscriber<T, F> where F: Fn(Event<T>) -> Result<(), Error> + Send + Sync {
    handler: F,
    _phantom: PhantomData<T>,
}

impl<T, F> FunctionSubscriber<T, F> where F: Fn(Event<T>) -> Result<(), Error> + Send + Sync {
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<T, F> Subscriber<T>
    for FunctionSubscriber<T, F>
    where T: Send + Sync, F: Fn(Event<T>) -> Result<(), Error> + Send + Sync
{
    async fn receive(&self, event: Event<T>) -> Result<(), Error> {
        (self.handler)(event)
    }
}

// Basic handler for testing
pub struct TestHandler<T> {
    _phantom: PhantomData<T>,
}

impl<T> TestHandler<T> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<T: Send + Sync> MessageHandler<T> for TestHandler<T> {
    async fn handle(&self, _ctx: &Context, _msg: Event<T>) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::models::{ Data, Metadata };
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_function_subscriber() {
        let subscriber = FunctionSubscriber::new(|event: Event<i32>| {
            assert_eq!(event.data.value, 42);
            Ok(())
        });

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
            event_id: "test".to_string(),
            event_time: Utc::now(),
            ordering_key: None,
        };

        subscriber.receive(event).await.unwrap();
    }

    #[tokio::test]
    async fn test_test_handler() {
        let handler = TestHandler::<i32>::new();

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
            event_id: "test".to_string(),
            event_time: Utc::now(),
            ordering_key: None,
        };

        let ctx = Context {
            trace_id: None,
            correlation_id: None,
            deadline: Utc::now(),
        };

        handler.handle(&ctx, event).await.unwrap();
    }
}
