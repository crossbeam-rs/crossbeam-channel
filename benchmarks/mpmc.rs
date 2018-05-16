extern crate mpmc;
extern crate crossbeam;
pub mod testtype;
use testtype::TestType;

use mpmc::Queue;
use std::thread;

const MESSAGES: usize = 5_000_000;
const THREADS: usize = 4;

fn seq(cap: usize) {
    let q = Queue::<TestType>::with_capacity(cap);

    for i in 0..MESSAGES {
        loop {
            if q.push(TestType::new(i)).is_ok() {
                break;
            } else {
                if cfg!(feature = "yield") {
                    thread::yield_now();
                }
            }
        }
    }
    for _ in 0..MESSAGES {
        q.pop().unwrap();
    }
}

fn spsc(cap: usize) {
    let q = Queue::<TestType>::with_capacity(cap);

    crossbeam::scope(|s| {
        s.spawn(|| {
            for i in 0..MESSAGES {
                loop {
                    if q.push(TestType::new(i)).is_ok() {
                        break;
                    } else {
                        if cfg!(feature = "yield") {
                            thread::yield_now();
                        }
                    }
                }
            }
        });
        s.spawn(|| {
            for _ in 0..MESSAGES {
                loop {
                    if q.pop().is_none() {
                        if cfg!(feature = "yield") {
                            thread::yield_now();
                        }
                    } else {
                        break;
                    }
                }
            }
        });
    });
}

fn mpsc(cap: usize) {
    let q = Queue::<TestType>::with_capacity(cap);

    crossbeam::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                for i in 0..MESSAGES / THREADS {
                    loop {
                        if q.push(TestType::new(i)).is_ok() {
                            break;
                        } else {
                            if cfg!(feature = "yield") {
                                thread::yield_now();
                            }
                        }
                    }
                }
            });
        }
        s.spawn(|| {
            for _ in 0..MESSAGES {
                loop {
                    if q.pop().is_none() {
                        if cfg!(feature = "yield") {
                            thread::yield_now();
                        }
                    } else {
                        break;
                    }
                }
            }
        });
    });
}

fn mpmc(cap: usize) {
    let q = Queue::<TestType>::with_capacity(cap);

    crossbeam::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                for i in 0..MESSAGES / THREADS {
                    loop {
                        if q.push(TestType::new(i)).is_ok() {
                            break;
                        } else {
                            if cfg!(feature = "yield") {
                                thread::yield_now();
                            }
                        }
                    }
                }
            });
        }
        for _ in 0..THREADS {
            s.spawn(|| {
                for _ in 0..MESSAGES / THREADS {
                    loop {
                        if q.pop().is_none() {
                            if cfg!(feature = "yield") {
                                thread::yield_now();
                            }
                        } else {
                            break;
                        }
                    }
                }
            });
        }
    });
}

fn main() {
    macro_rules! run {
        ($name:expr, $f:expr) => {
            let now = ::std::time::Instant::now();
            $f;
            let elapsed = now.elapsed();
            println!(
                "{:25} {:15} {:7.3} sec",
                $name,
                "Rust mpmc",
                elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1e9
            );
        }
    }

    run!("bounded_mpmc", mpmc(MESSAGES));
    run!("bounded_mpsc", mpsc(MESSAGES));
    run!("bounded_seq", seq(MESSAGES));
    run!("bounded_spsc", spsc(MESSAGES));
}
