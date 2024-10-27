use std::collections::HashMap;
use chrono::{ DateTime, Utc };
use serde::{ Serialize, Deserialize };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Data<T> {
    pub value: T,
    pub timestamp: DateTime<Utc>,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub source: String,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event<T> {
    pub data: Data<T>,
    pub event_type: String,
    pub event_id: String,
    pub event_time: DateTime<Utc>,
    pub ordering_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub trace_id: Option<String>,
    pub correlation_id: Option<String>,
    pub deadline: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let metadata = Metadata {
            source: "test".to_string(),
            correlation_id: Some("123".to_string()),
            trace_id: Some("456".to_string()),
            attributes: HashMap::new(),
        };

        let data = Data {
            value: 42,
            timestamp: Utc::now(),
            metadata,
        };

        let event = Event {
            data,
            event_type: "test_event".to_string(),
            event_id: "789".to_string(),
            event_time: Utc::now(),
            ordering_key: None,
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event<i32> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(event.event_id, deserialized.event_id);
        assert_eq!(event.data.value, deserialized.data.value);
    }
}
