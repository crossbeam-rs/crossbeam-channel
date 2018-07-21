extern crate crossbeam_channel as channel;

use std::thread;
use std::time::{Duration, Instant};

#[test]
fn foo() {
    // Converts a number into a `Duration` in milliseconds.
    let ms = |ms| Duration::from_millis(ms);

    // Returns `true` if `a` and `b` are very close `Instant`s.
    let eq = |a, b| a + ms(100) > b && b + ms(100) > a;

    let start = Instant::now();
    let r = channel::tick(ms(200));

    // This message was sent 200 ms from the start and received 200 ms from the start.
    assert!(eq(r.recv().unwrap(), start + ms(200)));
    assert!(eq(Instant::now(), start + ms(200)));

    thread::sleep(ms(500));

    // This message was sent 400 ms from the start and received 700 ms from the start.
    assert!(eq(r.recv().unwrap(), start + ms(400)));
    assert!(eq(Instant::now(), start + ms(700)), "700 => {:?} {:?} {:?}", Instant::now(), start, start + ms(700));

    // This message was sent 900 ms from the start and received 900 ms from the start.
    assert!(eq(r.recv().unwrap(), start + ms(900)));
    assert!(eq(Instant::now(), start + ms(900)), "900 => {:?} {:?} {:?}", Instant::now(), start, start + ms(900));
}
