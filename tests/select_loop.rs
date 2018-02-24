extern crate crossbeam;
#[macro_use]
extern crate crossbeam_channel;

use std::any::Any;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use crossbeam_channel::{RecvError, RecvTimeoutError, TryRecvError};
use crossbeam_channel::{SendError, SendTimeoutError, TrySendError};

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
#[allow(dead_code, unused_mut)]
fn it_compiles() {
    struct Foo(String);

    fn foo(
        mut struct_val: Foo,
        mut var: String,
        immutable_var: String,
        rx0: Receiver<String>,
        rx1: &Receiver<u32>,
        rx2: Receiver<()>,
        tx0: &mut Sender<String>,
        tx1: Sender<String>,
        tx2: Sender<String>,
        tx3: Sender<String>,
        tx4: Sender<String>,
        tx5: Sender<u32>,
    ) -> Option<String> {
        select_loop! {
            recv(rx0, val) => Some(val),
            recv(rx1, val) => Some(val.to_string()),
            recv(rx2, ()) => None,
            send(tx0, mut struct_val.0) => Some(var),
            send(tx1, mut var) => Some(struct_val.0),
            send(tx1, immutable_var) => Some(struct_val.0),
            send(tx2, struct_val.0.clone()) => Some(struct_val.0),
            send(tx3, "foo".to_string()) => Some(var),
            send(tx4, var.clone()) => Some(var),
            send(tx5, 42) => None,
            
            closed() => Some("closed".into()),
            would_block() => Some("would_block".into()),
            timed_out(Duration::from_secs(1)) => Some("timed_out".into()),
            // The previous timeout duration is overridden.
            timed_out(Duration::from_secs(2)) => Some("timed_out".into()),
        }
    }
}

#[test]
fn smoke1() {
    let (tx1, rx1) = unbounded();
    let (tx2, rx2) = unbounded();

    tx1.send(1).unwrap();

    select_loop! {
        recv(rx1, v) => assert_eq!(v, 1),
        recv(rx2, _) => panic!(),
    }

    tx2.send(2).unwrap();

    select_loop! {
        recv(rx1, _) => panic!(),
        recv(rx2, v) => assert_eq!(v, 2),
    }
}

#[test]
fn smoke2() {
    let (_tx1, rx1) = unbounded::<i32>();
    let (_tx2, rx2) = unbounded::<i32>();
    let (_tx3, rx3) = unbounded::<i32>();
    let (_tx4, rx4) = unbounded::<i32>();
    let (tx5, rx5) = unbounded::<i32>();

    tx5.send(5).unwrap();

    select_loop! {
        recv(rx1, _) => panic!(),
        recv(rx2, _) => panic!(),
        recv(rx3, _) => panic!(),
        recv(rx4, _) => panic!(),
        recv(rx5, v) => assert_eq!(v, 5),
    }
}

#[test]
fn closed() {
    let (tx1, rx1) = unbounded::<i32>();
    let (tx2, rx2) = unbounded::<i32>();

    crossbeam::scope(|s| {
        s.spawn(|| {
            thread::sleep(ms(500));
            drop(tx1);
        });

        select_loop! {
            recv(rx1, _) => panic!(),
            recv(rx2, _) => panic!(),
            closed() => panic!(),
            timed_out(ms(1000)) => {},
        }
    });

    crossbeam::scope(|s| {
        s.spawn(|| {
            thread::sleep(ms(500));
            drop(tx2);
        });

        select_loop! {
            recv(rx1, _) => panic!(),
            recv(rx2, _) => panic!(),
            closed() => {},
            timed_out(ms(1000)) => panic!(),
        }
    });
}

#[test]
fn would_block() {
    let (tx1, rx1) = unbounded::<i32>();
    let (tx2, rx2) = unbounded::<i32>();

    drop(tx1);

    select_loop! {
        recv(rx1, _) => panic!(),
        recv(rx2, _) => panic!(),
        closed() => panic!(),
        would_block() => {},
        timed_out(ms(0)) => panic!(),
    }

    tx2.send(2).unwrap();

    select_loop! {
        recv(rx1, _) => panic!(),
        recv(rx2, v) => assert_eq!(v, 2),
        closed() => panic!(),
        would_block() => panic!(),
        timed_out(ms(0)) => panic!(),
    }

    drop(tx2);

    select_loop! {
        recv(rx1, _) => panic!(),
        recv(rx2, _) => panic!(),
        closed() => {},
        would_block() => panic!(),
        timed_out(ms(0)) => panic!(),
    }
}

