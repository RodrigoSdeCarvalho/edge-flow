use crate::service::PubSubService;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(serde::Deserialize)]
pub struct SubscriptionRequest {
    callback_url: String,
}

pub async fn publish(
    State(service): State<Arc<PubSubService>>,
    Path(topic_name): Path<String>,
    Json(raw_message): Json<Value>,
) -> impl IntoResponse {
    match service
        .topic_registry
        .try_publish_to(&topic_name, raw_message)
        .await
    {
        Ok(msg_id) => (StatusCode::OK, Json(msg_id)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
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
    let topics = service.topic_registry.get_topic_names();
    Json(topics).into_response()
}
