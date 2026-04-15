# Changelog

All notable changes to `wasm-actor-bridge` are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

## [0.1.0] — 2026-04-15

### Added

- `WorkerHandle<Cmd, Evt>` — `Send + Sync + Clone` typed handle for commanding a Web Worker
  and receiving correlated RPC responses via an internal oneshot channel.
- `CallHandle<Evt>` — `Future` for in-flight RPC calls; cancels the pending request on drop.
- `EventStream<Evt>` — `futures::Stream` of uncorrelated events emitted by the worker.
- `SupervisorBuilder<Cmd, Evt, Init>` — type-safe builder for spawning a worker with
  configurable channel capacity and typed init payload.
- `WorkerPool<Cmd, Evt>` — pool of N workers with `RoundRobin` or `Pinned` routing strategies.
- `WorkerActor` trait — implement `init()` and `async fn handle()` to define worker behaviour.
- `Context<Evt>` — `Clone + 'static` response context passed to each `handle` invocation;
  buffers responses in memory on non-WASM targets for easy unit testing.
- `CancellationToken` — cooperative cancellation with `is_cancelled()` check; paired with a
  `CancellationGuard` that signals cancellation on drop.
- `run_actor_loop(actor)` — top-level entry point for the worker binary; handles WASM init,
  message dispatch, and actor lifecycle.
- Zero-copy binary transfer — `WorkerHandle::send_with_bytes()` and `ctx.respond_bytes()`
  transfer raw `Vec<u8>` as a transferred `ArrayBuffer` (zero-copy in the browser).
- `BridgeError` — typed error enum covering spawn, channel, serialisation, and RPC failures.
- Two-phase init: worker buffers incoming messages until `wasm_bindgen(start)` completes,
  then replays them — eliminates the post-message-before-init race condition.
