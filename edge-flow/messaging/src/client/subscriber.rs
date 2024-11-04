use crate::prelude::{Error, Event};
use axum::{response::IntoResponse, routing::post, Json, Router};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use std::sync::Arc;

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

    pub async fn subscribe<F>(
        &self,
        topic: &str,
        handle_name: &str,
        callback: F,
    ) -> Result<SubscriptionHandle, Error>
    where
        F: Fn(Event<T>) -> Result<(), Error> + Send + Sync + 'static + Clone,
    {
        let (addr, shutdown_tx) = self.start_callback_server(callback).await?;

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

        Ok(SubscriptionHandle::new(
            handle_name.to_string(),
            shutdown_tx,
        ))
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

        let app = Router::new().route(
            "/",
            post(move |payload: Json<Event<T>>| {
                let callback = callback.clone();
                async move {
                    match callback(payload.0) {
                        Ok(_) => StatusCode::OK.into_response(),
                        Err(e) => {
                            println!("Error in callback: {:?}", e);
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
                },
            }
        });

        Ok((addr, shutdown_tx))
    }
}

#[derive(Debug, Clone)]
pub struct SubscriptionHandle {
    name: String,
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
}

impl SubscriptionHandle {
    pub fn new(name: String, shutdown_tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self { name, shutdown_tx }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
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

#[derive(Debug, Clone)]
pub struct HandlerStore {
    handlers: Vec<SubscriptionHandle>,
}

impl HandlerStore {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn add_handler(&mut self, handler: SubscriptionHandle) {
        self.handlers.push(handler);
    }

    pub fn remove_handler(&mut self, name: &str) {
        self.handlers.retain(|h| h.name() != name);
    }

    pub async fn shutdown_all(&mut self) {
        for handler in self.handlers.drain(..) {
            let _ = handler.shutdown_tx.send(()).await;
        }
    }

    fn get_shutdown_txs(&self) -> Vec<tokio::sync::mpsc::Sender<()>> {
        self.handlers
            .iter()
            .map(|h| h.shutdown_tx.clone())
            .collect()
    }
}

impl Drop for HandlerStore {
    fn drop(&mut self) {
        let shutdown_txs = self.get_shutdown_txs();
        tokio::spawn(async move {
            for tx in shutdown_txs {
                let _ = tx.send(()).await;
            }
        });
    }
}
