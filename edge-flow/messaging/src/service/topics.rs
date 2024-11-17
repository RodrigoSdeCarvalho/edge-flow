use crate::prelude::{config::*, duration_unit::DurationUnit, Error, Topic, TopicConfig};
use crate::service::PubSubService;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};

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
    pub tags: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlertMessage {
    pub severity: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub status: String,
}

#[macro_export]
macro_rules! define_topics {
    (
        $(
            $topic:ident => {
                type: $type:ty,
                config: $config:expr
            }
        ),* $(,)?
    ) => {
        pub struct TopicRegistry {
            $(
                pub $topic: Arc<Topic<$type>>,
            )*
        }

        impl TopicRegistry {
            pub fn new() -> Self {
                Self {
                    $(
                        $topic: Arc::new(Topic::new(
                            stringify!($topic).to_string(),
                            $config
                        )),
                    )*
                }
            }

            pub async fn try_publish_to(&self, topic_name: &str, value: Value) -> Result<String, Error> {
                match topic_name {
                    $(
                        stringify!($topic) => {
                            match serde_json::from_value::<$type>(value.clone()) {
                                Ok(msg) => self.$topic.publish(msg).await,
                                Err(e) => Err(Error::Serialization(e))
                            }
                        },
                    )*
                    _ => Err(Error::Config(format!("Topic {} not found", topic_name)))
                }
            }

            pub fn get_topic_names(&self) -> Vec<String> {
                vec![
                    $(stringify!($topic).to_string(),)*
                ]
            }

            pub fn get_topic_by_name<T>(&self, name: &str) -> Option<&Arc<Topic<T>>>
            where
                T: Clone + Send + Sync + 'static
            {
                match name {
                    $(
                        stringify!($topic) => {
                            let target_id = std::any::TypeId::of::<T>();
                            let current_id = std::any::TypeId::of::<$type>();

                            if target_id == current_id {
                                Some(unsafe {
                                    &*(&self.$topic as *const Arc<Topic<$type>> as *const Arc<Topic<T>>)
                                })
                            } else {
                                None
                            }
                        },
                    )*
                    _ => None
                }
            }
        }

        pub async fn subscribe_by_topic_name(
            service: &PubSubService,
            topic_name: &str,
            callback_url: String
        ) -> Result<(), Error> {
            match topic_name {
                $(
                    stringify!($topic) => {
                        service.bridge.create_subscription::<$type>(topic_name, callback_url).await
                    },
                )*
                _ => Err(Error::Config(format!("Unknown topic: {}", topic_name)))
            }
        }
    };
}

define_topics! {
    logs => {
        type: LogMessage,
        config: TopicConfig {
            delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(DurationUnit::Days.as_seconds() * 1)
        }
    },
    metrics => {
        type: MetricMessage,
        config: TopicConfig {
            delivery_guarantee: DeliveryGuarantee::ExactlyOnce,
            ordering_attribute: Some("timestamp".to_string()),
            message_retention: Duration::from_secs(1)
        }
    },
    alerts => {
        type: AlertMessage,
        config: TopicConfig {
            delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: Some("severity".to_string()),
            message_retention: Duration::from_secs(DurationUnit::Weeks.as_seconds() * 1)
        }
    }
}
