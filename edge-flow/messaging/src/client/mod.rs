mod publisher;
mod subscriber;

pub use publisher::Publisher;
pub use subscriber::Subscriber;

use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

pub struct PubSubClient {
    base_url: String,
    client: Arc<Client>,
}

impl PubSubClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Arc::new(Client::new()),
        }
    }

    pub fn publisher<T>(&self) -> Publisher<T>
    where
        T: Send + Sync + Serialize + 'static,
    {
        Publisher::new(self.base_url.clone(), self.client.clone())
    }

    pub fn subscriber<T>(&self) -> Subscriber<T>
    where
        T: Send + Sync + DeserializeOwned + Clone + 'static,
    {
        Subscriber::new(self.base_url.clone(), self.client.clone())
    }
}
