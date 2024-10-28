use dotenv::dotenv;
use messaging::{service::PubSubService, DeliveryGuarantee, Error, TopicConfig};
use std::{net::SocketAddr, time::Duration};
use tracing::{error, info};

pub mod default_topics {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct LogMessage {
        pub level: String,
        pub message: String,
        pub timestamp: DateTime<Utc>,
        pub service: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct MetricMessage {
        pub name: String,
        pub value: f64,
        pub unit: String,
        pub timestamp: DateTime<Utc>,
        pub tags: std::collections::HashMap<String, String>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct AlertMessage {
        pub severity: String,
        pub message: String,
        pub timestamp: DateTime<Utc>,
        pub source: String,
        pub status: String,
    }
}

async fn register_default_topics(service: &PubSubService) -> Result<(), Error> {
    use default_topics::*;

    let config = TopicConfig {
        delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
        ordering_attribute: None,
        message_retention: Duration::from_secs(3600), // 1 hour
    };

    service
        .topic_registry
        .register::<LogMessage>("logs", config.clone())
        .await;
    service
        .topic_registry
        .register::<MetricMessage>("metrics", config.clone())
        .await;
    service
        .topic_registry
        .register::<AlertMessage>("alerts", config.clone())
        .await;

    info!("Default topics registered: logs, metrics, alerts");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_thread_ids(true)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_ansi(true)
        .init();

    info!("Starting Edge Flow messaging service...");

    let service = PubSubService::new();

    if let Err(e) = register_default_topics(&service).await {
        error!("Failed to register default topics: {}", e);
        return Err(e);
    }

    let port = std::env::var("PORT")
        .map(|p| p.parse::<u16>().expect("Invalid PORT"))
        .unwrap_or(3000);

    let addr: SocketAddr = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()
        .expect("Invalid BIND_ADDRESS");

    let addr = SocketAddr::new(addr.ip(), port);

    info!("Starting server on {}", addr);
    info!("Available topics: logs, metrics, alerts");
    info!("Default topics support the following message types:");
    info!("  - logs: LogMessage (level, message, timestamp, service)");
    info!("  - metrics: MetricMessage (name, value, unit, timestamp, tags)");
    info!("  - alerts: AlertMessage (severity, message, timestamp, source, status)");

    service.run(addr).await;

    Ok(())
}
