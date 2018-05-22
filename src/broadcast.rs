use parking_lot::RwLock;

use channel::Sender;

pub struct Broadcast<T> {
    senders: RwLock<Vec<Sender<T>>>,
}

unsafe impl<T: Send> Send for Broadcast<T> {}
unsafe impl<T: Send> Sync for Broadcast<T> {}

impl<T> Broadcast<T> {
    pub fn new() -> Broadcast<T> {
        Broadcast {
            senders: RwLock::new(Vec::new()),
        }
    }

    pub fn register(&self, tx: Sender<T>) {
        self.senders.write().push(tx);
    }
}

impl<T: Clone> Broadcast<T> {
    pub fn broadcast(&self, msg: T) {
        let mut needs_cleanup = false;

        {
            let senders = self.senders.write();

            for s in senders.iter().skip(1) {
                if s.send(msg.clone()).is_err() {
                    needs_cleanup = true;
                }
            }

            if let Some(s) = senders.get(0) {
                if s.send(msg).is_err() {
                    needs_cleanup = true;
                }
            }
        }

        if needs_cleanup {
            self.senders.write().retain(|s| !s.is_disconnected());
        }
    }
}

impl<T> Clone for Broadcast<T> {
    fn clone(&self) -> Broadcast<T> {
        Broadcast {
            senders: RwLock::new(self.senders.read().clone()),
        }
    }
}
