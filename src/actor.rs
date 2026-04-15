//! Worker-side actor trait.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::future::Future;

use crate::cancel::CancellationToken;
use crate::context::Context;

/// Trait for a worker-side actor that processes typed commands.
///
/// # Lifecycle
///
/// 1. The worker starts and calls `run_actor_loop(MyActor::default())`
/// 2. The main thread sends an `Init` payload via `WorkerHandle::send_init()`
/// 3. The actor's `init()` method is called with the payload
/// 4. The command loop begins — all subsequent messages are `Cmd`
///
/// # Type Parameters
///
/// - `Init`: one-time initialization payload. Use `()` for no init.
/// - `Cmd`: The command type posted from Main → Worker.
/// - `Evt`: The event type posted from Worker → Main.
pub trait WorkerActor: 'static {
    /// One-time initialisation payload. The first message from the main thread
    /// is always deserialized as this type before the command loop begins.
    ///
    /// Use `()` for workers that need no initialisation.
    type Init: DeserializeOwned + 'static;

    /// Command type (Main → Worker).
    type Cmd: DeserializeOwned + 'static;
    /// Event type (Worker → Main).
    type Evt: Serialize + 'static;

    /// Called exactly once with the first message, before the command loop.
    ///
    /// Default implementation: no-op.
    fn init(&mut self, _init: Self::Init) {}

    /// Handle a single command.
    ///
    /// Use `ctx` to send responses and access the binary payload.
    /// Use `token` to check for cancellation at async yield points.
    /// `ctx` is `Clone + 'static` — safe to move into spawned tasks.
    fn handle(
        &mut self,
        cmd: Self::Cmd,
        ctx: Context<Self::Evt>,
        token: CancellationToken,
    ) -> impl Future<Output = ()> + '_;
}
