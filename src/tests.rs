use crate::cancel::CancellationToken;
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
fn error_worker_crashed_display() {
    let e = BridgeError::WorkerCrashed;
    assert_eq!(e.to_string(), "worker crashed");
}

#[test]
fn error_is_crash() {
    assert!(BridgeError::WorkerCrashed.is_crash());
    assert!(!BridgeError::WorkerCrashed.is_permanent());
}

#[test]
fn error_is_permanent() {
    assert!(BridgeError::Spawn("x".into()).is_permanent());
    assert!(!BridgeError::Spawn("x".into()).is_crash());
    assert!(BridgeError::WorkerError("x".into()).is_permanent());
    assert!(!BridgeError::WorkerError("x".into()).is_crash());
}

#[test]
fn error_neither_crash_nor_permanent() {
    assert!(!BridgeError::Terminated.is_crash());
    assert!(!BridgeError::Terminated.is_permanent());
    assert!(!BridgeError::ChannelClosed.is_crash());
    assert!(!BridgeError::ChannelClosed.is_permanent());
    assert!(!BridgeError::Serialisation("x".into()).is_crash());
    assert!(!BridgeError::Serialisation("x".into()).is_permanent());
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
fn context_respond_collects() {
    let ctx = Context::<String>::new(None);
    let ctx2 = ctx.clone();
    ctx.respond("hello".into());
    ctx.respond("world".into());
    assert_eq!(ctx2.response_count(), 2);
    let responses = ctx2.responses();
    assert_eq!(responses[0].0, "hello");
    assert!(responses[0].1.is_none());
    assert_eq!(responses[1].0, "world");
}

#[test]
fn context_respond_bytes_collects() {
    let ctx = Context::<String>::new(None);
    ctx.respond_bytes("data".into(), vec![42, 43]);
    let responses = ctx.responses();
    assert_eq!(responses[0].0, "data");
    assert_eq!(responses[0].1.as_ref().expect("bytes"), &[42, 43]);
}

#[test]
fn context_clone_shares_responses() {
    let ctx1 = Context::<u32>::new(None);
    let ctx2 = ctx1.clone();
    ctx1.respond(1);
    ctx2.respond(2);
    assert_eq!(ctx1.response_count(), 2);
    assert_eq!(ctx2.response_count(), 2);
}

// ── WorkerActor compile check ────────────────────────────────

#[test]
fn worker_actor_compiles_with_context() {
    use crate::actor::WorkerActor;

    struct TestActor;

    impl WorkerActor for TestActor {
        type Init = String;
        type Cmd = String;
        type Evt = String;

        async fn handle(&mut self, _cmd: String, ctx: Context<String>, _token: CancellationToken) {
            ctx.respond("reply".into());
        }
    }

    fn assert_actor<T: WorkerActor>() {}
    assert_actor::<TestActor>();
}

#[test]
fn worker_actor_no_init_default_is_noop() {
    use crate::actor::WorkerActor;

    struct NoInitActor;

    impl WorkerActor for NoInitActor {
        type Init = ();
        type Cmd = u32;
        type Evt = u32;

        async fn handle(&mut self, _cmd: u32, _ctx: Context<u32>, _token: CancellationToken) {}
    }

    fn assert_actor<T: WorkerActor>() {}

    // The default `init` is a no-op — this just verifies it compiles
    // and can be called without side effects.
    let mut actor = NoInitActor;
    actor.init(());
    assert_actor::<NoInitActor>();
}

// ── Send + Sync assertions ──────────────────────────────────

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

#[test]
fn worker_event_is_send_sync() {
    assert_send::<WorkerEvent<String>>();
    assert_sync::<WorkerEvent<String>>();
}

#[test]
fn worker_event_with_complex_type_is_send_sync() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Complex {
        id: u64,
        name: String,
        tags: Vec<String>,
    }
    assert_send::<WorkerEvent<Complex>>();
    assert_sync::<WorkerEvent<Complex>>();
}

#[test]
fn bridge_error_is_send_sync() {
    assert_send::<BridgeError>();
    assert_sync::<BridgeError>();
}

// ── ActorMessage blanket impl ────────────────────────────────

#[test]
fn actor_message_blanket_impl() {
    use crate::message::ActorMessage;

    fn accepts_actor_message<T: ActorMessage>() {}

    accepts_actor_message::<u64>();
    accepts_actor_message::<String>();
    accepts_actor_message::<Vec<u8>>();
    accepts_actor_message::<(u32, String)>();
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

// ── CancellationToken tests ─────────────────────────────────

#[test]
fn cancel_token_starts_not_cancelled() {
    let (token, _guard) = CancellationToken::new();
    assert!(!token.is_cancelled());
}

#[test]
fn cancel_token_fires_on_guard_drop() {
    let (token, guard) = CancellationToken::new();
    assert!(!token.is_cancelled());
    drop(guard);
    assert!(token.is_cancelled());
}

#[test]
fn cancel_token_clone_sees_cancellation() {
    let (token, guard) = CancellationToken::new();
    let token2 = token.clone();
    drop(guard);
    assert!(token.is_cancelled());
    assert!(token2.is_cancelled());
}

#[test]
fn cancel_guard_double_drop_is_safe() {
    let (token, guard) = CancellationToken::new();
    drop(guard);
    assert!(token.is_cancelled());
    // token is still usable after guard dropped
    assert!(token.is_cancelled());
}

#[test]
fn cancel_token_multiple_clones_all_see_cancellation() {
    let (token, guard) = CancellationToken::new();
    let t2 = token.clone();
    let t3 = token.clone();
    assert!(!t2.is_cancelled());
    assert!(!t3.is_cancelled());
    drop(guard);
    assert!(token.is_cancelled());
    assert!(t2.is_cancelled());
    assert!(t3.is_cancelled());
}

#[test]
fn worker_actor_with_cancel_token_compiles() {
    use crate::actor::WorkerActor;

    struct CancelAwareActor;

    impl WorkerActor for CancelAwareActor {
        type Init = ();
        type Cmd = String;
        type Evt = String;

        async fn handle(&mut self, _cmd: String, ctx: Context<String>, token: CancellationToken) {
            if token.is_cancelled() {
                return;
            }
            ctx.respond("done".into());
        }
    }

    fn assert_actor<T: WorkerActor>() {}
    assert_actor::<CancelAwareActor>();
}

// ── Error variant tests ─────────────────────────────────────

#[test]
fn channel_full_error_display() {
    let err = BridgeError::ChannelFull;
    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("channel"), "got: {msg}");
    assert!(!err.is_permanent());
}

#[test]
fn invalid_config_error_display() {
    let err = BridgeError::InvalidConfig("evt_capacity must be > 0".into());
    let msg = err.to_string();
    assert!(msg.contains("evt_capacity"), "got: {msg}");
    assert!(err.is_permanent());
}

#[test]
fn channel_full_is_not_permanent() {
    assert!(!BridgeError::ChannelFull.is_permanent());
}

#[test]
fn invalid_config_is_permanent() {
    assert!(BridgeError::InvalidConfig("bad".into()).is_permanent());
}
