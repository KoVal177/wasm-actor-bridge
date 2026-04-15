//! Core message types.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Trait bound for types that can cross the Worker boundary.
///
/// Automatically implemented for any `T: Serialize + DeserializeOwned + 'static`.
pub trait ActorMessage: Serialize + DeserializeOwned + 'static {}
impl<T: Serialize + DeserializeOwned + 'static> ActorMessage for T {}

/// An event received from a Web Worker.
///
/// Contains the deserialized payload and an optional binary sidecar
/// that was transferred zero-copy via `ArrayBuffer`.
#[derive(Debug, Clone)]
pub struct WorkerEvent<T> {
    /// The deserialized event payload.
    pub payload: T,
    /// Optional binary data transferred zero-copy.
    pub bytes: Option<Vec<u8>>,
}
