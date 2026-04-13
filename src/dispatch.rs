//! Typed dispatch loop for the worker actor.
//!
//! Call `run_actor_loop` from your `#[wasm_bindgen(start)]` entry point.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

    use crate::actor::WorkerActor;
    use crate::context::Context;
    use crate::transfer;

    /// Start the typed dispatch loop for a [`WorkerActor`].
    ///
    /// Listens on `onmessage`, deserializes each command into `Cmd`,
    /// creates a [`Context`] for replies, and calls `actor.handle()`.
    ///
    /// # Panics
    ///
    /// Panics if the global scope is not a `DedicatedWorkerGlobalScope`.
    pub fn run_actor_loop<A>(actor: A)
    where
        A: WorkerActor + 'static,
    {
        let actor = Rc::new(std::cell::RefCell::new(actor));

        let closure = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let wire = match transfer::unpack_wire(&event) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!("invalid message: {e}");
                    return;
                }
            };

            let cmd: A::Cmd = match transfer::deserialize_payload(wire.payload) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("deserialize failed: {e}");
                    return;
                }
            };

            let ctx: Context<A::Evt> = Context::new(wire.correlation_id, wire.bytes);

            let actor = Rc::clone(&actor);
            #[allow(clippy::await_holding_refcell_ref)] // Single-threaded WASM — no contention.
            wasm_bindgen_futures::spawn_local(async move {
                let mut guard = actor.borrow_mut();
                guard.handle(cmd, ctx).await;
            });
        });

        let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
        scope.set_onmessage(Some(closure.as_ref().unchecked_ref()));
        closure.forget(); // Intentional: lives for Worker lifetime.
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
