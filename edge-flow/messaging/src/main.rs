use dotenv::dotenv;
use messaging::{prelude::Error, service::PubSubService};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    let service = PubSubService::new();

    let port = std::env::var("PORT")
        .map(|p| p.parse::<u16>().expect("Invalid PORT"))
        .unwrap_or(3000);

    let addr: SocketAddr = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()
        .expect("Invalid BIND_ADDRESS");

    let addr = SocketAddr::new(addr.ip(), port);

    service.run(addr).await;

    Ok(())
}
