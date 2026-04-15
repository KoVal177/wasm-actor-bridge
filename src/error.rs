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

    /// Script-level error (wrong URL, CSP, syntax error in worker script).
    /// Not automatically recoverable — the script itself is broken.
    #[error("worker script error: {0}")]
    WorkerError(String),

    /// Worker process crashed (WASM panic, OOM, unreachable trap).
    /// A Supervisor should attempt respawn.
    #[error("worker crashed")]
    WorkerCrashed,

    /// The worker handle has been terminated.
    #[error("bridge terminated")]
    Terminated,

    /// The event channel has been closed (receiver dropped).
    #[error("event channel closed")]
    ChannelClosed,

    /// The command channel is full — the worker cannot accept more commands.
    #[error("command channel full")]
    ChannelFull,

    /// Builder configuration is invalid.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}

impl BridgeError {
    /// True if the error indicates the worker process died unexpectedly
    /// and respawn may succeed.
    pub fn is_crash(&self) -> bool {
        matches!(self, Self::WorkerCrashed)
    }

    /// True if the error is permanent and respawn will not help.
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::Spawn(_) | Self::WorkerError(_) | Self::InvalidConfig(_)
        )
    }
}