#[test]
fn timeout() {
    let (_tx1, rx1) = unbounded::<i32>();
    let (tx2, rx2) = unbounded::<i32>();

    crossbeam::scope(|s| {
        s.spawn(|| {
            thread::sleep(ms(1500));
            tx2.send(2).unwrap();
        });

        select_loop! {
            recv(rx1, _) => panic!(),
            recv(rx2, _) => panic!(),
            timed_out(ms(1000)) => {},
        }

        select_loop! {
            recv(rx1, _) => panic!(),
            recv(rx2, v) => assert_eq!(v, 2),
            timed_out(ms(1000)) => panic!(),
        }
    });
}

#[test]
fn timeout_when_closed() {
    let (_, rx) = unbounded::<i32>();

    select_loop! {
        recv(rx, _) => {}
        timed_out(ms(1000)) => {}
    }
}

#[test]
fn would_block_when_closed() {
    let (_, rx) = unbounded::<i32>();

    select_loop! {
        recv(rx, _) => {}
        would_block() => {}
    }
}

#[test]
fn unblocks() {
    let (tx1, rx1) = bounded(0);
    let (tx2, rx2) = bounded(0);

    crossbeam::scope(|s| {
        s.spawn(|| {
            thread::sleep(ms(500));
            tx2.send(2).unwrap();
        });

        select_loop! {
            recv(rx1, _) => panic!(),
            recv(rx2, v) => assert_eq!(v, 2),
            timed_out(ms(1000)) => panic!(),
        }
    });

    crossbeam::scope(|s| {
        s.spawn(|| {
            thread::sleep(ms(500));
            assert_eq!(rx1.recv().unwrap(), 1);
        });

        select_loop! {
            send(tx1, 1) => {},
            send(tx2, 2) => panic!(),
            timed_out(ms(1000)) => panic!(),
        }
    });
}

#[test]
fn both_ready() {
    let (tx1, rx1) = bounded(0);
    let (tx2, rx2) = bounded(0);

    crossbeam::scope(|s| {
        s.spawn(|| {
            thread::sleep(ms(500));
            tx1.send(1).unwrap();
            assert_eq!(rx2.recv().unwrap(), 2);
        });

        for _ in 0..2 {
            select_loop! {
                recv(rx1, v) => assert_eq!(v, 1),
                send(tx2, 2) => {},
            }
        }
    });
}

#[test]
fn loop_try() {
    for _ in 0..20 {
        let (tx1, rx1) = bounded::<i32>(0);
        let (tx2, rx2) = bounded::<i32>(0);

        crossbeam::scope(|s| {
            s.spawn(|| {
                loop {
                    match tx1.try_send(1) {
                        Ok(()) => break,
                        Err(TrySendError::Closed(_)) => break,
                        Err(TrySendError::Full(_)) => continue,
                    }
                }
            });

            s.spawn(|| {
                loop {
                    match rx2.try_recv() {
                        Ok(x) => {
                            assert_eq!(x, 2);
                            break;
                        }
                        Err(TryRecvError::Closed) => break,
                        Err(TryRecvError::Empty) => continue,
                    }
                }
            });

            s.spawn(|| {
                thread::sleep(ms(500));

                select_loop! {
                    recv(rx1, v) => assert_eq!(v, 1),
                    send(tx2, 2) => {},
                    closed() => panic!(),
                    timed_out(ms(500)) => panic!(),
                }

                drop(rx1);
                drop(tx2);
            });
        });
    }
}

#[test]
fn cloning1() {
    crossbeam::scope(|s| {
        let (tx1, rx1) = unbounded::<i32>();
        let (_tx2, rx2) = unbounded::<i32>();
        let (tx3, rx3) = unbounded::<()>();

        s.spawn(move || {
            rx3.recv().unwrap();
            tx1.clone();
            assert_eq!(rx3.try_recv(), Err(TryRecvError::Empty));
            tx1.send(1).unwrap();
            rx3.recv().unwrap();
        });

        tx3.send(()).unwrap();

        select_loop! {
            recv(rx1, _) => {},
            recv(rx2, _) => {},
        }

        tx3.send(()).unwrap();
    });
}

