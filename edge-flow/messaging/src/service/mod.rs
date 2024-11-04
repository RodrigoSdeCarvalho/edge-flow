mod auth;
mod bridge;
mod handlers;
pub mod topics;

use axum::{
    routing::{get, post},
    Router,
};
pub use bridge::ServiceBridge;
use std::{net::SocketAddr, sync::Arc};
pub use topics::TopicRegistry;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::publish,
        handlers::subscribe,
        handlers::list_topics
    ),
    components(
        schemas(handlers::SubscriptionRequest)
    ),
    tags(
        (name = "pubsub", description = "Publish-Subscribe API endpoints")
    )
)]
struct ApiDoc;

pub struct PubSubService<'a>
where
    'a: 'static,
{
    pub topic_registry: Arc<TopicRegistry<'a>>,
    bridge: Arc<ServiceBridge<'a>>,
}

impl<'a> PubSubService<'a>
where
    'a: 'static,
{
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
            .route("/topics/:topic/publish", post(handlers::publish))
            .route("/topics/:topic/subscribe", post(handlers::subscribe))
            .route("/topics", get(handlers::list_topics))
            .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .with_state(Arc::new(self));

        axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
            .await
            .unwrap();
    }
}
