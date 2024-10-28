use crate::service::PubSubService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

#[derive(serde::Deserialize)]
pub struct SubscriptionRequest {
    callback_url: String,
}

pub async fn publish<T>(
    State(service): State<Arc<PubSubService>>,
    Path(topic_name): Path<String>,
    Json(message): Json<T>,
) -> impl IntoResponse
where
    T: 'static + Send + Sync + Serialize + DeserializeOwned + Clone,
{
    match service.topic_registry.get::<T>(&topic_name).await {
        Some(topic) => match topic.publish(message).await {
            Ok(msg_id) => (StatusCode::OK, Json(msg_id)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => (StatusCode::NOT_FOUND, "Topic not found".to_string()).into_response(),
    }
}

pub async fn subscribe<T>(
    State(service): State<Arc<PubSubService>>,
    Path(topic_name): Path<String>,
    Json(request): Json<SubscriptionRequest>,
) -> impl IntoResponse
where
    T: 'static + Send + Sync + Serialize + DeserializeOwned + Clone,
{
    match service
        .bridge
        .create_subscription::<T>(&topic_name, request.callback_url)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn list_topics(State(service): State<Arc<PubSubService>>) -> impl IntoResponse {
    let topics = service.topic_registry.get_topic_names().await;
    Json(topics).into_response()
}
