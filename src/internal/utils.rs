//! Miscellaneous utilities.

use std::cell::Cell;
use std::num::Wrapping;
use std::sync::atomic;
use std::thread;
use std::time::Duration;

use libc;
use rand;

/// A counter that performs exponential backoff in spin loops.
pub struct Backoff(u32);

impl Backoff {
    /// Creates a new `Backoff`.
    #[inline]
    pub fn new() -> Self {
        Backoff(0)
    }

    /// Increments the counter and backs off.
    ///
    /// Returns `true` if the counter has reached a large threshold. In that case it is advisable
    /// to break the loop, do something else, and try again later.
    ///
    /// This method may yield the current processor or the current thread.
    #[inline]
    pub fn step(&mut self) -> bool {
        if self.0 <= 6 {
            for _ in 0..1 << self.0 {
                atomic::spin_loop_hint();
            }
            self.0 += 1;
            true
        } else if self.0 <= 10 {
            thread::yield_now();
            self.0 += 1;
            true
        } else {
            thread::yield_now();
            false
        }
    }
}

/// Once dropped, aborts with an error message.
///
/// This guard is used for protection from unrecoverable panics.
pub struct AbortGuard(pub &'static str);

impl Drop for AbortGuard {
    fn drop(&mut self) {
        eprintln!(
            "{}, {}:{}:{}",
            self.0,
            file!(),
            line!(),
            column!(),
        );
        unsafe { libc::abort() }
    }
}

/// Shuffles a slice randomly.
pub fn shuffle<T>(v: &mut [T]) {
    let len = v.len();
    if len <= 1 {
        return;
    }

    thread_local! {
        static RNG: Cell<Wrapping<u32>> = {
            let init = rand::random::<u32>() | 1;
            Cell::new(Wrapping(init))
        }
    }

    RNG.with(|rng| {
        for i in 1..len {
            // This is the 32-bit variant of Xorshift.
            // https://en.wikipedia.org/wiki/Xorshift
            let mut x = rng.get();
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            rng.set(x);

            let x = x.0;
            let n = i + 1;

            // This is a fast alternative to `let j = x % n`.
            // https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
            let j = ((x as u64).wrapping_mul(n as u64) >> 32) as u32 as usize;

            v.swap(i, j);
        }
    });
}

/// Blocks the current thread forever.
pub fn sleep_forever() -> ! {
    loop {
        thread::sleep(Duration::from_secs(1000));
    }
}