#[test]
fn cloning2() {
    crossbeam::scope(|s| {
        let (tx1, rx1) = unbounded::<()>();
        let (tx2, rx2) = unbounded::<()>();
        let (_tx3, _rx3) = unbounded::<()>();

        s.spawn(move || {
            select_loop! {
                recv(rx1, _) => panic!(),
                recv(rx2, _) => {},
            }
        });

        thread::sleep(ms(500));
        drop(tx1.clone());
        tx2.send(()).unwrap();
    })
}

#[test]
fn preflight1() {
    let (tx, rx) = unbounded();
    tx.send(()).unwrap();

    select_loop! {
        recv(rx, _) => {}
    }
}

#[test]
fn preflight2() {
    let (tx, rx) = unbounded();
    drop(tx.clone());
    tx.send(()).unwrap();
    drop(tx);

    select_loop! {
        recv(rx, _) => {}
    }
    assert_eq!(rx.try_recv(), Err(TryRecvError::Closed));
}

#[test]
fn preflight3() {
    let (tx, rx) = unbounded();
    drop(tx.clone());
    tx.send(()).unwrap();
    drop(tx);
    rx.recv().unwrap();

    select_loop! {
        recv(rx, _) => panic!(),
        closed() => {},
    }
}

#[test]
fn stress_recv() {
    let (tx1, rx1) = unbounded();
    let (tx2, rx2) = bounded(5);
    let (tx3, rx3) = bounded(100);

    crossbeam::scope(|s| {
        s.spawn(|| {
            for i in 0..10_000 {
                tx1.send(i).unwrap();
                rx3.recv().unwrap();

                tx2.send(i).unwrap();
                rx3.recv().unwrap();
            }
        });

        for i in 0..10_000 {
            for _ in 0..2 {
                select_loop! {
                    recv(rx1, v) => assert_eq!(v, i),
                    recv(rx2, v) => assert_eq!(v, i),
                }

                tx3.send(()).unwrap();
            }
        }
    });
}

#[test]
fn stress_send() {
    let (tx1, rx1) = bounded(0);
    let (tx2, rx2) = bounded(0);
    let (tx3, rx3) = bounded(100);

    crossbeam::scope(|s| {
        s.spawn(|| {
            for i in 0..10_000 {
                assert_eq!(rx1.recv().unwrap(), i);
                assert_eq!(rx2.recv().unwrap(), i);
                rx3.recv().unwrap();
            }
        });

        for i in 0..10_000 {
            for _ in 0..2 {
                select_loop! {
                    send(tx1, i) => {},
                    send(tx2, i) => {},
                }
            }
            tx3.send(()).unwrap();
        }
    });
}

#[test]
fn stress_mixed() {
    let (tx1, rx1) = bounded(0);
    let (tx2, rx2) = bounded(0);
    let (tx3, rx3) = bounded(100);

    crossbeam::scope(|s| {
        s.spawn(|| {
            for i in 0..10_000 {
                tx1.send(i).unwrap();
                assert_eq!(rx2.recv().unwrap(), i);
                rx3.recv().unwrap();
            }
        });

        for i in 0..10_000 {
            for _ in 0..2 {
                select_loop! {
                    recv(rx1, v) => assert_eq!(v, i),
                    send(tx2, i) => {},
                }
            }
            tx3.send(()).unwrap();
        }
    });
}

#[test]
fn stress_timeout_two_threads() {
    const COUNT: usize = 20;

    let (tx, rx) = bounded(2);

    crossbeam::scope(|s| {
        s.spawn(|| {
            for i in 0..COUNT {
                if i % 2 == 0 {
                    thread::sleep(ms(500));
                }

                'outer: loop {
                    select_loop! {
                        send(tx, i) => break 'outer,
                        timed_out(ms(100)) => {}
                    }
                }
            }
        });

        s.spawn(|| {
            for i in 0..COUNT {
                if i % 2 == 0 {
                    thread::sleep(ms(500));
                }

                'outer: loop {
                    select_loop! {
                        recv(rx, v) => {
                            assert_eq!(v, i);
                            break 'outer;
                        }
                        timed_out(ms(100)) => {}
                    }
                }
            }
        });
    });
}

struct WrappedSender<T>(Sender<T>);

