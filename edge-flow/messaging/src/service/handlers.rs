use crate::service::{topics::subscribe_by_topic_name, PubSubService};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use serde_json::Value;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(serde::Deserialize, ToSchema)]
pub struct SubscriptionRequest {
    callback_url: String,
}

/// Publish a message to a topic
#[utoipa::path(
    post,
    path = "/topics/{topic}/publish",
    params(
        ("topic" = String, Path, description = "Name of the topic")
    ),
    request_body = Value,
    responses(
        (status = 200, description = "Message published successfully", body = String),
        (status = 400, description = "Bad request", body = String)
    )
)]
pub async fn publish<'a>(
    State(service): State<Arc<PubSubService<'a>>>,
    Path(topic_name): Path<String>,
    Json(raw_message): Json<Value>,
) -> impl IntoResponse {
    println!("Publishing to topic: {}", topic_name);
    match service
        .topic_registry
        .try_publish_to(&topic_name, raw_message)
        .await
    {
        Ok(msg_id) => (StatusCode::OK, Json(msg_id)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// Subscribe to a topic
#[utoipa::path(
    post,
    path = "/topics/{topic}/subscribe",
    params(
        ("topic" = String, Path, description = "Name of the topic")
    ),
    request_body = SubscriptionRequest,
    responses(
        (status = 200, description = "Subscription created successfully"),
        (status = 400, description = "Bad request", body = String)
    )
)]
pub async fn subscribe<'a>(
    State(service): State<Arc<PubSubService<'a>>>,
    Path(topic_name): Path<String>,
    Json(request): Json<SubscriptionRequest>,
) -> impl IntoResponse {
    println!("Subscribing to topic: {}", topic_name);

    match subscribe_by_topic_name(&service, &topic_name, request.callback_url).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// List all topics
#[utoipa::path(
    get,
    path = "/topics",
    responses(
        (status = 200, description = "List of topics", body = Vec<String>)
    )
)]
pub async fn list_topics<'a>(State(service): State<Arc<PubSubService<'a>>>) -> impl IntoResponse {
    let topics = service.topic_registry.get_topic_names();
    Json(topics).into_response()
}
