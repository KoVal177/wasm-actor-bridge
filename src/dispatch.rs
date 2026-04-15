//! Typed dispatch loop for the worker actor.
//!
//! Call `run_actor_loop` from your `#[wasm_bindgen(start)]` entry point.
//! Two-phase: first message → `Init`, then command loop.
//! Messages are processed sequentially via a channel — no `RefCell`, no re-entrancy risk.
//!
//! Cancellation: A wire message with a correlation ID but null/undefined payload
//! is interpreted as a cancel request. The corresponding `CancelGuard` is dropped,
//! which fires the `CancellationToken` held by the handler.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::collections::HashMap;
    use std::pin::pin;

    use futures_core::Stream;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

    use crate::actor::WorkerActor;
    use crate::cancel::{CancelGuard, CancellationToken};
    use crate::transfer;

    /// Start the two-phase dispatch loop for a [`WorkerActor`].
    ///
    /// 1. **Init phase:** Waits for exactly one message, deserializes as `Init`,
    ///    calls `actor.init(init)`.
    /// 2. **Command phase:** All subsequent messages are deserialized as `Cmd`
    ///    and processed sequentially via `actor.handle(cmd, ctx, token)`.
    ///
    /// Cancellation: A message with a correlation ID and a null payload is
    /// treated as a cancel request — the guard for that correlation is dropped,
    /// firing the token.
    ///
    /// # Panics
    ///
    /// Panics if the global scope is not a `DedicatedWorkerGlobalScope`.
    pub fn run_actor_loop<A>(actor: A)
    where
        A: WorkerActor,
    {
        let (sender, receiver) =
            futures_channel::mpsc::channel::<send_wrapper::SendWrapper<transfer::WireMessage>>(32);

        // Actor loop: init → command processing.
        wasm_bindgen_futures::spawn_local(async move {
            let mut actor = actor;
            let mut receiver = pin!(receiver);

            // Phase 1: Init — wait for exactly one message.
            let init_wire = {
                let item =
                    std::future::poll_fn(|cx| Stream::poll_next(receiver.as_mut(), cx)).await;
                if let Some(wrapped) = item {
                    send_wrapper::SendWrapper::take(wrapped)
                } else {
                    tracing::error!("actor channel closed before init");
                    return;
                }
            };

            match transfer::deserialize_payload::<A::Init>(init_wire.payload) {
                Ok(init) => actor.init(init),
                Err(e) => {
                    tracing::error!("init deserialization failed: {e}");
                    return;
                }
            }

            // Phase 2: Command loop with per-request cancellation.
            let mut cancel_guards: HashMap<u64, CancelGuard> = HashMap::new();

            while let Some(wrapped) =
                std::future::poll_fn(|cx| Stream::poll_next(receiver.as_mut(), cx)).await
            {
                let wire = send_wrapper::SendWrapper::take(wrapped);

                // Cancel message: correlation ID present, payload is null/undefined.
                if wire.payload.is_null() || wire.payload.is_undefined() {
                    if let Some(corr_id) = wire.correlation_id {
                        // Drop the guard → fires the CancellationToken.
                        cancel_guards.remove(&corr_id);
                    }
                    continue;
                }

                let cmd: A::Cmd = match transfer::deserialize_payload(wire.payload) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("command deserialization failed: {e}");
                        continue;
                    }
                };
                let ctx = crate::context::Context::new(wire.correlation_id, wire.bytes);
                let (cancel_token, cancel_guard) = CancellationToken::new();

                if let Some(corr_id) = wire.correlation_id {
                    cancel_guards.insert(corr_id, cancel_guard);
                }
                // For fire-and-forget (no corr_id): guard lives on
                // the stack until the handler returns, then drops.
                // It cannot be cancelled externally — by design.

                actor.handle(cmd, ctx, cancel_token).await;

                // Clean up guard after handler completes.
                if let Some(corr_id) = wire.correlation_id {
                    cancel_guards.remove(&corr_id);
                }
            }
            tracing::info!("actor channel closed, shutting down");
        });

        // onmessage: parse wire and enqueue.
        let mut sender = sender;
        let closure = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            match transfer::unpack_wire(&event) {
                Ok(wire) => {
                    let wrapped = send_wrapper::SendWrapper::new(wire);
                    if sender.try_send(wrapped).is_err() {
                        tracing::error!("actor channel full or closed, dropping message");
                    }
                }
                Err(e) => tracing::warn!("invalid wire message: {e}"),
            }
        });

        let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
        scope.set_onmessage(Some(closure.as_ref().unchecked_ref()));
        closure.forget(); // Intentional: lives for Worker lifetime.
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
