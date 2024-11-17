// TODO: Use websocket instead of HTTP for WebhookHandler

use crate::{
    prelude::{
        subscriber::{MessageHandler, QueuedSubscriber, Subscribers},
        Error, Event, SubscriptionConfig,
    },
    service::TopicRegistry,
};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

pub struct ServiceBridge {
    pub topic_registry: TopicRegistry,
}

impl ServiceBridge {
    pub fn new(topic_registry: TopicRegistry) -> Self {
        Self { topic_registry }
    }

    pub async fn create_subscription<T>(
        &self,
        topic_name: &str,
        callback_url: String,
    ) -> Result<(), Error>
    where
        T: 'static + Send + Sync + Serialize + DeserializeOwned + Clone,
    {
        let topic = self
            .topic_registry
            .get_topic_by_name::<T>(topic_name)
            .ok_or_else(|| Error::Config("Topic not found or type mismatch".into()))?;

        let handler = Box::new(WebhookHandler::new(callback_url));
        let subscriber = QueuedSubscriber::new(handler, SubscriptionConfig::new(), 1000);

        subscriber.start_processing().await?;
        topic.subscribe(Subscribers::from(subscriber)).await?;

        Ok(())
    }
}

struct WebhookHandler {
    callback_url: String,
    client: reqwest::Client,
}

impl WebhookHandler {
    fn new(callback_url: String) -> Self {
        println!("Creating new WebhookHandler");
        Self {
            callback_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl<T> MessageHandler<T> for WebhookHandler
where
    T: Send + Sync + Serialize + DeserializeOwned + Clone + 'static,
{
    async fn handle(&self, msg: Event<T>) -> Result<(), Error> {
        println!("Sending message to: {}", self.callback_url);
        self.client
            .post(&self.callback_url)
            .json(&msg)
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        Ok(())
    }
}
