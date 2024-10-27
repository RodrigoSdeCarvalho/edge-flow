# Introduction to Edge Flow

## Overview

Edge Flow is a comprehensive framework designed to solve the challenges of modern IoT and edge computing applications. It provides a cohesive set of tools and libraries that work together to handle data processing, event management, messaging, and logging in distributed environments.

## Core Philosophy

Edge Flow is built on several key principles:

1. **Safety First**: Leveraging Rust's safety guarantees throughout the system
2. **Data-Centric**: Everything revolves around well-defined data models
3. **Event-Driven**: Natural support for reactive and event-sourced architectures
4. **Observable**: Complete visibility into system behavior
5. **Extensible**: Easy integration with other languages and systems

## Core Components

### Data Library

The foundation of Edge Flow is its data handling system. It provides:

- Strong typing and validation
- Rich metadata support
- Efficient serialization
- Schema versioning
- Type-safe transformations

```rust
#[derive(Data)]
struct SensorReading {
    #[metadata(timestamp = "true")]
    timestamp: DateTime,
    value: f64,
    unit: String,
    #[metadata(device_id = "true")]
    device_id: String,
}
```

### Event System

Built on top of the data library, the event system provides:

- Event sourcing capabilities
- Event versioning
- Event store abstractions
- Replay capabilities
- Event handlers

```rust
#[derive(Event)]
struct TemperatureChanged {
    reading: SensorReading,
    previous_reading: Option,
}
```

### Messaging System

The messaging system handles communication between components:

- Pub/sub patterns
- Message queuing
- Delivery guarantees
- Backpressure handling
- Transport abstractions

```rust
#[derive(Topic)]
#[topic = "sensors/temperature/{device_id}"]
struct TemperatureTopic;

async fn handle_temperature(msg: Message) {
    println!("Temperature changed: {:?}", msg.data);
}
```

### Logging System

Comprehensive logging and tracing support:

- Structured logging
- Context preservation
- Log aggregation
- Query capabilities
- Metrics collection

```rust
#[instrument(skip(temperature))]
fn process_temperature(temperature: Temperature) {
    info!(
        value = temperature.value,
        unit = temperature.unit,
        "Processing temperature reading"
    );
}
```

## Getting Started

1. Add Edge Flow to your project:

    ```toml
    [
    dependencies
    ]
    edge-flow = "0.1"
    ```

2. Create your data models:

    ```rust
    use edge_flow::prelude::*;

    #[derive(Data)]
    struct Temperature {
        value: f64,
        unit: String,
    }
    ```

3. Define your events:

    ```rust
    #[derive(Event)]
    struct TemperatureReading(Temperature);
    ```

4. Set up message handling:

    ```rust
    #[derive(Topic)]
    #[topic = "sensors/temperature"]
    struct TemperatureTopic;

    async fn handle_temperature(msg: Message) {
        info!("Received temperature: {:?}", msg.data);
    }
    ```

## Next Steps

- [Messaging](messaging/docs.md)

## Contributing

We welcome contributions! See our [Contributing Guide](../CONTRIBUTING.md) for details.
