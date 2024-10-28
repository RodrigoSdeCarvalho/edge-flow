use crate::Error;
use reqwest::Client;
use serde::Serialize;
use std::marker::PhantomData;
use std::sync::Arc;

pub struct Publisher<T> {
    base_url: String,
    client: Arc<Client>,
    _phantom: PhantomData<T>,
}

impl<T> Publisher<T>
where
    T: Serialize + Send + Sync + 'static,
{
    pub(crate) fn new(base_url: String, client: Arc<Client>) -> Self {
        Self {
            base_url,
            client,
            _phantom: PhantomData,
        }
    }

    pub async fn publish(&self, topic: &str, message: T) -> Result<String, Error> {
        let url = format!("{}/topics/{}/publish", self.base_url, topic);

        let response = self
            .client
            .post(&url)
            .json(&message)
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;

        let message_id = response
            .json::<String>()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;

        Ok(message_id)
    }
}
