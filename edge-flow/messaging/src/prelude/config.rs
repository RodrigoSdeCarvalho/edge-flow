use crate::prelude::duration_unit::DurationUnit;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum DeliveryGuarantee {
    AtLeastOnce,
    ExactlyOnce,
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: i32,
    pub min_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            min_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TopicConfig {
    pub delivery_guarantee: DeliveryGuarantee,
    pub ordering_attribute: Option<String>,
    pub message_retention: Duration,
}

impl Default for TopicConfig {
    fn default() -> Self {
        Self {
            delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
            ordering_attribute: None,
            message_retention: Duration::from_secs(DurationUnit::Days.as_seconds() * 1),
        }
    }
}

#[derive(Clone)]
pub struct SubscriptionConfig {
    pub max_concurrency: i32,
    pub ack_deadline: Duration,
    pub retry_policy: RetryPolicy,
    pub delivery_guarantee: DeliveryGuarantee,
}

impl SubscriptionConfig {
    pub fn new() -> Self {
        Self {
            max_concurrency: 1,
            ack_deadline: Duration::from_secs(30),
            retry_policy: RetryPolicy::default(),
            delivery_guarantee: DeliveryGuarantee::AtLeastOnce,
        }
    }

    pub fn with_concurrency(mut self, max_concurrency: i32) -> Self {
        self.max_concurrency = max_concurrency;
        self
    }

    pub fn with_ack_deadline(mut self, ack_deadline: Duration) -> Self {
        self.ack_deadline = ack_deadline;
        self
    }

    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    pub fn with_delivery_guarantee(mut self, guarantee: DeliveryGuarantee) -> Self {
        self.delivery_guarantee = guarantee;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.min_backoff, Duration::from_secs(1));
        assert_eq!(policy.max_backoff, Duration::from_secs(60));
    }

    #[test]
    fn test_topic_config_default() {
        let config = TopicConfig::default();
        matches!(config.delivery_guarantee, DeliveryGuarantee::AtLeastOnce);
        assert!(config.ordering_attribute.is_none());
        assert_eq!(config.message_retention, Duration::from_secs(86400));
    }

    #[test]
    fn test_subscription_config_builder() {
        let config = SubscriptionConfig::new()
            .with_concurrency(5)
            .with_ack_deadline(Duration::from_secs(60))
            .with_retry_policy(RetryPolicy::default());

        assert_eq!(config.max_concurrency, 5);
        assert_eq!(config.ack_deadline, Duration::from_secs(60));
    }
}
