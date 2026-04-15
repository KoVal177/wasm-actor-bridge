//! Typed event stream from a Web Worker.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use futures_core::Stream;

    use crate::error::BridgeError;
    use crate::message::WorkerEvent;

    /// Stream of uncorrelated events from a Web Worker.
    ///
    /// Yields `Ok(WorkerEvent<Evt>)` for fire-and-forget events and
    /// `Err(BridgeError)` for errors. RPC responses are routed internally
    /// to [`CallHandle`](crate::CallHandle) and do not appear here.
    ///
    /// The stream ends when the [`WorkerHandle`](crate::WorkerHandle) is
    /// dropped (all clones) or explicitly terminated.
    pub struct EventStream<Evt> {
        receiver: futures_channel::mpsc::Receiver<Result<WorkerEvent<Evt>, BridgeError>>,
    }

    impl<Evt> EventStream<Evt> {
        pub(crate) fn new(
            receiver: futures_channel::mpsc::Receiver<Result<WorkerEvent<Evt>, BridgeError>>,
        ) -> Self {
            Self { receiver }
        }

        /// Close the stream. No more events will be received.
        ///
        /// The worker is **not** terminated — use
        /// [`WorkerHandle::terminate()`](crate::WorkerHandle::terminate) for that.
        pub fn close(&mut self) {
            self.receiver.close();
        }
    }

    impl<Evt> Stream for EventStream<Evt> {
        type Item = Result<WorkerEvent<Evt>, BridgeError>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Pin::new(&mut self.receiver).poll_next(cx)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.receiver.size_hint()
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
