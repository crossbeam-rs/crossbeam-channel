use std::cell::UnsafeCell;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Acquire, Release};

use futures_rs::task::{self, Task};

/// A synchronization mechanism for coordinating notifications between senders and receivers.
///
/// `SenderTask` is based heavily on `futures::AtomicTask`, but it is simpler and exposes more
/// state through its API. `SenderTask::register()` and `SenderTask::notify()` are used exactly
/// like the equivalents on `futures::AtomicTask` except for a few differences:
///
/// * `SenderTask` is a 'one-shot' notifier: once it's been notified, it can't be reset to a
/// non-notified state.
/// * `register()` and `notify()` return a `Result` which can be used by the caller to determine if
/// the `SenderTask` has already been notified.
/// * `register()` must only be called by a single thread/task (the `Sender`).
///
/// Because `SenderTask` does not support reseting back to a non-notified state, the state
/// transition diagram is simpler than `futures::AtomicTask`'s:
///
/// ```norun
///    ┌──────────┐                   ┌────────────┐
///    │          │                   │            │
///    │  Parked  │─────notify()─────▶│  Notified  │
///    │          │                   │            │
///    └──────────┘                   └────────────┘
///         ▲                                │
///         │                                │
///         │                                │
///     register()                       register()
///         │                                │
///         │                                │
///         ▼                                ▼
///   ┌──────────┐                 ┌──────────────────┐
///   │          │                 │                  │
///   │  Locked  │──────notify()──▶│  LockedNotified  │
///   │          │                 │                  │
///   └──────────┘                 └──────────────────┘
/// ```
pub struct SenderTask {
    state: AtomicUsize,
    task: UnsafeCell<Task>,
}

const NOTIFIED_BIT: usize = 1;
const LOCKED_BIT: usize = 1 << 1;

/// Initial state.
const PARKED: usize = 0;
/// Locked state, active only during a call to register().
const LOCKED: usize = LOCKED_BIT;
/// Notified state
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

                // Release the lock. If the state transitioned to `LOCKED_NOTIFIED`, this means
                // that notify() has been concurrently called, so return an error.
                if self.state.fetch_and(!LOCKED_BIT, Release) == LOCKED_NOTIFIED {
                    Err(())
                } else {
                    Ok(())
                }
            },
            NOTIFIED => Err(()),
            // LOCKED/LOCKED_NOTIFIED would indicate concurrent register() calls, which is
            // not supported by this API.
            LOCKED => unreachable!("SenderTask::register(): unexpected state: LOCKED"),
            LOCKED_NOTIFIED => unreachable!("SenderTask::register(): unexpected state: LOCKED_NOTIFIED"),
            other => unreachable!("SenderTask::register(): unexpected state: {}", other),
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
