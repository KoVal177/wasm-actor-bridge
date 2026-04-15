//! Spawn a Web Worker with typed communication channels.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64};

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    use crate::error::BridgeError;
    use crate::handle::{BridgeInner, RawResponse, WorkerHandle};
    use crate::message::{ActorMessage, WorkerEvent};
    use crate::stream::EventStream;
    use crate::transfer;

    /// Default event channel capacity when using the convenience `spawn_worker`.
    const DEFAULT_EVT_CAPACITY: usize = 16;

    /// Spawn a Web Worker and return a typed communication pair.
    ///
    /// Uses a default event channel capacity of 16. For explicit control,
    /// use [`SupervisorBuilder`](crate::SupervisorBuilder).
    pub fn spawn_worker<Cmd, Evt>(
        script_url: &str,
    ) -> Result<(WorkerHandle<Cmd, Evt>, EventStream<Evt>), BridgeError>
    where
        Cmd: ActorMessage,
        Evt: ActorMessage,
    {
        spawn_worker_with_capacity(script_url, DEFAULT_EVT_CAPACITY)
    }

    /// Spawn a Web Worker with an explicit bounded event channel capacity.
    pub(crate) fn spawn_worker_with_capacity<Cmd, Evt>(
        script_url: &str,
        evt_capacity: usize,
    ) -> Result<(WorkerHandle<Cmd, Evt>, EventStream<Evt>), BridgeError>
    where
        Cmd: ActorMessage,
        Evt: ActorMessage,
    {
        let worker =
            web_sys::Worker::new(script_url).map_err(|e| BridgeError::Spawn(format!("{e:?}")))?;

        // Bounded event channel for fire-and-forget messages.
        let (sender, receiver) = futures_channel::mpsc::channel(evt_capacity);

        // Pending RPC map — shared between closure and WorkerHandle.
        let pending: Rc<RefCell<HashMap<u64, crate::handle::PendingSender>>> =
            Rc::new(RefCell::new(HashMap::new()));

        // Wire onmessage → correlation routing.
        let mut msg_sender = sender.clone();
        let pending_rc = Rc::clone(&pending);
        let onmessage =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |msg: web_sys::MessageEvent| {
                match transfer::unpack_wire(&msg) {
                    Ok(wire) => {
                        if let Some(corr_id) = wire.correlation_id {
                            // RPC response — resolve pending call.
                            if let Some(tx) = pending_rc.borrow_mut().remove(&corr_id) {
                                let raw = RawResponse {
                                    js_value: wire.payload,
                                };
                                let _ = tx.send(Ok(raw));
                            }
                        } else {
                            // Fire-and-forget event → EventStream (bounded).
                            match transfer::deserialize_payload::<Evt>(wire.payload) {
                                Ok(event) => {
                                    if msg_sender
                                        .try_send(Ok(WorkerEvent {
                                            payload: event,
                                            bytes: wire.bytes,
                                        }))
                                        .is_err()
                                    {
                                        tracing::warn!("event channel full, dropping event");
                                    }
                                }
                                Err(e) => {
                                    if msg_sender.try_send(Err(e)).is_err() {
                                        tracing::warn!("event channel full, dropping error");
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("wire parse error: {e}");
                        if msg_sender.try_send(Err(e)).is_err() {
                            tracing::warn!("event channel full, dropping wire error");
                        }
                    }
                }
            });
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        // Wire onerror → EventStream (crash vs. script error classification).
        let mut err_sender = sender;
        let onerror =
            Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |err: web_sys::ErrorEvent| {
                let variant = classify_worker_error(&err.message());
                if err_sender.try_send(Err(variant)).is_err() {
                    tracing::warn!("event channel full, dropping worker error");
                }
            });
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        // Build shared inner state.
        let inner = Arc::new(BridgeInner {
            worker: send_wrapper::SendWrapper::new(worker),
            terminated: AtomicBool::new(false),
            counter: AtomicU64::new(0),
            pending: send_wrapper::SendWrapper::new(pending),
            _onmessage: send_wrapper::SendWrapper::new(onmessage),
            _onerror: send_wrapper::SendWrapper::new(onerror),
        });

        let handle = WorkerHandle::new(inner);
        let events = EventStream::new(receiver);

        Ok((handle, events))
    }

    /// Classify a Worker `ErrorEvent` into a crash (recoverable by respawn)
    /// or a script error (permanent).
    fn classify_worker_error(message: &str) -> BridgeError {
        const CRASH_PATTERNS: &[&str] = &[
            "unreachable",
            "runtimeerror",
            "memory access",
            "call stack",
            "out of memory",
            "recursion",
            "maximum call stack",
        ];

        if message.is_empty() {
            return BridgeError::WorkerCrashed;
        }

        let lower = message.to_lowercase();

        for pattern in CRASH_PATTERNS {
            if lower.contains(pattern) {
                return BridgeError::WorkerCrashed;
            }
        }

        BridgeError::WorkerError(message.to_owned())
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
