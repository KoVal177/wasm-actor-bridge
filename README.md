# wasm-actor-bridge

[![Crates.io version](https://img.shields.io/crates/v/wasm-actor-bridge.svg)](https://crates.io/crates/wasm-actor-bridge)
[![Docs.rs](https://img.shields.io/docsrs/wasm-actor-bridge)](https://docs.rs/wasm-actor-bridge)
[![CI](https://github.com/KoVal177/wasm-actor-bridge/actions/workflows/ci.yml/badge.svg)](https://github.com/KoVal177/wasm-actor-bridge/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/rustc-1.94+-blue.svg)](https://blog.rust-lang.org/)

Typed, zero-copy Web Worker bridge for Rust/WASM actor systems.

## Overview

`wasm-actor-bridge` provides a generic actor-model bridge between a main-thread supervisor
and a Web Worker. Commands and events are typed via serde; binary payloads are transferred
zero-copy via `ArrayBuffer`. Supports both fire-and-forget and request/response (RPC)
patterns. The worker side implements a single `WorkerActor` trait; the library handles all
message serialisation, correlation, and lifecycle management.

## Features

- **`WorkerHandle<Cmd, Evt>`** — `Send + Sync + Clone` handle for commands and RPC calls
- **`CallHandle<Evt>`** — RPC future with cancel-on-drop (fires `CancellationToken`)
- **`EventStream<Evt>`** — `futures::Stream` of uncorrelated worker events
- **`SupervisorBuilder`** — validated builder for spawning workers with bounded channels
- **`WorkerPool<Cmd, Evt>`** — pool of N workers with round-robin or pinned routing
- **`CancellationToken`** — cooperative cancellation propagated to actor handlers
- **`Context<Evt>`** — `Clone + 'static` response context for worker actors
- **`WorkerActor` trait** — generic typed dispatch with associated `Init` type
- **Zero-copy binary transfer** — `send_with_bytes()` / `ctx.respond_bytes()`
- **RAII cleanup** — `Drop` terminates the worker, rejects pending RPCs, closes channels

## Installation

```toml
[dependencies]
wasm-actor-bridge = "0.1"
```

Requires the `wasm32-unknown-unknown` target for browser builds:

```bash
rustup target add wasm32-unknown-unknown
```

## Quick Start

### Main thread (supervisor)

```rust
use wasm_actor_bridge::{SupervisorBuilder, WorkerEvent};

// Spawn a single worker.
let (handle, mut events) = SupervisorBuilder::<MyCmd, MyEvt, MyInit>::new("/worker.js")
    .evt_capacity(32)
    .init(MyInit { config: "default".into() })
    .build()?;

handle.send(MyCmd::Ping)?;                        // fire-and-forget
let status: MyEvt = handle.call(MyCmd::GetStatus).await?; // RPC (cancel-on-drop)
```

### Worker thread (actor)

```rust
use wasm_actor_bridge::{WorkerActor, Context, CancellationToken, run_actor_loop};

struct MyActor;

impl WorkerActor for MyActor {
    type Init = MyInit;
    type Cmd  = MyCmd;
    type Evt  = MyEvt;

    fn init(&mut self, _init: MyInit) {}

    async fn handle(&mut self, cmd: MyCmd, ctx: Context<MyEvt>, token: CancellationToken) {
        if token.is_cancelled() { return; }
        match cmd {
            MyCmd::Ping => ctx.respond(MyEvt::Pong),
            MyCmd::GetStatus => ctx.respond(MyEvt::Status { ok: true }),
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() { run_actor_loop(MyActor); }
```

## Examples

See [`EXAMPLES.md`](EXAMPLES.md) for detailed walkthroughs.

| Example | What it shows |
|---|---|
| [Ping–Pong RPC](EXAMPLES.md#pingpong-rpc) | Single worker, typed RPC round-trip |
| [Worker Pool](EXAMPLES.md#worker-pool) | Round-robin pool with `WorkerPool` |
| [Binary Transfer](EXAMPLES.md#binary-transfer) | Zero-copy `ArrayBuffer` send/receive |
| [Native Testing](EXAMPLES.md#native-testing) | Testing actors without a browser |

## Further Reading

- [Actor Pattern & WorkerActor Trait](docs/actor-pattern.md)
- [Testing Without a Browser](docs/testing.md)

## MSRV

Minimum supported Rust version: **1.94** (edition 2024).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
