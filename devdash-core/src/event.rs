// devdash-core/src/event.rs
use crossbeam::channel::{Receiver, Sender, unbounded};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Event payload - can be any type
#[derive(Clone)]
pub struct EventPayload(Arc<dyn Any + Send + Sync>);

impl EventPayload {
    pub fn new<T: Any + Send + Sync>(data: T) -> Self {
        Self(Arc::new(data))
    }

    pub fn downcast<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        Arc::downcast(self.0.clone()).ok()
    }
}

/// Event with topic and payload
#[derive(Clone)]
pub struct Event {
    pub topic: String,
    pub payload: EventPayload,
}

impl Event {
    pub fn new<T: Any + Send + Sync>(topic: impl Into<String>, data: T) -> Self {
        Self {
            topic: topic.into(),
            payload: EventPayload::new(data),
        }
    }
}

/// Subscription handle - dropping this unsubscribes
pub struct Subscription {
    id: usize,
    bus: Arc<EventBusInner>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut subs = self.bus.subscriptions.write().unwrap();
        subs.remove(&self.id);
    }
}

/// Internal bus state
struct EventBusInner {
    subscriptions: RwLock<HashMap<usize, (String, Sender<Event>)>>,
    next_id: std::sync::atomic::AtomicUsize,
}

/// Lockfree event bus with topic-based pub/sub
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                subscriptions: RwLock::new(HashMap::new()),
                next_id: std::sync::atomic::AtomicUsize::new(0),
            }),
        }
    }

    /// Publish an event to all matching subscribers
    pub fn publish(&self, event: Event) {
        let subs = self.inner.subscriptions.read().unwrap();

        for (pattern, tx) in subs.values() {
            if Self::topic_matches(&event.topic, pattern) {
                // Ignore send errors (subscriber dropped)
                let _ = tx.send(event.clone());
            }
        }
    }

    /// Subscribe to topics with wildcard support
    /// Returns (Subscription, Receiver) - drop Subscription to unsubscribe
    pub fn subscribe(&self, pattern: impl Into<String>) -> (Subscription, Receiver<Event>) {
        let (tx, rx) = unbounded();
        let pattern = pattern.into();

        let id = self
            .inner
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        {
            let mut subs = self.inner.subscriptions.write().unwrap();
            subs.insert(id, (pattern, tx));
        }

        let sub = Subscription {
            id,
            bus: self.inner.clone(),
        };

        (sub, rx)
    }

    /// Check if topic matches pattern (supports * wildcard)
    fn topic_matches(topic: &str, pattern: &str) -> bool {
        // Exact match
        if topic == pattern {
            return true;
        }

        // Wildcard matching
        let topic_parts: Vec<&str> = topic.split('.').collect();
        let pattern_parts: Vec<&str> = pattern.split('.').collect();

        if pattern_parts.len() > topic_parts.len() {
            return false;
        }

        for (i, pattern_part) in pattern_parts.iter().enumerate() {
            if *pattern_part == "*" {
                // Wildcard at end matches everything remaining
                if i == pattern_parts.len() - 1 {
                    return true;
                }
                continue;
            }

            if i >= topic_parts.len() || topic_parts[i] != *pattern_part {
                return false;
            }
        }

        pattern_parts.len() == topic_parts.len()
    }
}

// Common event types
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_used: u64,
    pub memory_total: u64,
}

#[derive(Debug, Clone)]
pub struct GitBranchChange {
    pub from: String,
    pub to: String,
    pub repo_path: String,
}

#[derive(Debug, Clone)]
pub struct ProcessUpdate {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_matching() {
        assert!(EventBus::topic_matches("system.cpu", "system.cpu"));
        assert!(EventBus::topic_matches("system.cpu", "system.*"));
        assert!(EventBus::topic_matches("system.cpu.usage", "system.*"));
        assert!(!EventBus::topic_matches("git.branch", "system.*"));
        assert!(EventBus::topic_matches("any.thing.here", "*"));
    }

    #[test]
    fn test_pubsub() {
        let bus = EventBus::new();
        let (_sub, rx) = bus.subscribe("system.*");

        let metrics = SystemMetrics {
            cpu_usage: 50.0,
            memory_used: 1024,
            memory_total: 2048,
        };

        bus.publish(Event::new("system.metrics", metrics.clone()));

        let event = rx.recv().unwrap();
        assert_eq!(event.topic, "system.metrics");

        let received: Arc<SystemMetrics> = event.payload.downcast().unwrap();
        assert_eq!(received.cpu_usage, 50.0);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = EventBus::new();
        let (sub, rx) = bus.subscribe("test");

        bus.publish(Event::new("test", 42));
        assert!(rx.recv().is_ok());

        drop(sub); // Unsubscribe

        bus.publish(Event::new("test", 43));
        assert!(rx.recv().is_err()); // Channel closed
    }
}
