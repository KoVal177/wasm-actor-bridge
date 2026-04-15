//! Typed handle for posting commands to a Web Worker.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::task::{Context, Poll};

    use serde::de::DeserializeOwned;

    use crate::error::BridgeError;
    use crate::message::ActorMessage;

    // ── Internal types ───────────────────────────────────────

    /// Raw response from the worker (pre-deserialization).
    pub(crate) struct RawResponse {
        pub js_value: wasm_bindgen::JsValue,
    }

    pub(crate) type PendingSender =
        futures_channel::oneshot::Sender<Result<RawResponse, BridgeError>>;

    pub(crate) type PendingMap = Rc<RefCell<HashMap<u64, PendingSender>>>;

    /// Shared state for the worker bridge.
    ///
    /// Owns the `Worker`, JS closures, and the pending RPC map.
    /// When the last `Arc<BridgeInner>` drops, the Worker is terminated,
    /// pending RPCs are rejected, and closures are released.
    pub(crate) struct BridgeInner {
        pub(crate) worker: send_wrapper::SendWrapper<web_sys::Worker>,
        pub(crate) terminated: AtomicBool,
        pub(crate) counter: AtomicU64,
        pub(crate) pending: send_wrapper::SendWrapper<PendingMap>,
        pub(crate) _onmessage: send_wrapper::SendWrapper<
            wasm_bindgen::prelude::Closure<dyn FnMut(web_sys::MessageEvent)>,
        >,
        pub(crate) _onerror: send_wrapper::SendWrapper<
            wasm_bindgen::prelude::Closure<dyn FnMut(web_sys::ErrorEvent)>,
        >,
    }

    impl Drop for BridgeInner {
        fn drop(&mut self) {
            if !self.terminated.load(Ordering::Acquire) {
                self.worker.terminate();
            }
            // Reject all pending RPC calls.
            for (_, tx) in self.pending.borrow_mut().drain() {
                let _ = tx.send(Err(BridgeError::Terminated));
            }
        }
    }

    // ── WorkerHandle ─────────────────────────────────────────

    /// A typed, thread-safe handle for communicating with a Web Worker.
    ///
    /// `Clone + Send + Sync` — safe to share across async tasks, store in
    /// framework state, or pass to event handlers.
    ///
    /// Supports both fire-and-forget ([`send`](Self::send)) and
    /// request/response ([`call`](Self::call)) patterns.
    ///
    /// When the last clone is dropped, the Worker is terminated and the
    /// associated [`EventStream`](crate::EventStream) yields `None`.
    pub struct WorkerHandle<Cmd: ActorMessage, Evt: ActorMessage> {
        pub(crate) inner: Arc<BridgeInner>,
        _phantom: PhantomData<fn(Cmd, Evt)>,
    }

    // Manual Clone: derived Clone would require Cmd: Clone + Evt: Clone
    // but we only clone Arc + PhantomData, neither of which need it.
    impl<Cmd: ActorMessage, Evt: ActorMessage> Clone for WorkerHandle<Cmd, Evt> {
        fn clone(&self) -> Self {
            Self {
                inner: Arc::clone(&self.inner),
                _phantom: PhantomData,
            }
        }
    }

    #[allow(clippy::needless_pass_by_value)] // Taking ownership of Cmd/bytes is the intended API.
    impl<Cmd: ActorMessage, Evt: ActorMessage> WorkerHandle<Cmd, Evt> {
        pub(crate) fn new(inner: Arc<BridgeInner>) -> Self {
            Self {
                inner,
                _phantom: PhantomData,
            }
        }

        /// Send the one-time init payload to the worker.
        ///
        /// Must be called exactly once, immediately after `spawn_worker`,
        /// before any `send` or `call`. Sending a second init is a logic error
        /// — it will be deserialized as a `Cmd` and likely fail.
        pub fn send_init<Init: serde::Serialize>(&self, init: &Init) -> Result<(), BridgeError> {
            if self.inner.terminated.load(Ordering::Acquire) {
                return Err(BridgeError::Terminated);
            }
            crate::transfer::post_to_worker(&self.inner.worker, None, init, None)
        }

        /// Post a command (fire-and-forget, no binary sidecar).
        pub fn send(&self, cmd: Cmd) -> Result<(), BridgeError> {
            if self.inner.terminated.load(Ordering::Acquire) {
                return Err(BridgeError::Terminated);
            }
            crate::transfer::post_to_worker(&self.inner.worker, None, &cmd, None)
        }

        /// Post a command with a binary payload (fire-and-forget).
        pub fn send_with_bytes(&self, cmd: Cmd, bytes: Vec<u8>) -> Result<(), BridgeError> {
            if self.inner.terminated.load(Ordering::Acquire) {
                return Err(BridgeError::Terminated);
            }
            crate::transfer::post_to_worker(&self.inner.worker, None, &cmd, Some(&bytes))
        }

        /// Send a command and await the response (RPC).
        ///
        /// Returns a [`CallHandle`] that:
        /// - Resolves to the worker's first `ctx.respond()` for this request.
        /// - Sends a cancellation message to the worker if dropped before
        ///   resolving — the worker's `CancellationToken` fires.
        pub fn call(&self, cmd: Cmd) -> CallHandle<Evt> {
            if self.inner.terminated.load(Ordering::Acquire) {
                return CallHandle::ready(Err(BridgeError::Terminated));
            }

            let id = self.inner.counter.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = futures_channel::oneshot::channel();
            self.inner.pending.borrow_mut().insert(id, tx);

            if let Err(e) =
                crate::transfer::post_to_worker(&self.inner.worker, Some(id), &cmd, None)
            {
                self.inner.pending.borrow_mut().remove(&id);
                return CallHandle::ready(Err(e));
            }

            CallHandle::pending(rx, id, Arc::clone(&self.inner))
        }

        /// Terminate the Worker.
        ///
        /// Subsequent `send()` / `call()` return `BridgeError::Terminated`.
        /// Pending RPCs are rejected. The `EventStream` yields `None`
        /// after pending events drain.
        pub fn terminate(&self) {
            if !self.inner.terminated.swap(true, Ordering::AcqRel) {
                self.inner.worker.terminate();
                // Reject pending RPCs immediately.
                for (_, tx) in self.inner.pending.borrow_mut().drain() {
                    let _ = tx.send(Err(BridgeError::Terminated));
                }
            }
        }

        /// Check if the Worker has been terminated.
        pub fn is_terminated(&self) -> bool {
            self.inner.terminated.load(Ordering::Acquire)
        }
    }

    // ── CallHandle ───────────────────────────────────────────

    /// Future returned by [`WorkerHandle::call`].
    ///
    /// Resolves to the worker's first response for the correlated request.
    /// **Cancels on drop**: if dropped before resolving, sends a cancellation
    /// message to the worker, firing the handler's `CancellationToken`.
    pub struct CallHandle<Evt> {
        state: CallState<Evt>,
        cancel_state: Option<CancelState>,
    }

    struct CancelState {
        correlation_id: u64,
        bridge: Arc<BridgeInner>,
    }

    enum CallState<Evt> {
        Pending(
            send_wrapper::SendWrapper<
                futures_channel::oneshot::Receiver<Result<RawResponse, BridgeError>>,
            >,
        ),
        // Boxed so CallState is Unpin regardless of Evt.
        Ready(Option<Box<Result<Evt, BridgeError>>>),
    }

    impl<Evt: DeserializeOwned> CallHandle<Evt> {
        fn pending(
            rx: futures_channel::oneshot::Receiver<Result<RawResponse, BridgeError>>,
            correlation_id: u64,
            bridge: Arc<BridgeInner>,
        ) -> Self {
            Self {
                state: CallState::Pending(send_wrapper::SendWrapper::new(rx)),
                cancel_state: Some(CancelState {
                    correlation_id,
                    bridge,
                }),
            }
        }

        fn ready(result: Result<Evt, BridgeError>) -> Self {
            Self {
                state: CallState::Ready(Some(Box::new(result))),
                cancel_state: None,
            }
        }
    }

    impl<Evt> Drop for CallHandle<Evt> {
        fn drop(&mut self) {
            if let Some(state) = self.cancel_state.take() {
                // Send cancel message to worker: [correlation_id, null, null].
                let _ = crate::transfer::post_to_worker_raw(
                    &state.bridge.worker,
                    Some(state.correlation_id),
                    &wasm_bindgen::JsValue::NULL,
                    None,
                );
                // Reject the pending RPC immediately so no stale response leaks.
                if let Some(tx) = state
                    .bridge
                    .pending
                    .borrow_mut()
                    .remove(&state.correlation_id)
                {
                    let _ = tx.send(Err(BridgeError::ChannelClosed));
                }
            }
        }
    }

    // SAFETY (logical): CallState is Unpin because:
    // - Pending: SendWrapper<Receiver<...>> is Unpin (Receiver is Unpin).
    // - Ready: Option<Box<...>> is always Unpin (Box is Unpin for any T).
    impl<Evt> Unpin for CallHandle<Evt> {}

    impl<Evt: DeserializeOwned> std::future::Future for CallHandle<Evt> {
        type Output = Result<Evt, BridgeError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            match &mut this.state {
                CallState::Ready(result) => match result.take() {
                    Some(r) => {
                        // Resolved — don't cancel on drop.
                        this.cancel_state = None;
                        Poll::Ready(*r)
                    }
                    None => Poll::Ready(Err(BridgeError::ChannelClosed)),
                },
                CallState::Pending(rx) => match Pin::new(&mut **rx).poll(cx) {
                    Poll::Ready(Ok(Ok(raw))) => {
                        // Resolved — don't cancel on drop.
                        this.cancel_state = None;
                        let evt: Evt = serde_wasm_bindgen::from_value(raw.js_value)
                            .map_err(|e| BridgeError::Serialisation(format!("deserialize: {e}")))?;
                        Poll::Ready(Ok(evt))
                    }
                    Poll::Ready(Ok(Err(e))) => {
                        this.cancel_state = None;
                        Poll::Ready(Err(e))
                    }
                    Poll::Ready(Err(_cancelled)) => {
                        this.cancel_state = None;
                        Poll::Ready(Err(BridgeError::ChannelClosed))
                    }
                    Poll::Pending => Poll::Pending,
                },
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
