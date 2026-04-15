//! Lightweight cancellation token for WASM actors.
//!
//! Dual implementation:
//! - **WASM**: `Rc<Cell<>>` — single-threaded, `!Send`.
//! - **Native**: `Arc<AtomicBool>` — `Send + Sync` for test convenience.

// ── Native implementation (for testing) ──────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod native_impl {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{Context, Poll};

    /// A cancellation token that can be checked synchronously or awaited.
    ///
    /// `Clone + Send + Sync` on native for test ergonomics.
    #[derive(Clone)]
    pub struct CancellationToken {
        cancelled: Arc<AtomicBool>,
    }

    /// Dropping the guard cancels the associated token.
    pub struct CancelGuard {
        cancelled: Arc<AtomicBool>,
    }

    impl CancellationToken {
        /// Create a new token/guard pair.
        ///
        /// The `CancelGuard` cancels the token when dropped.
        pub fn new() -> (Self, CancelGuard) {
            let cancelled = Arc::new(AtomicBool::new(false));
            (
                Self {
                    cancelled: Arc::clone(&cancelled),
                },
                CancelGuard { cancelled },
            )
        }

        /// Check if cancellation has been requested.
        pub fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
        }

        /// Returns a future that resolves when cancellation is requested.
        pub fn cancelled(&self) -> CancelledFuture {
            CancelledFuture {
                token: self.clone(),
            }
        }
    }

    impl Drop for CancelGuard {
        fn drop(&mut self) {
            self.cancelled.store(true, Ordering::Release);
        }
    }

    /// Future that resolves when the token is cancelled.
    pub struct CancelledFuture {
        token: CancellationToken,
    }

    impl Future for CancelledFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
            if self.token.is_cancelled() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_impl::*;

// ── WASM implementation ──────────────────────────────────────

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::cell::Cell;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    /// A cancellation token that can be checked synchronously or awaited.
    ///
    /// `Clone + !Send` — designed for the single-threaded WASM event loop.
    #[derive(Clone)]
    pub struct CancellationToken {
        inner: Rc<TokenInner>,
    }

    struct TokenInner {
        cancelled: Cell<bool>,
        waker: Cell<Option<Waker>>,
    }

    /// Dropping the guard cancels the associated token.
    pub struct CancelGuard {
        inner: Rc<TokenInner>,
    }

    impl CancellationToken {
        /// Create a new token/guard pair.
        ///
        /// The `CancelGuard` cancels the token when dropped.
        pub fn new() -> (Self, CancelGuard) {
            let inner = Rc::new(TokenInner {
                cancelled: Cell::new(false),
                waker: Cell::new(None),
            });
            let token = Self {
                inner: Rc::clone(&inner),
            };
            let guard = CancelGuard { inner };
            (token, guard)
        }

        /// Check if cancellation has been requested.
        pub fn is_cancelled(&self) -> bool {
            self.inner.cancelled.get()
        }

        /// Returns a future that resolves when cancellation is requested.
        pub fn cancelled(&self) -> CancelledFuture {
            CancelledFuture {
                token: self.clone(),
            }
        }
    }

    impl Drop for CancelGuard {
        fn drop(&mut self) {
            self.inner.cancelled.set(true);
            if let Some(waker) = self.inner.waker.take() {
                waker.wake();
            }
        }
    }

    /// Future that resolves when the token is cancelled.
    pub struct CancelledFuture {
        token: CancellationToken,
    }

    impl Future for CancelledFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.token.is_cancelled() {
                Poll::Ready(())
            } else {
                self.token.inner.waker.set(Some(cx.waker().clone()));
                Poll::Pending
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
