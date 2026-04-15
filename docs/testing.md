← [Back to README](../README.md) | [Examples](../EXAMPLES.md)

# Testing Without a Browser

`wasm-actor-bridge` is designed so that all actor *logic* can be tested with standard
`cargo test` — no browser, no wasm-bindgen-test harness, no headless Chrome needed.

---

## How It Works

`Context<Evt>` has two implementations:

- **WASM** — sends responses via `postMessage` back to the supervisor.
- **Native** — buffers responses in a `Vec` behind an `Arc<Mutex<...>>` that stays
  accessible from `clone()`d copies after the handler completes.

This means you can unit-test every `WorkerActor::handle` path on native Rust.

---

## Minimal Test Skeleton

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wasm_actor_bridge::{Context, CancellationToken};

    // A test helper to run one command and collect responses.
    async fn run(cmd: MyCmd, bytes: Option<Vec<u8>>) -> Vec<MyEvt> {
        let mut actor = MyActor::default();
        actor.init(MyInit::default());

        let ctx = Context::<MyEvt>::new(bytes);
        let (token, _guard) = CancellationToken::new();
        actor.handle(cmd, ctx.clone(), token).await;
        ctx.take_responses().into_iter().map(|r| r.payload).collect()
    }

    #[tokio::test]
    async fn ping_returns_pong() {
        let responses = run(MyCmd::Ping, None).await;
        assert_eq!(responses, vec![MyEvt::Pong]);
    }

    #[tokio::test]
    async fn cancelled_before_start_returns_nothing() {
        let mut actor = MyActor::default();
        actor.init(MyInit::default());

        let ctx = Context::<MyEvt>::new(None);
        let (token, guard) = CancellationToken::new();
        drop(guard); // cancel immediately, before handle runs

        actor.handle(MyCmd::Ping, ctx.clone(), token).await;
        assert_eq!(ctx.response_count(), 0);
    }
}
```

---

## Testing Binary Payloads

```rust
#[tokio::test]
async fn upload_stores_bytes() {
    let payload = b"hello world".to_vec();
    let responses = run(MyCmd::Upload { name: "test.txt".into() }, Some(payload)).await;
    assert!(matches!(responses[0], MyEvt::Uploaded { .. }));
}
```

Pass `Some(bytes)` as the second argument to `Context::new` to simulate a binary transfer.
Inside the actor, `ctx.bytes()` will return those bytes.

---

## When You Do Need a Browser

Some things cannot be tested natively:

- `ArrayBuffer` zero-copy transfer semantics
- `Worker::postMessage` timing and backpressure
- `EventStream` ordering under real browser scheduling

For those, use [`wasm-bindgen-test`](https://rustwasm.github.io/wasm-bindgen/wasm-bindgen-test/index.html)
with a headless browser target:

```rust
#[wasm_bindgen_test]
async fn worker_lifecycle_in_browser() {
    let (handle, _events) = SupervisorBuilder::<Cmd, Evt, Init>::new("/pkg/worker.js")
        .init(Init)
        .build()
        .unwrap();
    handle.send(Cmd::Ping).unwrap();
    // assert via events...
}
```

---

## CI Strategy

In CI, run the native tests on all platforms and a WASM compilation check:

```yaml
- run: cargo test                                       # native logic tests
- run: cargo build --target wasm32-unknown-unknown      # WASM compile check
```

Full browser tests are optional — they require a headless Chrome runner and add
considerable CI cost.
