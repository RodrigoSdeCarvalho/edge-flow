use crate::{Error, Event};
use axum::{response::IntoResponse, routing::post, Json, Router};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;
use tracing::{error, info};

pub struct Subscriber<T> {
    base_url: String,
    client: Arc<reqwest::Client>,
    _phantom: PhantomData<T>,
}

impl<T> Subscriber<T>
where
    T: DeserializeOwned + Send + Sync + Clone + 'static,
{
    pub(crate) fn new(base_url: String, client: Arc<reqwest::Client>) -> Self {
        Self {
            base_url,
            client,
            _phantom: PhantomData,
        }
    }

    pub async fn subscribe<F>(&self, topic: &str, callback: F) -> Result<SubscriptionHandle, Error>
    where
        F: Fn(Event<T>) -> Result<(), Error> + Send + Sync + 'static + Clone,
    {
        // Start local callback server
        let (addr, shutdown_tx) = self.start_callback_server(callback).await?;

        // Subscribe to topic with our callback URL
        let callback_url = format!("http://{}", addr);
        let url = format!("{}/topics/{}/subscribe", self.base_url, topic);

        self.client
            .post(&url)
            .json(&serde_json::json!({
                "callback_url": callback_url
            }))
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;

        Ok(SubscriptionHandle { shutdown_tx })
    }

    async fn start_callback_server<F>(
        &self,
        callback: F,
    ) -> Result<(std::net::SocketAddr, tokio::sync::mpsc::Sender<()>), Error>
    where
        F: Fn(Event<T>) -> Result<(), Error> + Send + Sync + 'static + Clone,
    {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
        let callback = Arc::new(callback);

        // Create router with callback handling
        let app = Router::new().route(
            "/",
            post(move |payload: Json<Event<T>>| {
                let callback = callback.clone();
                async move {
                    match callback(payload.0) {
                        Ok(_) => StatusCode::OK.into_response(),
                        Err(e) => {
                            error!("Error processing event: {}", e);
                            StatusCode::INTERNAL_SERVER_ERROR.into_response()
                        }
                    }
                }
            }),
        );

        let addr = find_available_port().await?;
        let server = axum::serve(
            tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| Error::Transport(e.to_string()))?,
            app,
        );

        tokio::spawn(async move {
            tokio::select! {
                _ = server => {},
                _ = shutdown_rx.recv() => {
                    info!("Shutting down callback server");
                },
            }
        });

        Ok((addr, shutdown_tx))
    }
}

pub struct SubscriptionHandle {
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        let shutdown_tx = self.shutdown_tx.clone();
        tokio::spawn(async move {
            let _ = shutdown_tx.send(()).await;
        });
    }
}

async fn find_available_port() -> Result<std::net::SocketAddr, Error> {
    tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map(|listener| listener.local_addr().unwrap())
        .map_err(|e| Error::Transport(e.to_string()))
}

use axum::http::StatusCode;
