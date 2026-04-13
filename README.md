# wasm-actor-bridge

Typed, zero-copy Web Worker bridge for Rust/WASM actor systems.

Provides a generic actor-model bridge between a main-thread supervisor and
a Web Worker. Commands and events are typed via serde, binary payloads
are transferred zero-copy via `ArrayBuffer`. Supports both fire-and-forget
and request/response (RPC) communication.

## Features

- `WorkerHandle<Cmd, Evt>` — Send + Sync + Clone handle for commands and RPC
- `EventStream<Evt>` — `futures::Stream` of uncorrelated worker events
- `handle.call(cmd).await` — request/response RPC with automatic correlation
- `Context<Evt>` — Clone + 'static reply context for worker actors
- `WorkerActor` trait — generic typed dispatch, raw `Cmd` (no wrapper)
- Zero-copy binary transfer — explicit `send_with_bytes()` / `ctx.reply_with_bytes()`
- RAII cleanup — Drop terminates worker, rejects pending RPCs, closes channels

## Usage

### Main Thread

```rust
use wasm_actor_bridge::{spawn_worker, WorkerEvent};

let (handle, mut events) = spawn_worker::<MyCmd, MyEvt>("/worker.js")?;

// Fire-and-forget.
handle.send(MyCmd::Ping)?;

// Fire-and-forget with binary payload.
handle.send_with_bytes(MyCmd::Upload { name: "data".into() }, raw_bytes)?;

// RPC (request/response) — awaits the worker's first ctx.reply().
let status: MyEvt = handle.call(MyCmd::GetStatus).await?;

// Consume uncorrelated events as a Stream.
wasm_bindgen_futures::spawn_local(async move {
    use futures_core::Stream;
    while let Some(result) = pin!(events).next().await {
        match result {
            Ok(WorkerEvent { payload, bytes }) => { /* typed event */ },
            Err(err) => { /* bridge error */ },
        }
    }
});
```

### Worker Thread

```rust
use wasm_actor_bridge::{WorkerActor, Context, run_actor_loop};

struct MyActor;

impl WorkerActor for MyActor {
    type Cmd = MyCmd;
    type Evt = MyEvt;

    async fn handle(&mut self, cmd: MyCmd, ctx: Context<MyEvt>) {
        match cmd {
            MyCmd::Ping => ctx.reply(MyEvt::Pong),
            MyCmd::GetStatus => ctx.reply(MyEvt::Status { ok: true }),
            MyCmd::Upload { name } => {
                let data = ctx.bytes().unwrap_or_default();
                // process data...
                ctx.reply(MyEvt::Uploaded { name });
            }
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    run_actor_loop(MyActor);
}
```

### Testing (native, no browser)

`Context` collects replies in memory on non-WASM targets:

```rust
let ctx = Context::<MyEvt>::new(Some(raw_bytes));
let ctx2 = ctx.clone();
actor.handle(MyCmd::Upload { name: "f".into() }, ctx).await;
assert_eq!(ctx2.reply_count(), 1);
```

## Dependencies

Only bedrock Rust/WASM ecosystem crates:

- `serde` + `serde-wasm-bindgen` — typed serialization
- `futures-channel` + `futures-core` — async channels, oneshot, Stream trait
- `web-sys` / `js-sys` / `wasm-bindgen` — official browser bindings
- `send_wrapper` — Send+Sync for single-threaded WASM
- `thiserror` — typed errors
- `tracing` — structured logging

## License

Licensed under either of Apache License 2.0 or MIT license at your option.
