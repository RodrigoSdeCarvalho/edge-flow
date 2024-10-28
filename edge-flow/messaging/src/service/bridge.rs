use crate::{
    models::Context,
    service::TopicRegistry,
    subscriber::{MessageHandler, QueuedSubscriber},
    Error, Event, SubscriptionConfig,
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

pub struct ServiceBridge {
    topic_registry: Arc<TopicRegistry>,
}

impl ServiceBridge {
    pub fn new(topic_registry: Arc<TopicRegistry>) -> Self {
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
            .get::<T>(topic_name)
            .await
            .ok_or_else(|| Error::Config("Topic not found".into()))?;

        let handler = Arc::new(WebhookHandler::new(callback_url));
        let subscriber =
            QueuedSubscriber::new(handler.clone(), SubscriptionConfig::new(handler), 1000);

        subscriber.start_processing().await?;
        topic.subscribe(subscriber).await?;

        Ok(())
    }
}

struct WebhookHandler {
    callback_url: String,
    client: reqwest::Client,
}

impl WebhookHandler {
    fn new(callback_url: String) -> Self {
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
    async fn handle(&self, _ctx: &Context, msg: Event<T>) -> Result<(), Error> {
        self.client
            .post(&self.callback_url)
            .json(&msg)
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        Ok(())
    }
}
