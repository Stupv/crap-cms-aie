//! Real-time event bus for broadcasting mutation events to subscribers.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;

/// The type of entity that was mutated.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EventTarget {
    Collection,
    Global,
}

/// The mutation operation that occurred.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EventOperation {
    Create,
    Update,
    Delete,
}

/// A mutation event broadcast to all subscribers.
#[derive(Debug, Clone, Serialize)]
pub struct MutationEvent {
    pub sequence: u64,
    pub timestamp: String,
    pub target: EventTarget,
    pub operation: EventOperation,
    pub collection: String,
    pub document_id: String,
    pub data: HashMap<String, serde_json::Value>,
}

/// Broadcast channel for real-time mutation events.
/// Clone is cheap (Arc internals).
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<MutationEvent>,
    sequence: Arc<AtomicU64>,
}

impl EventBus {
    /// Create a new EventBus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            sequence: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Publish a mutation event to all subscribers.
    /// Assigns a monotonic sequence number and ISO 8601 timestamp.
    /// Returns the published event, or None if there are no receivers.
    pub fn publish(
        &self,
        target: EventTarget,
        operation: EventOperation,
        collection: String,
        document_id: String,
        data: HashMap<String, serde_json::Value>,
    ) -> Option<MutationEvent> {
        let event = MutationEvent {
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            timestamp: chrono::Utc::now().to_rfc3339(),
            target,
            operation,
            collection,
            document_id,
            data,
        };

        match self.sender.send(event.clone()) {
            Ok(_) => Some(event),
            Err(_) => None, // no active receivers
        }
    }

    /// Subscribe to the event stream. Returns a receiver that gets all
    /// future events. Missed events (due to slow consumption) result in
    /// `broadcast::error::RecvError::Lagged`.
    pub fn subscribe(&self) -> broadcast::Receiver<MutationEvent> {
        self.sender.subscribe()
    }
}
