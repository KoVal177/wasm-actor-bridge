//! Worker-side actor trait.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;

use crate::context::Context;

/// Trait for a worker-side actor that processes typed commands.
///
/// Implement this in your worker crate. The library provides the dispatch
/// loop (`run_actor_loop`) that deserializes incoming messages and calls
/// `handle()`.
///
/// # Type Parameters
///
/// - `Cmd`: The command type posted from Main → Worker.
/// - `Evt`: The event type posted from Worker → Main.
pub trait WorkerActor {
    /// Command type (Main → Worker).
    type Cmd: DeserializeOwned + 'static;
    /// Event type (Worker → Main).
    type Evt: Serialize + 'static;

    /// Handle a single command.
    ///
    /// Use `ctx` to send replies and access the binary payload.
    /// `ctx` is `Clone + 'static` — safe to move into spawned tasks.
    fn handle(
        &mut self,
        cmd: Self::Cmd,
        ctx: Context<Self::Evt>,
    ) -> impl Future<Output = ()> + '_;
}