impl<T> WrappedSender<T> {
    pub fn try_send(&self, mut value: T) -> Result<(), TrySendError<T>> {
        select_loop! {
            send(self.0, mut value) => return Ok(()),
            closed() => return Err(TrySendError::Closed(value)),
            would_block() => return Err(TrySendError::Full(value)),
        }
    }

    pub fn send(&self, mut value: T) -> Result<(), SendError<T>> {
        select_loop! {
            send(self.0, mut value) => return Ok(()),
            closed() => return Err(SendError(value)),
        }
    }

    pub fn send_timeout(&self, mut value: T, dur: Duration) -> Result<(), SendTimeoutError<T>> {
        select_loop! {
            send(self.0, mut value) => return Ok(()),
            closed() => return Err(SendTimeoutError::Closed(value)),
            timed_out(dur) => return Err(SendTimeoutError::Timeout(value)),
        }
    }
}

struct WrappedReceiver<T>(Receiver<T>);

impl<T> WrappedReceiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        select_loop! {
            recv(self.0, v) => return Ok(v),
            closed() => return Err(TryRecvError::Closed),
            would_block() => return Err(TryRecvError::Empty),
        }
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        select_loop! {
            recv(self.0, v) => return Ok(v),
            closed() => return Err(RecvError),
        }
    }

    pub fn recv_timeout(&self, dur: Duration) -> Result<T, RecvTimeoutError> {
        select_loop! {
            recv(self.0, v) => return Ok(v),
            closed() => return Err(RecvTimeoutError::Closed),
            timed_out(dur) => return Err(RecvTimeoutError::Timeout),
        }
    }
}

#[test]
fn recv() {
    let (tx, rx) = bounded(100);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(rx.recv(), Ok(7));
            thread::sleep(ms(1000));
            assert_eq!(rx.recv(), Ok(8));
            thread::sleep(ms(1000));
            assert_eq!(rx.recv(), Ok(9));
            assert_eq!(rx.recv(), Err(RecvError));
        });
        s.spawn(move || {
            thread::sleep(ms(1500));
            assert_eq!(tx.send(7), Ok(()));
            assert_eq!(tx.send(8), Ok(()));
            assert_eq!(tx.send(9), Ok(()));
        });
    });

    let (tx, rx) = bounded(0);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(rx.recv(), Ok(7));
            thread::sleep(ms(1000));
            assert_eq!(rx.recv(), Ok(8));
            thread::sleep(ms(1000));
            assert_eq!(rx.recv(), Ok(9));
            assert_eq!(rx.recv(), Err(RecvError));
        });
        s.spawn(move || {
            thread::sleep(ms(1500));
            assert_eq!(tx.send(7), Ok(()));
            assert_eq!(tx.send(8), Ok(()));
            assert_eq!(tx.send(9), Ok(()));
        });
    });
}

#[test]
fn recv_timeout() {
    let (tx, rx) = bounded(100);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(rx.recv_timeout(ms(1000)), Err(RecvTimeoutError::Timeout));
            assert_eq!(rx.recv_timeout(ms(1000)), Ok(7));
            assert_eq!(
                rx.recv_timeout(ms(1000)),
                Err(RecvTimeoutError::Closed)
            );
        });
        s.spawn(move || {
            thread::sleep(ms(1500));
            assert_eq!(tx.send(7), Ok(()));
        });
    });
}

#[test]
fn try_recv() {
    let (tx, rx) = bounded(100);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
            thread::sleep(ms(1500));
            assert_eq!(rx.try_recv(), Ok(7));
            thread::sleep(ms(500));
            assert_eq!(rx.try_recv(), Err(TryRecvError::Closed));
        });
        s.spawn(move || {
            thread::sleep(ms(1000));
            assert_eq!(tx.send(7), Ok(()));
        });
    });

    let (tx, rx) = bounded(0);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(rx.recv_timeout(ms(1000)), Err(RecvTimeoutError::Timeout));
            assert_eq!(rx.recv_timeout(ms(1000)), Ok(7));
            assert_eq!(
                rx.recv_timeout(ms(1000)),
                Err(RecvTimeoutError::Closed)
            );
        });
        s.spawn(move || {
            thread::sleep(ms(1500));
            assert_eq!(tx.send(7), Ok(()));
        });
    });
}

