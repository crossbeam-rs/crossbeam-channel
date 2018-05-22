extern crate crossbeam;
extern crate crossbeam_channel;

use crossbeam_channel::{Broadcast, bounded, unbounded};

#[test]
fn smoke() {
    let (tx1, rx1) = bounded(1);
    let (tx2, rx2) = unbounded();

    let b = Broadcast::new();
    b.register(tx1);
    b.register(tx2);
    b.broadcast(1);
    drop(b);

    assert_eq!(rx1.into_iter().collect::<Vec<_>>(), [1]);
    assert_eq!(rx2.into_iter().collect::<Vec<_>>(), [1]);
}
