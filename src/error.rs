//! Typed error hierarchy for the bridge.

/// Errors from the worker bridge.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// `Worker::new()` failed (invalid script URL, CSP, etc.).
    #[error("worker spawn failed: {0}")]
    Spawn(String),

    /// Serialisation of a command or event failed.
    #[error("serialisation failed: {0}")]
    Serialisation(String),

    /// `postMessage` rejected by the browser.
    #[error("postMessage failed: {0}")]
    PostMessage(String),

    /// Worker fired an `error` or `messageerror` event.
    #[error("worker error: {0}")]
    WorkerError(String),

    /// The worker handle has been terminated.
    #[error("bridge terminated")]
    Terminated,

    /// The event channel has been closed (receiver dropped).
    #[error("event channel closed")]
    ChannelClosed,
}