#[test]
fn send() {
    let (tx, rx) = bounded(1);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(tx.send(7), Ok(()));
            thread::sleep(ms(1000));
            assert_eq!(tx.send(8), Ok(()));
            thread::sleep(ms(1000));
            assert_eq!(tx.send(9), Ok(()));
            thread::sleep(ms(1000));
            assert_eq!(tx.send(10), Err(SendError(10)));
        });
        s.spawn(move || {
            thread::sleep(ms(1500));
            assert_eq!(rx.recv(), Ok(7));
            assert_eq!(rx.recv(), Ok(8));
            assert_eq!(rx.recv(), Ok(9));
        });
    });

    let (tx, rx) = bounded(0);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(tx.send(7), Ok(()));
            thread::sleep(ms(1000));
            assert_eq!(tx.send(8), Ok(()));
            thread::sleep(ms(1000));
            assert_eq!(tx.send(9), Ok(()));
            assert_eq!(tx.send(10), Err(SendError(10)));
        });
        s.spawn(move || {
            thread::sleep(ms(1500));
            assert_eq!(rx.recv(), Ok(7));
            assert_eq!(rx.recv(), Ok(8));
            assert_eq!(rx.recv(), Ok(9));
        });
    });
}

#[test]
fn send_timeout() {
    let (tx, rx) = bounded(2);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(tx.send_timeout(1, ms(1000)), Ok(()));
            assert_eq!(tx.send_timeout(2, ms(1000)), Ok(()));
            assert_eq!(
                tx.send_timeout(3, ms(500)),
                Err(SendTimeoutError::Timeout(3))
            );
            thread::sleep(ms(1000));
            assert_eq!(tx.send_timeout(4, ms(1000)), Ok(()));
            thread::sleep(ms(1000));
            assert_eq!(tx.send(5), Err(SendError(5)));
        });
        s.spawn(move || {
            thread::sleep(ms(1000));
            assert_eq!(rx.recv(), Ok(1));
            thread::sleep(ms(1000));
            assert_eq!(rx.recv(), Ok(2));
            assert_eq!(rx.recv(), Ok(4));
        });
    });

    let (tx, rx) = bounded(0);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(
                tx.send_timeout(7, ms(1000)),
                Err(SendTimeoutError::Timeout(7))
            );
            assert_eq!(tx.send_timeout(8, ms(1000)), Ok(()));
            assert_eq!(
                tx.send_timeout(9, ms(1000)),
                Err(SendTimeoutError::Closed(9))
            );
        });
        s.spawn(move || {
            thread::sleep(ms(1500));
            assert_eq!(rx.recv(), Ok(8));
        });
    });
}

#[test]
fn try_send() {
    let (tx, rx) = bounded(1);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(tx.try_send(1), Ok(()));
            assert_eq!(tx.try_send(2), Err(TrySendError::Full(2)));
            thread::sleep(ms(1500));
            assert_eq!(tx.try_send(3), Ok(()));
            thread::sleep(ms(500));
            assert_eq!(tx.try_send(4), Err(TrySendError::Closed(4)));
        });
        s.spawn(move || {
            thread::sleep(ms(1000));
            assert_eq!(rx.try_recv(), Ok(1));
            assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
            assert_eq!(rx.recv(), Ok(3));
        });
    });

    let (tx, rx) = bounded(0);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    crossbeam::scope(|s| {
        s.spawn(move || {
            assert_eq!(tx.try_send(7), Err(TrySendError::Full(7)));
            thread::sleep(ms(1500));
            assert_eq!(tx.try_send(8), Ok(()));
            thread::sleep(ms(500));
            assert_eq!(tx.try_send(9), Err(TrySendError::Closed(9)));
        });
        s.spawn(move || {
            thread::sleep(ms(1000));
            assert_eq!(rx.recv(), Ok(8));
        });
    });
}

#[test]
fn recv_after_close() {
    let (tx, rx) = bounded(100);
    let tx = WrappedSender(tx);
    let rx = WrappedReceiver(rx);

    tx.send(1).unwrap();
    tx.send(2).unwrap();
    tx.send(3).unwrap();

    drop(tx);

    assert_eq!(rx.recv(), Ok(1));
    assert_eq!(rx.recv(), Ok(2));
    assert_eq!(rx.recv(), Ok(3));
    assert_eq!(rx.recv(), Err(RecvError));
}

