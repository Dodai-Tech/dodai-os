//! The Hibari OS microkernel (first stage): a static, cooperative scheduler with message queues
//! and a capability table.
//!
//! It only does what a microkernel must: tasks, messages between them and the rule of who may
//! use which capability. Drivers, networking and the UI are services that talk by message. The
//! crate is `no_std`, allocates nothing and forbids `unsafe`; architecture-specific code (timer,
//! interrupts, context switch for preemption) will live in separate port crates behind the
//! [`Port`] trait.
#![cfg_attr(not(test), no_std)]

mod kernel;
mod queue;

pub use kernel::{Error, Kernel, Message, TaskId};
pub use queue::Queue;

/// What the kernel needs from an architecture or backend.
pub trait Port {
    /// Milliseconds since boot; wraps after about 49 days.
    fn now_ms(&self) -> u32;

    /// Waits for the next interrupt (low-power idle) when no task is ready.
    fn idle(&self);
}
