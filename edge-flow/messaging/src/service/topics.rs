use crate::{Topic, TopicConfig};
use serde::{de::DeserializeOwned, Serialize};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Serialize)]
pub struct TopicInfo {
    pub name: String,
    pub type_name: String,
}

pub struct TopicWrapper {
    inner: Arc<dyn Any + Send + Sync>,
    type_id: TypeId,
    type_name: &'static str,
}

impl TopicWrapper {
    pub fn new<T>(topic: Arc<Topic<T>>) -> Self
    where
        T: 'static + Send + Sync,
    {
        Self {
            inner: topic,
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
        }
    }

    pub fn get_type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn downcast<T>(&self) -> Option<Arc<Topic<T>>>
    where
        T: 'static + Send + Sync,
    {
        if self.type_id == TypeId::of::<T>() {
            Some(self.inner.clone().downcast::<Topic<T>>().unwrap())
        } else {
            None
        }
    }
}

pub struct TopicRegistry {
    topics: RwLock<HashMap<String, TopicWrapper>>,
}

impl TopicRegistry {
    pub fn new() -> Self {
        Self {
            topics: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register<T>(&self, name: &str, config: TopicConfig) -> Arc<Topic<T>>
    where
        T: 'static + Send + Sync + Serialize + DeserializeOwned + Clone,
    {
        let mut topics = self.topics.write().await;
        let topic = Arc::new(Topic::new(name.to_string(), config));
        topics.insert(name.to_string(), TopicWrapper::new(topic.clone()));
        topic
    }

    pub async fn get<T>(&self, name: &str) -> Option<Arc<Topic<T>>>
    where
        T: 'static + Send + Sync + Clone,
    {
        let topics = self.topics.read().await;
        topics.get(name).and_then(|t| t.downcast::<T>())
    }

    pub async fn get_topic_names(&self) -> Vec<String> {
        let topics = self.topics.read().await;
        topics.keys().cloned().collect()
    }

    pub async fn get_topics_info(&self) -> Vec<TopicInfo> {
        let topics = self.topics.read().await;
        topics
            .iter()
            .map(|(name, wrapper)| TopicInfo {
                name: name.clone(),
                type_name: wrapper.get_type_name().to_string(),
            })
            .collect()
    }
}