#[test]
fn matching() {
    let (tx, rx) = bounded(0);
    let (tx, rx) = (&tx, &rx);

    crossbeam::scope(|s| {
        for i in 0..44 {
            s.spawn(move || {
                select_loop! {
                    recv(rx, v) => assert_ne!(v, i),
                    send(tx, i) => {},
                }
            });
        }
    });

    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn matching_with_leftover() {
    let (tx, rx) = bounded(0);
    let (tx, rx) = (&tx, &rx);

    crossbeam::scope(|s| {
        for i in 0..55 {
            s.spawn(move || {
                select_loop! {
                    recv(rx, v) => assert_ne!(v, i),
                    send(tx, i) => {},
                }
            });
        }
        tx.send(!0).unwrap();
    });

    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn channel_through_channel() {
    const COUNT: usize = 1000;

    type T = Box<Any + Send>;

    for cap in 0..3 {
        let (tx, rx) = bounded::<T>(cap);

        crossbeam::scope(|s| {
            s.spawn(move || {
                let mut tx = tx;

                for _ in 0..COUNT {
                    let (new_tx, new_rx) = bounded(cap);
                    let mut new_rx: T = Box::new(Some(new_rx));

                    select_loop! {
                        send(tx, mut new_rx) => {}
                    }

                    tx = new_tx;
                }
            });

            s.spawn(move || {
                let mut rx = rx;

                for _ in 0..COUNT {
                    select_loop! {
                        recv(rx, mut r) => {
                            rx = r.downcast_mut::<Option<Receiver<T>>>()
                                .unwrap()
                                .take()
                                .unwrap();
                        }
                    }
                }
            });
        });
    }
}

#[test]
fn conditional_send() {
    let (tx, rx) = bounded(0);

    crossbeam::scope(|s| {
        s.spawn(move || rx.recv().unwrap());

        select_loop! {
            send(tx, ()) if 1 + 1 == 3 => panic!(),
            timed_out(ms(1000)) => {}
        }

        select_loop! {
            send(tx, ()) if 1 + 1 == 2 => {},
            timed_out(ms(1000)) => panic!(),
        }
    });
}

#[test]
fn conditional_recv() {
    let (tx, rx) = unbounded();
    tx.send(()).unwrap();

    select_loop! {
        recv(rx, _) if 1 + 1 == 3 => panic!(),
        timed_out(ms(1000)) => {}
    }

    select_loop! {
        recv(rx, _) if 1 + 1 == 2 => {},
        timed_out(ms(1000)) => panic!(),
    }
}

#[test]
fn conditional_closed() {
    let (_, rx) = bounded::<i32>(0);

    select_loop! {
        recv(rx, _) => panic!(),
        closed() if 1 + 1 == 3 => panic!(),
        would_block() => {}
        timed_out(ms(100)) => panic!(),
    }

    select_loop! {
        recv(rx, _) => panic!(),
        closed() if 1 + 1 == 2 => {}
        would_block() => panic!(),
        timed_out(ms(100)) => panic!(),
    }
}

#[test]
fn conditional_would_block() {
    let (_tx, rx) = bounded::<i32>(0);

    select_loop! {
        recv(rx, _) => panic!(),
        closed() => panic!(),
        would_block() if 1 + 1 == 3 => panic!(),
        timed_out(ms(100)) => {}
    }

    select_loop! {
        recv(rx, _) => panic!(),
        closed() => panic!(),
        would_block() if 1 + 1 == 2 => {}
        timed_out(ms(100)) => panic!(),
    }
}

#[test]
fn conditional_timed_out() {
    let (_tx, rx) = bounded::<i32>(0);

    select_loop! {
        recv(rx, _) => panic!(),
        timed_out(ms(100)) if 1 + 1 == 3 => panic!(),
        timed_out(ms(1000)) if 1 + 1 == 2 => {}
    }

    select_loop! {
        recv(rx, _) => panic!(),
        timed_out(ms(100)) if 1 + 1 == 2 => {}
        timed_out(ms(1000)) if 1 + 1 == 3 => panic!(),
    }
}

#[test]
fn conditional_option_unwrap() {
    let (tx, rx) = unbounded();
    tx.send(()).unwrap();
    let rx = Some(&rx);

    select_loop! {
        recv(rx.unwrap(), _) if rx.is_some() => {}
        would_block() => panic!()
    }
}
