mod publisher;
mod subscriber;

pub use publisher::Publisher;
pub use subscriber::HandlerStore;
pub use subscriber::Subscriber;

use reqwest::Client;
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

    pub fn publisher(&self) -> Publisher {
        Publisher::new(self.base_url.clone(), self.client.clone())
    }

    pub fn subscriber(&self) -> Subscriber {
        Subscriber::new(self.base_url.clone(), self.client.clone())
    }
}
