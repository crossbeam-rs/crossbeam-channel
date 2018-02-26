// #![cfg(feature="futures")]

extern crate crossbeam_channel;
extern crate futures;

use std::time::Duration;
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use crossbeam_channel::futures::mpmc;

use futures::prelude::*;
use futures::future::lazy;

trait AssertSend: Send {}
impl AssertSend for mpmc::Sender<i32> {}
impl AssertSend for mpmc::Receiver<i32> {}

pub trait ForgetExt {
    fn forget(self);
}

impl<F> ForgetExt for F
    where F: Future + Sized + Send + 'static,
          F::Item: Send,
          F::Error: Send
{
    fn forget(self) {
        thread::spawn(|| self.wait());
    }
}

#[test]
fn send_recv() {
    let (tx, rx) = mpmc::channel::<i32>();
    let mut rx = rx.wait();

    tx.send(1).wait().unwrap();

    assert_eq!(rx.next().unwrap(), Ok(1));
}

#[test]
fn send_recv_shared() {
	let (tx, rx) = mpmc::channel::<i32>();
	let clone1 = rx.clone();
	let clone2 = rx.clone();

	thread::spawn(move || {
		let rx = clone1.clone();
		let mut rx = rx.wait();
		for item in rx {
			println!("[thread1]: Got Item {:?}", &item);
			assert!(true);
		}
	});

	thread::spawn(move || {
		let rx = clone2.clone();
		let mut rx = rx.wait();
		for item in rx {
			println!("[thread2]: Got Item {:?}", &item);
			assert!(true);
		}
	});

	tx.send(1)
		.and_then(|tx| tx.send(2))
		.and_then(|tx| tx.send(3))
		.and_then(|tx| tx.send(4))
		.and_then(|tx| tx.send(5))
		.and_then(|tx| tx.send(6))
		.wait()
		.unwrap();
}

// #[test]
// fn send_recv_no_buffer() {
//     //let (mut tx, mut rx) = mpmc::channel::<i32>(0);
//     let (mut tx, mut rx) = mpmc::channel::<i32>(1);

//     // Run on a task context
//     lazy(move || {
//         assert!(tx.poll_ready().unwrap().is_ready());

//         // Send first message
//         tx.try_send(1).unwrap();
//         assert!(tx.poll_ready().unwrap().is_not_ready());

//         // Send second message
//         assert!(tx.try_send(2).is_err());

//         // Take the value
//         assert_eq!(rx.poll().unwrap(), Async::Ready(Some(1)));
//         assert!(tx.poll_ready().unwrap().is_ready());

//         tx.try_send(2).unwrap();
//         assert!(tx.poll_ready().unwrap().is_not_ready());

//         // Take the value
//         assert_eq!(rx.poll().unwrap(), Async::Ready(Some(2)));
//         assert!(tx.poll_ready().unwrap().is_ready());

//         Ok::<(), ()>(())
//     }).wait().unwrap();
// }

 #[test]
 fn send_shared_recv() {
     let (tx1, rx) = mpmc::channel::<i32>();
     let tx2 = tx1.clone();
     let mut rx = rx.wait();

     tx1.send(1).wait().unwrap();
     assert_eq!(rx.next().unwrap(), Ok(1));

     tx2.send(2).wait().unwrap();
     assert_eq!(rx.next().unwrap(), Ok(2));
 }

 #[test]
 fn send_recv_threads() {
     let (tx, rx) = mpmc::channel::<i32>();
     let mut rx = rx.wait();

     thread::spawn(move|| {
         tx.send(1).wait().unwrap();
     });

     assert_eq!(rx.next().unwrap(), Ok(1));
 }

 #[test]
 fn send_recv_threads_no_capacity() {
     let (mut tx, rx) = mpmc::channel::<i32>();
     let mut rx = rx.wait();

     let t = thread::spawn(move|| {
         tx = tx.send(1).wait().unwrap();
         tx = tx.send(2).wait().unwrap();
     });

     thread::sleep(Duration::from_millis(100));
     assert_eq!(rx.next().unwrap(), Ok(1));

     thread::sleep(Duration::from_millis(100));
     assert_eq!(rx.next().unwrap(), Ok(2));

     t.join().unwrap();
 }

 #[test]
 fn recv_close_gets_none() {
     let (mut tx, mut rx) = mpmc::channel::<i32>();

     // Run on a task context
     lazy(move || {
         rx.close();

         assert_eq!(rx.poll(), Ok(Async::Ready(None)));
         assert!(tx.poll_ready().is_err());

         drop(tx);

         Ok::<(), ()>(())
     }).wait().unwrap();
 }


 #[test]
 fn tx_close_gets_none() {
     let (_, mut rx) = mpmc::channel::<i32>();

     // Run on a task context
     lazy(move || {
         assert_eq!(rx.poll(), Ok(Async::Ready(None)));
         assert_eq!(rx.poll(), Ok(Async::Ready(None)));

         Ok::<(), ()>(())
     }).wait().unwrap();
 }

 #[test]
 fn stress_shared_bounded_hard() {
     const AMT: u32 = 5000;
     const NTHREADS: u32 = 8;
     let (tx, rx) = mpmc::channel::<i32>();
     let mut rx = rx.wait();

     let t = thread::spawn(move|| {
         for _ in 0..AMT * NTHREADS {
             assert_eq!(rx.next().unwrap(), Ok(1));
         }

         if rx.next().is_some() {
             panic!();
         }
     });

     for _ in 0..NTHREADS {
         let mut tx = tx.clone();

         thread::spawn(move|| {
             for _ in 0..AMT {
                 tx = tx.send(1).wait().unwrap();
             }
         });
     }

     drop(tx);

     t.join().ok().unwrap();
 }

