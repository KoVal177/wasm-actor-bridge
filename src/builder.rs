//! Builder for spawning a Web Worker with validated configuration.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::marker::PhantomData;

    use crate::error::BridgeError;
    use crate::handle::WorkerHandle;
    use crate::message::ActorMessage;
    use crate::spawn::spawn_worker_with_capacity;
    use crate::stream::EventStream;

    /// Builder for spawning a typed Web Worker bridge.
    ///
    /// Validates configuration before spawning. Sends the init payload
    /// as the first message to the worker, ensuring the dispatch loop
    /// receives it before any commands.
    ///
    /// # Example (pseudo-code)
    ///
    /// ```ignore
    /// let (handle, events) = SupervisorBuilder::<MyCmd, MyEvt, MyInit>::new("/worker.js")
    ///     .evt_capacity(32)
    ///     .init(MyInit { config: "..." })
    ///     .build()?;
    /// ```
    pub struct SupervisorBuilder<'a, Cmd, Evt, Init> {
        script_url: &'a str,
        evt_capacity: usize,
        init: Option<Init>,
        _marker: PhantomData<fn(Cmd, Evt)>,
    }

    impl<'a, Cmd, Evt, Init> SupervisorBuilder<'a, Cmd, Evt, Init>
    where
        Cmd: ActorMessage,
        Evt: ActorMessage,
        Init: serde::Serialize,
    {
        /// Create a new builder for the given worker script URL.
        ///
        /// Default event capacity is 16. Call `.evt_capacity()` to override.
        pub fn new(script_url: &'a str) -> Self {
            Self {
                script_url,
                evt_capacity: 16,
                init: None,
                _marker: PhantomData,
            }
        }

        /// Set the bounded event channel capacity (worker → main).
        ///
        /// Must be > 0. Validated at `.build()` time.
        #[must_use]
        pub fn evt_capacity(mut self, capacity: usize) -> Self {
            self.evt_capacity = capacity;
            self
        }

        /// Set the init payload sent as the first message to the worker.
        #[must_use]
        pub fn init(mut self, init: Init) -> Self {
            self.init = Some(init);
            self
        }

        /// Spawn the worker, send init, and return the typed handle + event stream.
        ///
        /// # Errors
        ///
        /// - [`BridgeError::InvalidConfig`] if `evt_capacity` is 0 or `init` is missing.
        /// - [`BridgeError::Spawn`] if the worker fails to start.
        /// - [`BridgeError::Serialisation`] / [`BridgeError::PostMessage`] if init delivery fails.
        pub fn build(self) -> Result<(WorkerHandle<Cmd, Evt>, EventStream<Evt>), BridgeError> {
            if self.evt_capacity == 0 {
                return Err(BridgeError::InvalidConfig(
                    "evt_capacity must be > 0".into(),
                ));
            }
            let init = self
                .init
                .ok_or_else(|| BridgeError::InvalidConfig("init payload is required".into()))?;

            let (handle, events) = spawn_worker_with_capacity(self.script_url, self.evt_capacity)?;

            handle.send_init(&init)?;

            Ok((handle, events))
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
