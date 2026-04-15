//! Response context for worker actors.
//!
//! On WASM, responses are posted to the main thread via `postMessage`.
//! On native (non-WASM), responses are collected in memory for testing.

// ── Native implementation (for testing) ──────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod native_impl {
    use std::cell::RefCell;
    use std::rc::Rc;

    struct ContextInner<Evt> {
        bytes: Option<Vec<u8>>,
        responses: RefCell<Vec<(Evt, Option<Vec<u8>>)>>,
    }

    /// Response context for dispatching events back to the main thread.
    ///
    /// On native targets, responses are collected in memory for testing.
    /// `Clone + 'static` — safe to move into spawned tasks.
    pub struct Context<Evt> {
        inner: Rc<ContextInner<Evt>>,
    }

    impl<Evt> Clone for Context<Evt> {
        fn clone(&self) -> Self {
            Self {
                inner: Rc::clone(&self.inner),
            }
        }
    }

    impl<Evt> Context<Evt> {
        /// Create a test context with optional incoming bytes.
        pub fn new(bytes: Option<Vec<u8>>) -> Self {
            Self {
                inner: Rc::new(ContextInner {
                    bytes,
                    responses: RefCell::new(Vec::new()),
                }),
            }
        }

        /// Access the binary payload from the incoming command (if any).
        pub fn bytes(&self) -> Option<&[u8]> {
            self.inner.bytes.as_deref()
        }

        /// Send an event back to the main thread.
        pub fn respond(&self, evt: Evt) {
            self.inner.responses.borrow_mut().push((evt, None));
        }

        /// Send an event with a binary sidecar back to the main thread.
        pub fn respond_bytes(&self, evt: Evt, bytes: Vec<u8>) {
            self.inner.responses.borrow_mut().push((evt, Some(bytes)));
        }

        /// Number of responses sent so far.
        pub fn response_count(&self) -> usize {
            self.inner.responses.borrow().len()
        }
    }

    impl<Evt: Clone> Context<Evt> {
        /// Collect all responses (test helper).
        pub fn responses(&self) -> Vec<(Evt, Option<Vec<u8>>)> {
            self.inner.responses.borrow().clone()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_impl::*;

// ── WASM implementation ──────────────────────────────────────

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::cell::Cell;
    use std::marker::PhantomData;
    use std::rc::Rc;

    use serde::Serialize;

    struct ContextInner {
        correlation_id: Option<u64>,
        bytes: Option<Vec<u8>>,
        replied_correlated: Cell<bool>,
    }

    /// Response context for dispatching events back to the main thread.
    ///
    /// `Clone + 'static` — safe to move into spawned tasks on the Worker.
    pub struct Context<Evt> {
        inner: Rc<ContextInner>,
        _phantom: PhantomData<fn(Evt)>,
    }

    impl<Evt> Clone for Context<Evt> {
        fn clone(&self) -> Self {
            Self {
                inner: Rc::clone(&self.inner),
                _phantom: PhantomData,
            }
        }
    }

    impl<Evt> Context<Evt> {
        pub(crate) fn new(correlation_id: Option<u64>, bytes: Option<Vec<u8>>) -> Self {
            Self {
                inner: Rc::new(ContextInner {
                    correlation_id,
                    bytes,
                    replied_correlated: Cell::new(false),
                }),
                _phantom: PhantomData,
            }
        }

        /// Access the binary payload from the incoming command (if any).
        pub fn bytes(&self) -> Option<&[u8]> {
            self.inner.bytes.as_deref()
        }
    }

    #[allow(clippy::needless_pass_by_value)] // Taking ownership mirrors the main-thread API.
    impl<Evt: Serialize + 'static> Context<Evt> {
        /// Take the correlation ID for the first reply (RPC routing).
        fn take_correlation_id(&self) -> Option<u64> {
            if self.inner.replied_correlated.get() {
                return None;
            }
            self.inner.replied_correlated.set(true);
            self.inner.correlation_id
        }

        /// Send an event back to the main thread.
        pub fn respond(&self, evt: Evt) {
            let corr_id = self.take_correlation_id();
            if let Err(e) = crate::transfer::post_to_main(corr_id, &evt, None) {
                tracing::error!("respond failed: {e}");
            }
        }

        /// Send an event with a binary sidecar back to the main thread.
        pub fn respond_bytes(&self, evt: Evt, bytes: Vec<u8>) {
            let corr_id = self.take_correlation_id();
            if let Err(e) = crate::transfer::post_to_main(corr_id, &evt, Some(&bytes)) {
                tracing::error!("respond failed: {e}");
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
