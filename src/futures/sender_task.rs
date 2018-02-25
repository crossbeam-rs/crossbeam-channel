use std::cell::UnsafeCell;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Acquire, Release};

use futures_rs::task::{self, Task};

/// A synchronization mechanism between senders and notifiers.
///
/// Based on `futures::AtomicTask`.
pub struct SenderTask {
    state: AtomicUsize,
    task: UnsafeCell<Task>,
}

/// Initial state.
const NOTIFIED_BIT: usize = 1;
const LOCKED_BIT: usize = 1 << 1;

const PARKED: usize = 0;
const LOCKED: usize = LOCKED_BIT;
const NOTIFIED: usize = NOTIFIED_BIT;
const LOCKED_NOTIFIED: usize = LOCKED_BIT | NOTIFIED_BIT;

impl SenderTask {

    /// Create a new `SenderTask` initialized with the current task.
    pub fn new() -> SenderTask {
        SenderTask {
            state: AtomicUsize::new(PARKED),
            task: UnsafeCell::new(task::current()),
        }
    }

    /// Registers the current task to be notified on calls to `notify`.
    ///
    /// Must only be called by the sender.
    ///
    /// If the sender task has already been notified, an error is returned.
    pub fn register(&self) -> Result<(), ()> {
        // Get a new task handle
        let task = task::current();

        match self.state.fetch_or(LOCKED_BIT, Acquire) {
            PARKED => unsafe {
                // Lock acquired, update the task cell.
                *self.task.get() = task;

                // Release the lock. If the state transitioned to
                // `NOTIFIED`, this means that a notify has been
                // signaled, so return an error.
                if self.state.fetch_and(!LOCKED_BIT, Release) & NOTIFIED_BIT > 0 {
                    Err(())
                } else {
                    Ok(())
                }
            },
            _ => Err(())
        }
    }

    /// Notifies the sender task.
    ///
    /// If the sender task has already been notified, an error is returned.
    pub fn notify(&self) -> Result<(), ()> {
        let state = self.state.fetch_or(NOTIFIED_BIT, Acquire);
        if state == PARKED {
            unsafe {
                (*self.task.get()).notify();
            }
        }
        if state == PARKED || state == LOCKED {
            Ok(())
        } else {
            debug_assert!(state == NOTIFIED || state == LOCKED_NOTIFIED);
            Err(())
        }
    }
}

unsafe impl Send for SenderTask {}
unsafe impl Sync for SenderTask {}
