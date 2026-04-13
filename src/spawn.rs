//! Spawn a Web Worker with typed communication channels.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, AtomicU64};
    use std::sync::Arc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    use crate::error::BridgeError;
    use crate::handle::{BridgeInner, RawResponse, WorkerHandle};
    use crate::message::{ActorMessage, WorkerEvent};
    use crate::stream::EventStream;
    use crate::transfer;

    /// Spawn a Web Worker and return a typed communication pair.
    ///
    /// - `script_url`: Path to the compiled worker JS bundle.
    ///
    /// Returns `(WorkerHandle<Cmd, Evt>, EventStream<Evt>)`:
    /// - `WorkerHandle` — Clone + Send + Sync handle for commands and RPC.
    /// - `EventStream` — `Stream` of uncorrelated worker events.
    ///
    /// The Worker is terminated when the last `WorkerHandle` clone is dropped
    /// (or via explicit `terminate()`), which also closes the `EventStream`.
    pub fn spawn_worker<Cmd, Evt>(
        script_url: &str,
    ) -> Result<(WorkerHandle<Cmd, Evt>, EventStream<Evt>), BridgeError>
    where
        Cmd: ActorMessage,
        Evt: ActorMessage,
    {
        let worker = web_sys::Worker::new(script_url)
            .map_err(|e| BridgeError::Spawn(format!("{e:?}")))?;

        // Event channel for fire-and-forget messages.
        let (sender, receiver) = futures_channel::mpsc::unbounded();

        // Pending RPC map — shared between closure and WorkerHandle.
        let pending: Rc<RefCell<HashMap<u64, crate::handle::PendingSender>>> =
            Rc::new(RefCell::new(HashMap::new()));

        // Wire onmessage → correlation routing.
        let msg_sender = sender.clone();
        let pending_rc = Rc::clone(&pending);
        let onmessage = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |msg: web_sys::MessageEvent| {
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
                            // Fire-and-forget event → EventStream.
                            match transfer::deserialize_payload::<Evt>(wire.payload) {
                                Ok(event) => {
                                    let _ = msg_sender.unbounded_send(Ok(WorkerEvent {
                                        payload: event,
                                        bytes: wire.bytes,
                                    }));
                                }
                                Err(e) => {
                                    let _ = msg_sender.unbounded_send(Err(e));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("wire parse error: {e}");
                        let _ = msg_sender.unbounded_send(Err(e));
                    }
                }
            },
        );
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        // Wire onerror → EventStream.
        let err_sender = sender;
        let onerror = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(
            move |err: web_sys::ErrorEvent| {
                let _ = err_sender
                    .unbounded_send(Err(BridgeError::WorkerError(err.message())));
            },
        );
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
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
