//! `wasm-actor-bridge` — typed, zero-copy Web Worker bridge for Rust/WASM.
//!
//! # Architecture
//!
//! - Define an actor: `impl WorkerActor for MyWorker { ... }`
//! - Spawn via `SupervisorBuilder::new(url).evt_capacity(32).init(payload).build()`
//! - Send commands: `handle.send(cmd)` (fire-and-forget) or `handle.call(cmd)` (RPC with `CallHandle`)
//! - Receive events: `EventStream<Evt>` (async `Stream`)
//! - Cancel requests: drop the `CallHandle` or check `CancellationToken::is_cancelled()`
//!
//! # Worker Pool
//!
//! For parallel processing, use `WorkerPool::new(handles, RoutingStrategy::RoundRobin)`.
//! Merge event streams via `futures::stream::select_all`.
//!
//! # Main thread
//!
//! ```rust,ignore
//! let (handle, mut events) = SupervisorBuilder::<MyCmd, MyEvt, MyInit>::new("/worker.js")
//!     .evt_capacity(32)
//!     .init(my_init)
//!     .build()?;
//!
//! // Fire-and-forget.
//! handle.send(MyCmd::Ping)?;
//!
//! // RPC (request/response) — cancel-on-drop.
//! let response: MyEvt = handle.call(MyCmd::FetchPage { start: 0 }).await?;
//! ```
//!
//! # Worker thread
//!
//! ```rust,ignore
//! struct MyActor;
//!
//! impl WorkerActor for MyActor {
//!     type Init = MyInit;
//!     type Cmd = MyCmd;
//!     type Evt = MyEvt;
//!
//!     async fn handle(&mut self, cmd: MyCmd, ctx: Context<MyEvt>, token: CancellationToken) {
//!         if token.is_cancelled() { return; }
//!         ctx.respond(MyEvt::Pong);
//!     }
//! }
//!
//! #[wasm_bindgen(start)]
//! pub fn main() {
//!     run_actor_loop(MyActor);
//! }
//! ```

mod actor;
mod builder;
mod cancel;
pub mod context;
mod dispatch;
mod error;
mod handle;
mod message;
mod pool;
mod spawn;
mod stream;
pub(crate) mod transfer;

#[cfg(test)]
mod tests;

pub use actor::WorkerActor;
pub use cancel::{CancelGuard, CancellationToken};
pub use context::Context;
pub use error::BridgeError;
pub use message::{ActorMessage, WorkerEvent};

#[cfg(target_arch = "wasm32")]
pub use builder::SupervisorBuilder;
#[cfg(target_arch = "wasm32")]
pub use dispatch::run_actor_loop;
#[cfg(target_arch = "wasm32")]
pub use handle::{CallHandle, WorkerHandle};
#[cfg(target_arch = "wasm32")]
pub use pool::{RoutingStrategy, WorkerPool};
#[cfg(target_arch = "wasm32")]
pub use spawn::spawn_worker;
#[cfg(target_arch = "wasm32")]
pub use stream::EventStream;
