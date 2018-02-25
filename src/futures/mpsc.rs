use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

use futures_rs::{
    Async,
    AsyncSink,
    Poll,
    Sink,
    Stream,
    StartSend,
};
use futures_rs::task::AtomicTask;

use flavors::{array, list};
use err::{
    SendError,
    TryRecvError,
    TrySendError,
};
use futures::sender_task::SenderTask;

struct Channel<T> {
    senders: AtomicUsize,
    items: array::Channel<T>,
    sender_tasks: list::Channel<Arc<SenderTask>>,
    receiver_task: AtomicTask,
}

impl <T> Channel<T> {
    fn notify_one(&self) {
        while let Ok(sender) = self.sender_tasks.try_recv() {
            if sender.notify().is_ok() {
                break;
            }
        }
    }

    fn notify_all(&self) {
        while let Ok(sender) = self.sender_tasks.try_recv() {
            let _ = sender.notify();
        }
    }
}

/// Creates an in-memory channel implementation of the `Stream` trait with
/// bounded capacity.
///
/// This method creates a concrete implementation of the `Stream` trait which
/// can be used to send values across threads in a streaming fashion. This
/// channel is unique in that it implements back pressure to ensure that the
/// sender never outpaces the receiver. The channel will refuse sending new
/// items when there are `cap` items already in-flight.
///
/// The `Receiver` returned implements the `Stream` trait and has access to any
/// number of the associated combinators for transforming the result.
pub fn channel<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(Channel {
        senders: AtomicUsize::new(1),
        items: array::Channel::with_capacity(cap),
        sender_tasks: list::Channel::new(),
        receiver_task: AtomicTask::new(),
    });
    let sender = Sender {
        channel: channel.clone(),
        task: None,
    };
    let receiver = Receiver(channel);
    (sender, receiver)
}

pub struct Sender<T> {
    channel: Arc<Channel<T>>,
    task: Option<Arc<SenderTask>>,
}

unsafe impl<T: Send> Send for Sender<T> {}
unsafe impl<T: Send> Sync for Sender<T> {}

impl <T> Sender<T> {

    /// Attempts to send a message on this `Sender<T>` without blocking.
    ///
    /// This function, unlike `start_send`, is safe to call whether it's being
    /// called on a task or not. Note that this function, however, will *not*
    /// attempt to block the current task if the message cannot be sent.
    ///
    /// It is not recommended to call this function from inside of a future,
    /// only from an external thread where you've otherwise arranged to be
    /// notified when the channel is no longer full.
    pub fn try_send(&mut self, item: T) -> Result<(), TrySendError<T>> {
        self.channel.items.try_send(item)?;
        // Notify the receiver, since the channel may have been empty.
        self.channel.receiver_task.notify();
        Ok(())
    }

    /// Polls the channel to determine if there may be capacity to send at least one
    /// item without waiting.
    ///
    /// Returns `Ok(Async::Ready(_))` if there may be sufficient capacity, or returns
    /// `Ok(Async::NotReady)` if the channel may not have capacity. Returns
    /// `Err(SendError(_))` if the receiver has been dropped.
    ///
    /// # Panics
    ///
    /// This method will panic if called from outside the context of a task or future.
    pub fn poll_ready(&mut self) -> Poll<(), SendError<()>> {
        // Register the current task as the parked sender task, or if it's already been notified
        // then discard it.
        let task = self.task.take().and_then(|task| match task.register() {
            Ok(()) => Some(task),
            Err(()) => None,
        });

        if self.channel.items.len() >= self.channel.items.capacity() {
            // There's no channel capacity available. If this sender hasn't already sent its task
            // to the parked tasks channel then do so.
            let task = match task {
                Some(task) => task,
                None => {
                    let task = Arc::new(SenderTask::new());
                    self.channel.sender_tasks.send(task.clone()).map_err(|_| SendError(()))?;
                    self.channel.receiver_task.notify();
                    task
                }
            };
            self.task = Some(task.clone());
            Ok(Async::NotReady)
        } else if self.channel.items.is_disconnected() {
            Err(SendError(()))
        } else {
            Ok(Async::Ready(()))
        }
    }
}

impl <T> Sink for Sender<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, mut item: T) -> StartSend<T, SendError<T>> {
        loop {
            match self.try_send(item) {
                Ok(()) => return Ok(AsyncSink::Ready),
                Err(TrySendError::Full(i)) => match self.poll_ready() {
                    Ok(Async::Ready(())) => {
                        item = i;
                        continue
                    },
                    Ok(Async::NotReady) => return Ok(AsyncSink::NotReady(i)),
                    Err(SendError(())) => return Err(SendError(i)),
                },
                Err(TrySendError::Disconnected(i)) => return Err(SendError(i)),
            };
        }
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<T>> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), SendError<T>> {
        // If this Sender has a task in the parked_senders channel that has been notified, then
        // notify another parked sender.
        if self.task.take().map_or(false, |task| task.notify().is_err()) {
            self.channel.notify_one();
        }
        Ok(Async::Ready(()))
    }
}

impl <T> Clone for Sender<T> {

    fn clone(&self) -> Sender<T> {
        let channel = self.channel.clone();
        channel.senders.fetch_add(1, SeqCst);
        Sender{
            channel,
            task: None,
        }
    }
}

impl <T> Drop for Sender<T> {
    fn drop(&mut self) {
        let _ = self.close();
        if self.channel.senders.fetch_sub(1, SeqCst) == 1 {
            // This is the final Sender. Disconnect the channels and notify the receiver.
            self.channel.items.disconnect();
            self.channel.sender_tasks.disconnect();
            self.channel.receiver_task.notify();
        }
    }
}

pub struct Receiver<T>(Arc<Channel<T>>);

unsafe impl<T: Send> Send for Receiver<T> {}
unsafe impl<T: Send> Sync for Receiver<T> {}

impl <T> Receiver<T> {
    /// Closes the receiving half
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        self.0.items.disconnect();
        self.0.sender_tasks.disconnect();
    }
}

impl<T> Stream for Receiver<T> {
    type Item = T;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<T>, ()> {
        self.0.receiver_task.register();

        match self.0.items.try_recv() {
            Ok(item) => {
                // Success; an item has been received. Notify a single waiting task that a
                // slot has opened up.
                self.0.notify_one();
                return Ok(Async::Ready(Some(item)));
            },
            Err(TryRecvError::Empty) => {
                self.0.notify_one();
                return Ok(Async::NotReady);
            },
            Err(TryRecvError::Disconnected) => {
                // The channel has been closed. Cleanup remaining parked tasks.
                self.0.sender_tasks.disconnect();
                self.0.notify_all();
                return Ok(Async::Ready(None))
            },
        }
    }
}

impl <T> Drop for Receiver<T> {
    fn drop(&mut self) {
        // Disconnect the channels and notify remaining parked tasks.
        self.0.items.disconnect();
        self.0.sender_tasks.disconnect();
        self.0.notify_all();
    }
}
