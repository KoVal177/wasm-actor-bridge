# Examples

← [Back to README](README.md)

`wasm-actor-bridge` targets the `wasm32-unknown-unknown` environment, so its examples
can't run natively as `cargo run` binaries. All actor logic is fully testable in native
Rust — see the [Native Testing](EXAMPLES.md#native-testing) section. For browser integration
you need a bundler (Trunk, wasm-pack) and a hosted HTML page.

---

## Ping–Pong RPC

Demonstrates a minimal typed RPC round-trip between the main thread and a single worker.

### What it shows

- Defining `Cmd`/`Evt`/`Init` types with serde derive
- Spawning a worker with `SupervisorBuilder`
- Fire-and-forget `send` vs awaitable `call`
- Consuming the `EventStream`

### Main-thread code

```rust
use serde::{Deserialize, Serialize};
use wasm_actor_bridge::{SupervisorBuilder, WorkerEvent};

#[derive(Serialize, Deserialize)]
enum Cmd { Ping, Echo(String) }

#[derive(Serialize, Deserialize, Debug)]
enum Evt { Pong, Echoed(String) }

#[derive(Serialize, Deserialize)]
struct Init;

// Spawn the worker (path to the compiled wasm-bindgen JS glue).
let (handle, mut events) = SupervisorBuilder::<Cmd, Evt, Init>::new("/pkg/my_worker.js")
    .init(Init)
    .build()
    .expect("failed to spawn worker");

handle.send(Cmd::Ping).expect("send failed");
let response: Evt = handle.call(Cmd::Echo("hello".into())).await.expect("rpc failed");
// response == Evt::Echoed("hello")
```

### Worker code

```rust
use wasm_actor_bridge::{WorkerActor, Context, CancellationToken, run_actor_loop};

struct PingActor;

impl WorkerActor for PingActor {
    type Init = Init;
    type Cmd  = Cmd;
    type Evt  = Evt;
    fn init(&mut self, _: Init) {}
    async fn handle(&mut self, cmd: Cmd, ctx: Context<Evt>, _: CancellationToken) {
        match cmd {
            Cmd::Ping         => ctx.respond(Evt::Pong),
            Cmd::Echo(s)      => ctx.respond(Evt::Echoed(s)),
        }
    }
}

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() { run_actor_loop(PingActor); }
```

### Key Concepts

- `SupervisorBuilder::new(url)` — the URL must point to the compiled wasm-bindgen JS glue.
- `handle.send(cmd)` — fire-and-forget; returns `Result<(), BridgeError>` immediately.
- `handle.call(cmd)` — returns a `CallHandle<Evt>` future; cancels on drop.

---

## Worker Pool

Demonstrates distributing work across multiple workers with `WorkerPool`.

### What it shows

- Creating multiple `WorkerHandle`s
- Wrapping them in a `WorkerPool` with `RoundRobin` routing
- Pinned routing with `send_to`

### Code

```rust
use wasm_actor_bridge::{WorkerPool, RoutingStrategy};

// Assume handle_a, handle_b spawned via SupervisorBuilder.
let pool = WorkerPool::new(vec![handle_a, handle_b], RoutingStrategy::RoundRobin);

pool.send(Cmd::Ping).unwrap(); // → handle_a
pool.send(Cmd::Ping).unwrap(); // → handle_b
pool.send(Cmd::Ping).unwrap(); // → handle_a (wraps around)

pool.send_to(1, Cmd::Ping).unwrap(); // → always handle_b
```

### Key Concepts

- `RoundRobin` — distributes sequentially across all workers.
- `send_to(index, cmd)` — pinned dispatch for session-affined work.
- **Note:** worker state is not shared. For stateful operations (sessions, caches) always
  use `send_to` with the same index.

---

## Binary Transfer

Demonstrates zero-copy `ArrayBuffer` transfer for large payloads.

### What it shows

- Sending raw bytes from the main thread via `send_with_bytes`
- Receiving them in the worker via `ctx.bytes()`
- Responding with bytes via `ctx.respond_bytes`

### Code

```rust
// Main thread — send a 1 MB buffer.
let data: Vec<u8> = vec![0u8; 1_024 * 1_024];
handle.send_with_bytes(Cmd::Process, data).unwrap();

// Worker — consume the transferred bytes.
async fn handle(&mut self, cmd: Cmd, ctx: Context<Evt>, _: CancellationToken) {
    match cmd {
        Cmd::Process => {
            let bytes = ctx.bytes().unwrap_or_default();
            let checksum: u32 = bytes.iter().map(|&b| u32::from(b)).sum();
            ctx.respond(Evt::Checksum(checksum));
        }
    }
}
```

### Key Concepts

- `send_with_bytes(cmd, data)` transfers the `Vec<u8>` as a JS `ArrayBuffer` — ownership
  moves to the worker thread in the browser (zero-copy).
- `ctx.bytes()` — retrieves the transferred buffer inside the actor handler.
- `ctx.respond_bytes(evt, data)` — transfer bytes back to the main thread.

---

## Native Testing

`Context` stores responses in memory on non-WASM targets. This lets you unit-test all
actor logic without a browser.

### What it shows

- Creating a `Context` directly (no worker, no browser)
- Driving the actor with arbitrary commands
- Asserting on collected responses

### Code

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wasm_actor_bridge::{Context, CancellationToken};

    #[tokio::test]
    async fn ping_responds_pong() {
        let mut actor = PingActor;
        let ctx = Context::<Evt>::new(None);
        let (token, _guard) = CancellationToken::new();

        actor.handle(Cmd::Ping, ctx.clone(), token).await;

        assert_eq!(ctx.response_count(), 1);
        let responses = ctx.take_responses();
        assert!(matches!(responses[0].payload, Evt::Pong));
    }

    #[tokio::test]
    async fn cancelled_command_does_nothing() {
        let mut actor = PingActor;
        let ctx = Context::<Evt>::new(None);
        let (token, guard) = CancellationToken::new();
        drop(guard); // cancel immediately

        actor.handle(Cmd::Ping, ctx.clone(), token).await;
        assert_eq!(ctx.response_count(), 0);
    }
}
```

### Key Concepts

- `Context::new(bytes)` — construct a test context with optional raw bytes payload.
- `ctx.response_count()` — how many `respond` / `respond_bytes` calls were made.
- `ctx.take_responses()` — consume and return all collected responses.
- No `#[wasm_bindgen_test]` needed for actor logic — keep browser tests for integration only.
