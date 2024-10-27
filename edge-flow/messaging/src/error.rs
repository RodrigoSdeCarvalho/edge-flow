#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("serialization error: {0}")] Serialization(#[from] serde_json::Error),

    #[error("transport error: {0}")] Transport(String),

    #[error("handler error: {0}")] Handler(String),

    #[error("timeout error")]
    Timeout,

    #[error("subscription error: {0}")] Subscription(String),

    #[error("configuration error: {0}")] Config(String),

    #[error("internal error: {0}")] Internal(String),
}
