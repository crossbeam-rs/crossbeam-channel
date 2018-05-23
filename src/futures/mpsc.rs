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


/// The inner channel data structure shared among the senders and the receiver of a future-aware
/// mpsc channel.
struct Channel<T> {

    /// The count of `Sender` handles outstanding. When the count reaches 0, the final `Sender`
    /// will disconnect the `items` and `sender_tasks` channels and notify the receiver task that
    /// the channel is no longer active.
    senders: AtomicUsize,

    /// The bounded queue of items. `Sender`s push to the channel, and the `Receiver` pops items
    /// as they become available.
    items: array::Channel<T>,

    /// An unbounded queue of parked `Sender` tasks which are waiting for capacity in the `items`
    /// channel to become available.
    ///
    /// ## `Sender` Usage
    ///
    /// During the first attempt to send an item to a channel, if the `Sender` discovers that there
    /// is no channel capacity available, it will create a new `SenderTask`, send it to this
    /// channel, and park itself (return `NotReady`).
    ///
    /// During subsequent attempts to send the item to the channel, the `Sender` will
    /// simultaneously register it's task with the `SenderTask` and check if the `SenderTask` has
    /// already been notified (and thus removed from the `senders_task` queue) via
    /// `SenderTask::register`. If during this subsequent attempt to send an item there is still no
    /// capacity, and the existing `SenderTask` has already been notified, then the `Sender` will
    /// enqueue a a new `SenderTask` to this channel to ensure it will be notified.
    ///
    /// ## `Receiver` Usage
    ///
    /// When there is channel capacity available, the `Receiver` is responsible for dequeing sender
    /// tasks from this channel and notifying them. The `Receiver` will notify one sender task per
    /// item dequeued from the `items` channel.
    ///
    /// ## Notes on `Sender::drop`
    ///
    /// If a `Sender` is dropped while it has a sender task enqueued on this channel, it is
    /// responsible for notifying an alternate sender task. This ensures that there are no 'lost
    /// wakeups', which can occur when the `Receiver` notifies a sender task, but the sender task
    /// is immediately dropped, or never again attempts to send an item to the channel.
    sender_tasks: list::Channel<Arc<SenderTask>>,

    /// A notifier for the `Receiver`'s task.
    ///
    /// `Sender` instances are responsible for notifying the `Receiver` after taking any action
    /// which the `Receiver` must wake up to handle.
    ///
    /// * a new item is enqueued to `items`
    /// * a new parked sender task is enqueued to `sender_tasks`
    /// * the final `Sender` is dropped, and the channel is disconnected
    receiver_task: AtomicTask,
}

impl <T> Channel<T> {

    /// Notifies a single sender task that channel capacity is available.
    fn notify_one_sender(&self) {
        while let Ok(sender) = self.sender_tasks.try_recv() {
            if sender.notify().is_ok() {
                break;
            }
        }
    }

    /// Notifies all sender tasks that the channel has been disconnected.
    fn notify_all_senders(&self) {
        debug_assert!(self.items.is_disconnected());
        debug_assert!(self.sender_tasks.is_disconnected());
        while let Ok(sender) = self.sender_tasks.try_recv() {
            let _ = sender.notify();
        }
    }
}

/// Creates an in-memory, bounded, futures-aware MPSC channel.
///
/// This method returns `Sender` and `Receiver` instance which can be used to send values across
/// tasks. This channel is unique in that it implements back pressure to ensure that the sender
/// never outpaces the receiver. The channel will refuse sending new items when there are `cap`
/// items already in-flight.
///
/// The `Receiver` returned implements the `Stream` trait and has access to all of the associated
/// combinators for transforming the result.
///
/// The `Sender` returned implements the `Sink` trait and has access to all of the associated
/// combinators to transform the input. `Sender` may be cloned freely and sent across tasks.
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

/// The producer, or sender half of a futures-aware, bounded, MPSC channel.
pub struct Sender<T> {
    channel: Arc<Channel<T>>,
    task: Option<Arc<SenderTask>>,
}

unsafe impl<T: Send> Send for Sender<T> {}
unsafe impl<T: Send> Sync for Sender<T> {}

impl <T> Sender<T> {

    /// Attempts to send a message on this `Sender` without blocking.
    ///
    /// This function, unlike `start_send` and `send`, is safe to call whether it's being called on
    /// a task or not. Note that this function, however, will *not* attempt to block the current
    /// task if the message cannot be sent.
    ///
    /// It is not recommended to call this function from inside of a future, only from an external
    /// thread where you've otherwise arranged to be notified when the channel is no longer full.
    /// While insed a future, the `send` method should be preferred.
    pub fn try_send(&mut self, item: T) -> Result<(), TrySendError<T>> {
        self.channel.items.try_send(item)?;
        // Notify the receiver, since the channel may have been empty.
        self.channel.receiver_task.notify();
        Ok(())
    }

    /// Polls the channel to determine if there may be capacity to send at least one item without
    /// waiting.
    ///
    /// Returns `Ok(Async::Ready(_))` if there may be sufficient capacity, or returns
    /// `Ok(Async::NotReady)` if the channel may not have capacity. Returns `Err(SendError(_))` if
    /// the receiver has been dropped.
    ///
    /// # Panics
    ///
    /// This method will panic if called from outside the context of a task or future.
    pub fn poll_ready(&mut self) -> Poll<(), SendError<()>> {
        // Register the current task as the parked sender task, or if it's already been notified,
        // then discard it.
        let task = self.task.take().and_then(|task| match task.register() {
            Ok(()) => Some(task),
            Err(()) => None,
        });

        if self.channel.items.len() >= self.channel.items.capacity() {
            // There is no channel capacity available. If this sender hasn't already sent its task
            // to the parked tasks channel, then do so.
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
                // Try send indicated the channel is full. Call poll_ready() in order to register
                // ourselves with the `sender_tasks` channel, and go to sleep.
                Err(TrySendError::Full(i)) => match self.poll_ready() {
                    Ok(Async::Ready(())) => {
                        // Between the calls to try_send() and poll_ready() some capacity became
                        // available, so try to send again.
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
            self.channel.notify_one_sender();
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
        // Ensure the `Sender` is closed, which will notify another `Sender` if necessary.
        let _ = self.close();
        if self.channel.senders.fetch_sub(1, SeqCst) == 1 {
            // This is the final Sender. Disconnect the channels and notify the receiver.
            self.channel.items.disconnect();
            self.channel.sender_tasks.disconnect();
            self.channel.receiver_task.notify();
        }
    }
}

/// The consumer, or receiver half of a futures-aware, bounded, MPSC channel.
pub struct Receiver<T>(Arc<Channel<T>>);

unsafe impl<T: Send> Send for Receiver<T> {}
unsafe impl<T: Send> Sync for Receiver<T> {}

impl <T> Receiver<T> {
    /// Closes the receiving half of the channel.
    ///
    /// This prevents any further messages from being sent on the channel while still enabling the
    /// receiver to drain messages that are buffered.
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
                self.0.notify_one_sender();
                return Ok(Async::Ready(Some(item)));
            },
            Err(TryRecvError::Empty) => {
                // TODO(danburkert): figure out why this notification is necessary. The tests
                // deadlock without it even with a single sender, but it may indicate we have a
                // 'lost wakeup' somewhere which this papers over.
                self.0.notify_one_sender();
                return Ok(Async::NotReady);
            },
            Err(TryRecvError::Disconnected) => {
                // The channel has been closed. Cleanup remaining parked tasks.
                self.0.sender_tasks.disconnect();
                self.0.notify_all_senders();
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
        self.0.notify_all_senders();
    }
}
