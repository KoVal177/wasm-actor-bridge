//! Typed handle for posting commands to a Web Worker.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
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
    #[derive(Clone)]
    pub struct WorkerHandle<Cmd: ActorMessage, Evt: ActorMessage> {
        pub(crate) inner: Arc<BridgeInner>,
        _phantom: PhantomData<fn(Cmd, Evt)>,
    }

    #[allow(clippy::needless_pass_by_value)] // Taking ownership of Cmd/bytes is the intended API.
    impl<Cmd: ActorMessage, Evt: ActorMessage> WorkerHandle<Cmd, Evt> {
        pub(crate) fn new(inner: Arc<BridgeInner>) -> Self {
            Self {
                inner,
                _phantom: PhantomData,
            }
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
        /// The worker's first `ctx.reply()` for this command is routed
        /// back as the return value. Subsequent replies (if any) go to
        /// the [`EventStream`](crate::EventStream).
        pub fn call(&self, cmd: Cmd) -> CallFuture<Evt> {
            if self.inner.terminated.load(Ordering::Acquire) {
                return CallFuture::ready(Err(BridgeError::Terminated));
            }

            let id = self.inner.counter.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = futures_channel::oneshot::channel();
            self.inner.pending.borrow_mut().insert(id, tx);

            if let Err(e) =
                crate::transfer::post_to_worker(&self.inner.worker, Some(id), &cmd, None)
            {
                self.inner.pending.borrow_mut().remove(&id);
                return CallFuture::ready(Err(e));
            }

            CallFuture::pending(rx)
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

    // ── CallFuture ───────────────────────────────────────────

    /// Future returned by [`WorkerHandle::call`].
    ///
    /// Resolves to the worker's first reply for the correlated request.
    pub struct CallFuture<Evt> {
        state: CallState<Evt>,
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

    impl<Evt: DeserializeOwned> CallFuture<Evt> {
        fn pending(
            rx: futures_channel::oneshot::Receiver<Result<RawResponse, BridgeError>>,
        ) -> Self {
            Self {
                state: CallState::Pending(send_wrapper::SendWrapper::new(rx)),
            }
        }

        fn ready(result: Result<Evt, BridgeError>) -> Self {
            Self {
                state: CallState::Ready(Some(Box::new(result))),
            }
        }
    }

    // SAFETY (logical): CallState is Unpin because:
    // - Pending: SendWrapper<Receiver<...>> is Unpin (Receiver is Unpin).
    // - Ready: Option<Box<...>> is always Unpin (Box is Unpin for any T).
    impl<Evt> Unpin for CallFuture<Evt> {}

    impl<Evt: DeserializeOwned> std::future::Future for CallFuture<Evt> {
        type Output = Result<Evt, BridgeError>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            match &mut this.state {
                CallState::Ready(result) => match result.take() {
                    Some(r) => Poll::Ready(*r),
                    None => Poll::Ready(Err(BridgeError::ChannelClosed)),
                },
                CallState::Pending(rx) => match Pin::new(&mut **rx).poll(cx) {
                    Poll::Ready(Ok(Ok(raw))) => {
                        let evt: Evt = serde_wasm_bindgen::from_value(raw.js_value)
                            .map_err(|e| {
                                BridgeError::Serialisation(format!("deserialize: {e}"))
                            })?;
                        Poll::Ready(Ok(evt))
                    }
                    Poll::Ready(Ok(Err(e))) => Poll::Ready(Err(e)),
                    Poll::Ready(Err(_cancelled)) => {
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
