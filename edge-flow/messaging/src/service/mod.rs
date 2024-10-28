mod bridge;
mod handlers;
mod topics;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

pub use bridge::ServiceBridge;
pub use topics::TopicRegistry;

pub struct PubSubService {
    pub topic_registry: Arc<TopicRegistry>,
    bridge: Arc<ServiceBridge>,
}

impl PubSubService {
    pub fn new() -> Self {
        let topic_registry = Arc::new(TopicRegistry::new());
        let bridge = Arc::new(ServiceBridge::new(topic_registry.clone()));

        Self {
            topic_registry,
            bridge,
        }
    }

    pub async fn run(self, addr: SocketAddr) {
        let app = Router::new()
            .route("/topics/:topic/publish", post(handlers::publish::<String>))
            .route(
                "/topics/:topic/subscribe",
                post(handlers::subscribe::<String>),
            )
            .route("/topics", get(handlers::list_topics))
            .with_state(Arc::new(self));

        axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
            .await
            .unwrap();
    }
}
