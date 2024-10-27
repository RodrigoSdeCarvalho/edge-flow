# Edge Flow Messaging Library

A robust, type-safe messaging library for IoT and Edge computing applications built in Rust. This library provides an event-driven messaging system with support for pub/sub patterns, flexible message handling, and configurable delivery guarantees.

## Table of Contents

- [Edge Flow Messaging Library](#edge-flow-messaging-library)
  - [Table of Contents](#table-of-contents)
  - [Features](#features)
  - [Getting Started](#getting-started)
  - [Core Concepts](#core-concepts)
    - [Event \& Data Model](#event--data-model)
    - [Topics](#topics)
    - [Subscribers](#subscribers)
    - [Configuration](#configuration)
      - [Topic Configuration](#topic-configuration)
      - [Subscription Configuration](#subscription-configuration)
      - [Retry Policy](#retry-policy)
    - [Error Handling](#error-handling)
  - [Usage Examples](#usage-examples)
    - [Basic Publishing and Subscription](#basic-publishing-and-subscription)
    - [Custom Message Handler](#custom-message-handler)
  - [Testing](#testing)
  - [License](#license)

## Features

- Generic `Event<Data<T>>` structure for type-safe message handling
- Configurable delivery guarantees (At-least-once, Exactly-once)
- Asynchronous messaging with Tokio
- Flexible subscription handling with trait-based interfaces
- Comprehensive error handling and retry policies
- Full metadata support for tracing and correlation
- Thread-safe implementation

## Getting Started

Add the library to your `Cargo.toml`:

```toml
[dependencies]
messaging = "0.1.0"
```

Basic usage example:

```rust
use edge_flow_messaging::{Topic, TopicConfig, DeliveryGuarantee};
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
struct TemperatureReading {
    value: f64,
    sensor_id: String,
}

// Create a topic
let config = TopicConfig::default();
let topic: Topic<TemperatureReading> = Topic::new("temperature".to_string(), config);

// Subscribe to messages
let subscriber = FunctionSubscriber::new(|event| {
    println!("Received temperature: {}", event.data.value.value);
    Ok(())
});
topic.subscribe(subscriber).await?;

// Publish a message
let reading = TemperatureReading {
    value: 25.5,
    sensor_id: "sensor1".to_string(),
};
let msg_id = topic.publish(reading).await?;
```

## Core Concepts

### Event & Data Model

The messaging system is built around three core types:

```rust
struct Event<T> {
    data: Data<T>,
    event_type: String,
    event_id: String,
    event_time: DateTime<Utc>,
    ordering_key: Option<String>,
}

struct Data<T> {
    value: T,
    timestamp: DateTime<Utc>,
    metadata: Metadata,
}

struct Metadata {
    source: String,
    correlation_id: Option<String>,
    trace_id: Option<String>,
    attributes: HashMap<String, String>,
}
```

- `Event<T>`: The top-level container for messages
- `Data<T>`: Contains the actual payload and associated metadata
- `Metadata`: Holds contextual information for tracing and correlation

### Topics

Topics are the main abstraction for message publishing and subscription. They provide:

- Message distribution to subscribers
- Delivery guarantee enforcement
- Subscriber management
- Message retention policies

```rust
let topic: Topic<T> = Topic::new(
    "my-topic".to_string(),
    TopicConfig {
        delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
        ordering_attribute: None,
        message_retention: Duration::from_secs(86400),
    }
);
```

### Subscribers

Subscribers can be implemented in two ways:

1. Using the `Subscriber` trait:

    ```rust
    #[async_trait]
    pub trait Subscriber<T>: Send + Sync {
        async fn receive(&self, event: Event<T>) -> Result<(), Error>;
    }
    ```

2. Using the `MessageHandler` trait with subscription configuration:

    ```rust
    #[async_trait]
    pub trait MessageHandler<T>: Send + Sync {
        async fn handle(&self, ctx: &Context, msg: Event<T>) -> Result<(), Error>;
    }
    ```

### Configuration

#### Topic Configuration

```rust
struct TopicConfig {
    pub delivery_guarantee: DeliveryGuarantee,
    pub ordering_attribute: Option<String>,
    pub message_retention: Duration,
}
```

#### Subscription Configuration

```rust
struct SubscriptionConfig<T> {
    pub handler: Arc<dyn MessageHandler<T> + Send + Sync>,
    pub max_concurrency: i32,
    pub ack_deadline: Duration,
    pub retry_policy: RetryPolicy,
}
```

#### Retry Policy

```rust
struct RetryPolicy {
    pub max_retries: i32,
    pub min_backoff: Duration,
    pub max_backoff: Duration,
}
```

### Error Handling

The library defines several error types:

```rust
pub enum Error {
    Serialization(serde_json::Error),
    Transport(String),
    Handler(String),
    Timeout,
    Subscription(String),
    Config(String),
    Internal(String),
}
```

## Usage Examples

### Basic Publishing and Subscription

```rust
// Define a message type
#[derive(Clone, Serialize, Deserialize)]
struct SensorData {
    sensor_id: String,
    value: f64,
    timestamp: DateTime<Utc>,
}

// Create a topic
let topic: Topic<SensorData> = Topic::new(
    "sensors".to_string(),
    TopicConfig::default(),
);

// Create a subscriber
let subscriber = FunctionSubscriber::new(|event| {
    println!("Received data from sensor {}: {}", 
        event.data.value.sensor_id,
        event.data.value.value);
    Ok(())
});

// Subscribe
topic.subscribe(subscriber).await?;

// Publish messages
let data = SensorData {
    sensor_id: "sensor1".to_string(),
    value: 42.0,
    timestamp: Utc::now(),
};

let msg_id = topic.publish(data).await?;
```

### Custom Message Handler

```rust
struct CustomHandler;

#[async_trait]
impl MessageHandler<SensorData> for CustomHandler {
    async fn handle(&self, ctx: &Context, msg: Event<SensorData>) -> Result<(), Error> {
        // Process the message
        println!("Processing message with ID: {}", msg.event_id);
        Ok(())
    }
}

// Create subscription config
let config = SubscriptionConfig::new(Arc::new(CustomHandler))
    .with_concurrency(5)
    .with_ack_deadline(Duration::from_secs(30))
    .with_retry_policy(RetryPolicy::default());
```

## Testing

The library includes comprehensive tests for all major components. Run the test suite with:

```bash
cargo test
```

Test examples include:

- Message publishing and subscription
- Delivery guarantee enforcement
- Error handling and retry policies
- Concurrent message processing
- Topic configuration
- Subscriber management

## License

This project is licensed under the MIT License - see the LICENSE file for details.
