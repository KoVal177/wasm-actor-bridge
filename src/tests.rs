use crate::context::Context;
use crate::error::BridgeError;
use crate::message::WorkerEvent;

// ── WorkerEvent tests ────────────────────────────────────────

#[test]
fn worker_event_payload_only() {
    let evt = WorkerEvent {
        payload: 42u64,
        bytes: None,
    };
    assert_eq!(evt.payload, 42);
    assert!(evt.bytes.is_none());
}

#[test]
fn worker_event_with_bytes() {
    let data = vec![1, 2, 3];
    let evt = WorkerEvent {
        payload: "hello",
        bytes: Some(data.clone()),
    };
    assert_eq!(evt.payload, "hello");
    assert_eq!(evt.bytes.as_ref().expect("bytes present"), &data);
}

#[test]
fn worker_event_empty_bytes() {
    let evt = WorkerEvent {
        payload: 99u32,
        bytes: Some(vec![]),
    };
    assert_eq!(evt.bytes.as_ref().expect("bytes present").len(), 0);
}

#[test]
fn worker_event_clone() {
    let evt = WorkerEvent {
        payload: "msg",
        bytes: Some(vec![10, 20]),
    };
    let cloned = evt.clone();
    assert_eq!(cloned.payload, "msg");
    assert_eq!(cloned.bytes, evt.bytes);
}

#[test]
fn worker_event_debug() {
    let evt = WorkerEvent {
        payload: "test",
        bytes: None,
    };
    let debug = format!("{evt:?}");
    assert!(debug.contains("test"));
}

// ── BridgeError tests ────────────────────────────────────────

#[test]
fn error_terminated_display() {
    let e = BridgeError::Terminated;
    assert_eq!(e.to_string(), "bridge terminated");
}

#[test]
fn error_spawn_display() {
    let e = BridgeError::Spawn("CSP violation".into());
    assert!(e.to_string().contains("CSP violation"));
}

#[test]
fn error_serialisation_display() {
    let e = BridgeError::Serialisation("invalid utf8".into());
    assert!(e.to_string().contains("invalid utf8"));
}

#[test]
fn error_post_message_display() {
    let e = BridgeError::PostMessage("DataCloneError".into());
    assert!(e.to_string().contains("DataCloneError"));
}

#[test]
fn error_worker_error_display() {
    let e = BridgeError::WorkerError("script error at line 1".into());
    assert!(e.to_string().contains("script error"));
}

#[test]
fn error_channel_closed_display() {
    let e = BridgeError::ChannelClosed;
    assert_eq!(e.to_string(), "event channel closed");
}

// ── Context tests (native) ───────────────────────────────────

#[test]
fn context_bytes_none() {
    let ctx = Context::<String>::new(None);
    assert!(ctx.bytes().is_none());
}

#[test]
fn context_bytes_some() {
    let ctx = Context::<String>::new(Some(vec![1, 2, 3]));
    assert_eq!(ctx.bytes().expect("bytes"), &[1, 2, 3]);
}

#[test]
fn context_reply_collects() {
    let ctx = Context::<String>::new(None);
    let ctx2 = ctx.clone();
    ctx.reply("hello".into());
    ctx.reply("world".into());
    assert_eq!(ctx2.reply_count(), 2);
    let replies = ctx2.replies();
    assert_eq!(replies[0].0, "hello");
    assert!(replies[0].1.is_none());
    assert_eq!(replies[1].0, "world");
}

#[test]
fn context_reply_with_bytes_collects() {
    let ctx = Context::<String>::new(None);
    ctx.reply_with_bytes("data".into(), vec![42, 43]);
    let replies = ctx.replies();
    assert_eq!(replies[0].0, "data");
    assert_eq!(replies[0].1.as_ref().expect("bytes"), &[42, 43]);
}

#[test]
fn context_clone_shares_replies() {
    let ctx1 = Context::<u32>::new(None);
    let ctx2 = ctx1.clone();
    ctx1.reply(1);
    ctx2.reply(2);
    assert_eq!(ctx1.reply_count(), 2);
    assert_eq!(ctx2.reply_count(), 2);
}

// ── WorkerActor compile check ────────────────────────────────

#[test]
fn worker_actor_compiles_with_context() {
    use crate::actor::WorkerActor;

    struct TestActor;

    impl WorkerActor for TestActor {
        type Cmd = String;
        type Evt = String;

        async fn handle(&mut self, _cmd: String, ctx: Context<String>) {
            ctx.reply("reply".into());
        }
    }

    fn _assert_actor<T: WorkerActor>() {}
    _assert_actor::<TestActor>();
}

// ── Send + Sync assertions ──────────────────────────────────

fn _assert_send<T: Send>() {}
fn _assert_sync<T: Sync>() {}

#[test]
fn worker_event_is_send_sync() {
    _assert_send::<WorkerEvent<String>>();
    _assert_sync::<WorkerEvent<String>>();
}

#[test]
fn worker_event_with_complex_type_is_send_sync() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Complex {
        id: u64,
        name: String,
        tags: Vec<String>,
    }
    _assert_send::<WorkerEvent<Complex>>();
    _assert_sync::<WorkerEvent<Complex>>();
}

#[test]
fn bridge_error_is_send_sync() {
    _assert_send::<BridgeError>();
    _assert_sync::<BridgeError>();
}

// ── ActorMessage blanket impl ────────────────────────────────

#[test]
fn actor_message_blanket_impl() {
    use crate::message::ActorMessage;

    fn _accepts_actor_message<T: ActorMessage>() {}

    _accepts_actor_message::<u64>();
    _accepts_actor_message::<String>();
    _accepts_actor_message::<Vec<u8>>();
    _accepts_actor_message::<(u32, String)>();
}

// ── WorkerEvent field regression ─────────────────────────────

#[test]
fn worker_event_has_only_payload_and_bytes() {
    // Compile-time regression: if extra fields are added,
    // this struct literal will fail to compile.
    let evt = WorkerEvent {
        payload: "cmd",
        bytes: None,
    };
    assert_eq!(evt.payload, "cmd");
}
