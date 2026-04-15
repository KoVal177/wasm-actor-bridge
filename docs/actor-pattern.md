← [Back to README](../README.md) | [Examples](../EXAMPLES.md)

# Actor Pattern & WorkerActor Trait

`wasm-actor-bridge` is built around a single trait: `WorkerActor`. This document explains
the contract, the lifecycle, and common patterns.

---

## The `WorkerActor` Trait

```rust
pub trait WorkerActor: 'static {
    type Init: for<'de> Deserialize<'de> + 'static;
    type Cmd:  for<'de> Deserialize<'de> + 'static;
    type Evt:  Serialize + 'static;

    fn init(&mut self, init: Self::Init);

    async fn handle(
        &mut self,
        cmd:   Self::Cmd,
        ctx:   Context<Self::Evt>,
        token: CancellationToken,
    );
}
```

There are only two methods:

- `init` — called once when the supervisor sends the init payload. Use it to configure
  your actor (load config, create connection handles, etc.).
- `handle` — called for every command. Always check `token.is_cancelled()` at the entry
  point of long-running handlers.

---

## Lifecycle

```
Supervisor                  Worker
    │                          │
    │ ── spawn worker ────────►│
    │ ── send Init ───────────►│ init() called
    │ ── send Cmd  ───────────►│ handle() called (async)
    │ ◄── Evt ────────────────│ ctx.respond(evt)
    │ ── drop Handle ─────────►│ terminate_worker() + channel close
```

The worker loop runs until the `WorkerHandle` is dropped (or the browser tab is closed).

---

## Responding to Commands

Every `handle` invocation receives a `Context<Evt>`. You may call `respond` zero, one, or
many times:

```rust
// No response (fire-and-forget ack):
async fn handle(&mut self, cmd: Cmd, ctx: Context<Evt>, _: CancellationToken) {
    match cmd {
        Cmd::Log(msg) => log::info!("{msg}"),   // no ctx.respond needed
        Cmd::Fetch(url) => {
            let data = fetch(url).await;
            ctx.respond(Evt::Data(data));       // single response
        }
        Cmd::StreamRows => {
            for row in self.rows.iter() {
                ctx.respond(Evt::Row(row.clone())); // multiple responses
            }
            ctx.respond(Evt::Done);
        }
    }
}
```

**Important:** For `handle.call(cmd)` RPC calls, the bridge expects **exactly one**
`ctx.respond` call. The first response resolves the `CallHandle` future; subsequent
responses are routed to the `EventStream`.

---

## Cancellation

```rust
async fn handle(&mut self, cmd: Cmd, ctx: Context<Evt>, token: CancellationToken) {
    if token.is_cancelled() { return; }   // fast path

    for chunk in large_dataset.chunks(1000) {
        if token.is_cancelled() { return; } // check inside loops
        process(chunk);
        ctx.respond(Evt::Progress(chunk.len()));
    }
}
```

The `CancellationToken` is set when the caller drops the `CallHandle`. Check it at the
start and inside any long loop to respect cooperative cancellation.

---

## Structuring a Complex Actor

For actors with many command variants, split dispatch into dedicated methods:

```rust
struct DataActor { cache: HashMap<String, Vec<u8>> }

impl WorkerActor for DataActor {
    type Init = Config;
    type Cmd  = DataCmd;
    type Evt  = DataEvt;

    fn init(&mut self, cfg: Config) { self.cache = HashMap::new(); }

    async fn handle(&mut self, cmd: DataCmd, ctx: Context<DataEvt>, token: CancellationToken) {
        match cmd {
            DataCmd::Load(key)       => self.handle_load(key, ctx, token).await,
            DataCmd::Store(key, val) => self.handle_store(key, val, ctx),
            DataCmd::Clear           => { self.cache.clear(); }
        }
    }
}

impl DataActor {
    async fn handle_load(&mut self, key: String, ctx: Context<DataEvt>, token: CancellationToken) {
        if token.is_cancelled() { return; }
        if let Some(val) = self.cache.get(&key) {
            ctx.respond(DataEvt::Loaded(val.clone()));
        } else {
            ctx.respond(DataEvt::NotFound);
        }
    }
    fn handle_store(&mut self, key: String, val: Vec<u8>, ctx: Context<DataEvt>) {
        self.cache.insert(key, val);
        ctx.respond(DataEvt::Stored);
    }
}
```
