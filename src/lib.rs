//! `wasm-actor-bridge` — typed, zero-copy Web Worker bridge for Rust/WASM.
//!
//! Provides a generic actor-model bridge between a main-thread supervisor and
//! a Web Worker. Commands and events are typed via serde, binary payloads are
//! transferred zero-copy via `ArrayBuffer`. Supports both fire-and-forget and
//! request/response (RPC) communication patterns.
//!
//! # Main thread
//!
//! ```rust,ignore
//! let (handle, mut events) = spawn_worker::<MyCmd, MyEvt>("/worker.js")?;
//!
//! // Fire-and-forget.
//! handle.send(MyCmd::Ping)?;
//!
//! // RPC (request/response).
//! let response: MyEvt = handle.call(MyCmd::FetchPage { start: 0 }).await?;
//! ```
//!
//! # Worker thread
//!
//! ```rust,ignore
//! struct MyActor;
//!
//! impl WorkerActor for MyActor {
//!     type Cmd = MyCmd;
//!     type Evt = MyEvt;
//!
//!     async fn handle(&mut self, cmd: MyCmd, ctx: Context<MyEvt>) {
//!         ctx.reply(MyEvt::Pong);
//!     }
//! }
//!
//! #[wasm_bindgen(start)]
//! pub fn main() {
//!     run_actor_loop(MyActor);
//! }
//! ```

mod actor;
pub mod context;
mod dispatch;
mod error;
mod handle;
mod message;
mod spawn;
mod stream;
pub(crate) mod transfer;

#[cfg(test)]
mod tests;

pub use actor::WorkerActor;
pub use context::Context;
pub use error::BridgeError;
pub use message::{ActorMessage, WorkerEvent};

#[cfg(target_arch = "wasm32")]
pub use dispatch::run_actor_loop;
#[cfg(target_arch = "wasm32")]
pub use handle::{CallFuture, WorkerHandle};
#[cfg(target_arch = "wasm32")]
pub use spawn::spawn_worker;
#[cfg(target_arch = "wasm32")]
pub use stream::EventStream;