// #[test]
// fn stress_receiver_multi_task_bounded_hard() {
//     const AMT: usize = 10_000;
//     const NTHREADS: u32 = 2;

//     let (mut tx, rx) = mpmc::channel::<usize>(1);
//     let rx = Arc::new(Mutex::new(Some(rx)));
//     let n = Arc::new(AtomicUsize::new(0));

//     let mut th = vec![];

//     for _ in 0..NTHREADS {
//         let rx = rx.clone();
//         let n = n.clone();

//         let t = thread::spawn(move || {
//             let mut i = 0;

//             loop {
//                 i += 1;
//                 let mut lock = rx.lock().ok().unwrap();

//                 match lock.take() {
//                     Some(mut rx) => {
//                         if i % 5 == 0 {
//                             let (item, rest) = rx.into_future().wait().ok().unwrap();

//                             if item.is_none() {
//                                 break;
//                             }

//                             n.fetch_add(1, Ordering::Relaxed);
//                             *lock = Some(rest);
//                         } else {
//                             // Just poll
//                             let n = n.clone();
//                             let r = lazy(move || {
//                                 let r = match rx.poll().unwrap() {
//                                     Async::Ready(Some(_)) => {
//                                         n.fetch_add(1, Ordering::Relaxed);
//                                         *lock = Some(rx);
//                                         false
//                                     }
//                                     Async::Ready(None) => {
//                                         true
//                                     }
//                                     Async::NotReady => {
//                                         *lock = Some(rx);
//                                         false
//                                     }
//                                 };

//                                 Ok::<bool, ()>(r)
//                             }).wait().unwrap();

//                             if r {
//                                 break;
//                             }
//                         }
//                     }
//                     None => break,
//                 }
//             }
//         });

//         th.push(t);
//     }

//     for i in 0..AMT {
//         tx = tx.send(i).wait().unwrap();
//     }

//     drop(tx);

//     for t in th {
//         t.join().unwrap();
//     }

//     assert_eq!(AMT, n.load(Ordering::Relaxed));
// }

// /// Stress test that receiver properly receives all the messages
// /// after sender dropped.
// #[test]
// fn stress_drop_sender() {
//     fn list() -> Box<Stream<Item=i32, Error=u32>> {
//         let (tx, rx) = mpmc::channel();
//         tx.send(Ok(1))
//           .and_then(|tx| tx.send(Ok(2)))
//           .and_then(|tx| tx.send(Ok(3)))
//           .forget();
//         Box::new(rx.then(|r| r.unwrap()))
//     }
//
//     for _ in 0..1000 {
//         assert_eq!(list().wait().collect::<Result<Vec<_>, _>>(),
//                    Ok(vec![1, 2, 3]));
//         eprintln!("done!");
//     }
// }

// #[test]
// fn try_send_1() {
//     const N: usize = 3000;
//     let (mut tx, rx) = mpmc::channel(1);

//     let t = thread::spawn(move || {
//         for i in 0..N {
//             loop {
//                 if tx.try_send(i).is_ok() {
//                     break
//                 }
//             }
//         }
//     });
//     for (i, j) in rx.wait().enumerate() {
//         assert_eq!(i, j.unwrap());
//     }
//     t.join().unwrap();
// }

// #[test]
// fn try_send_2() {
//     use std::thread;
//     use std::time::Duration;

//     let (mut tx, rx) = mpmc::channel(1);

//     tx.try_send("hello").unwrap();

//     let th = thread::spawn(|| {
//         lazy(|| {
//             assert!(tx.try_send("fail").is_err());
//             Ok::<_, ()>(())
//         }).wait().unwrap();

//         tx.send("goodbye").wait().unwrap();
//     });

//     // Little sleep to hopefully get the action on the thread to happen first
//     thread::sleep(Duration::from_millis(300));

//     let mut rx = rx.wait();

//     assert_eq!(rx.next(), Some(Ok("hello")));
//     assert_eq!(rx.next(), Some(Ok("goodbye")));
//     assert!(rx.next().is_none());

//     th.join().unwrap();
// }

// #[test]
// fn try_send_fail() {
//     //let (mut tx, rx) = mpmc::channel(0);
//     let (mut tx, rx) = mpmc::channel(1);
//     let mut rx = rx.wait();

//     tx.try_send("hello").unwrap();

//     // This should fail
//     assert!(tx.try_send("fail").is_err());

//     assert_eq!(rx.next(), Some(Ok("hello")));

//     tx.try_send("goodbye").unwrap();
//     drop(tx);

//     assert_eq!(rx.next(), Some(Ok("goodbye")));
//     assert!(rx.next().is_none());
// }
