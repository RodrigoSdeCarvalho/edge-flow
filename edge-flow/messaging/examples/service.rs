use messaging::service::PubSubService;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let service = PubSubService::new();
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    service.run(addr).await;
}
