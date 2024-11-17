use crate::prelude::Error;
use reqwest::Client;
use serde::Serialize;
use std::sync::Arc;

pub struct Publisher {
    base_url: String,
    client: Arc<Client>,
}

impl Publisher {
    pub(crate) fn new(base_url: String, client: Arc<Client>) -> Self {
        Self { base_url, client }
    }

    pub async fn publish<T>(&self, topic: &str, message: T) -> Result<String, Error>
    where
        T: Serialize + Send + Sync,
    {
        let url = format!("{}/topics/{}/publish", self.base_url, topic);

        println!("Publishing message to topic: {}", topic);
        let response = self
            .client
            .post(&url)
            .json(&message)
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        println!("response: {:?}", response);

        let message_id = response
            .json::<String>()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;

        Ok(message_id)
    }
}
