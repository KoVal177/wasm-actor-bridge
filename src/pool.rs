//! Worker pool — routes commands across N workers.

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::CallHandle;
    use crate::error::BridgeError;
    use crate::handle::WorkerHandle;
    use crate::message::ActorMessage;

    /// Strategy for routing commands to workers.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RoutingStrategy {
        /// Round-robin across all workers. Simple, fair.
        RoundRobin,
        /// Always route to worker 0. Use for ordered commands
        /// where sequencing matters (e.g., `OpenDataset` → `FetchPage`).
        Pinned,
    }

    /// A pool of N workers with command routing.
    ///
    /// `Clone` — all handles are `Arc` internally.
    pub struct WorkerPool<Cmd: ActorMessage, Evt: ActorMessage> {
        handles: Vec<WorkerHandle<Cmd, Evt>>,
        counter: AtomicUsize,
        strategy: RoutingStrategy,
    }

    impl<Cmd: ActorMessage, Evt: ActorMessage> WorkerPool<Cmd, Evt> {
        /// Create a pool from pre-built handles.
        ///
        /// The caller spawns N workers (via [`SupervisorBuilder`](crate::SupervisorBuilder)
        /// or [`spawn_worker`](crate::spawn_worker)) and passes the handles here.
        ///
        /// # Panics
        ///
        /// Panics if `handles` is empty.
        pub fn new(handles: Vec<WorkerHandle<Cmd, Evt>>, strategy: RoutingStrategy) -> Self {
            assert!(!handles.is_empty(), "pool must have at least one worker");
            Self {
                handles,
                counter: AtomicUsize::new(0),
                strategy,
            }
        }

        /// Number of workers in the pool.
        pub fn size(&self) -> usize {
            self.handles.len()
        }

        /// Fire-and-forget command to the next worker (per routing strategy).
        pub fn send(&self, cmd: Cmd) -> Result<(), BridgeError> {
            let idx = self.pick();
            self.handles[idx].send(cmd)
        }

        /// RPC command to the next worker — returns a [`CallHandle`].
        pub fn call(&self, cmd: Cmd) -> CallHandle<Evt> {
            let idx = self.pick();
            self.handles[idx].call(cmd)
        }

        /// Fire-and-forget command to a specific worker by index.
        ///
        /// # Panics
        ///
        /// Panics if `worker_idx >= self.size()`.
        pub fn send_to(&self, worker_idx: usize, cmd: Cmd) -> Result<(), BridgeError> {
            self.handles[worker_idx].send(cmd)
        }

        /// RPC to a specific worker by index.
        ///
        /// # Panics
        ///
        /// Panics if `worker_idx >= self.size()`.
        pub fn call_on(&self, worker_idx: usize, cmd: Cmd) -> CallHandle<Evt> {
            self.handles[worker_idx].call(cmd)
        }

        /// Terminate all workers in the pool.
        pub fn terminate_all(&self) {
            for h in &self.handles {
                h.terminate();
            }
        }

        fn pick(&self) -> usize {
            match self.strategy {
                RoutingStrategy::Pinned => 0,
                RoutingStrategy::RoundRobin => {
                    let idx = self.counter.fetch_add(1, Ordering::Relaxed);
                    idx % self.handles.len()
                }
            }
        }
    }

    // Manual Clone — AtomicUsize isn't Clone, but we can snapshot its value.
    impl<Cmd: ActorMessage, Evt: ActorMessage> Clone for WorkerPool<Cmd, Evt> {
        fn clone(&self) -> Self {
            Self {
                handles: self.handles.clone(),
                counter: AtomicUsize::new(self.counter.load(Ordering::Relaxed)),
                strategy: self.strategy,
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
