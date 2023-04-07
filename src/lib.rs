#![doc = include_str!("../README.md")]
#![warn(missing_docs, missing_debug_implementations)]

use std::{
    future::Future,
    process::abort,
    sync::atomic::{AtomicBool, AtomicIsize, Ordering},
    task::{Poll, Waker},
    thread::{self, current, Thread},
};

use crossbeam_queue::SegQueue;

/// A HybridFutex is a synchronization primitive that allows threads to wait for
/// a notification from another thread. The HybridFutex maintains a counter that
/// represents the number of waiters, and a queue of waiters. The counter is
/// incremented when a thread calls `wait_sync` or `wait_async` methods, and
/// decremented when a thread calls `notify_one` or `notify_many` methods.
/// A thread calling `wait_sync` or `wait_async` is blocked until it is notified
/// by another thread calling `notify_one` or `notify_many`.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::thread;
/// use std::time::Duration;
/// use hybridfutex::HybridFutex;
///
/// let wait_queue = Arc::new(HybridFutex::new());
/// let wait_queue_clone = wait_queue.clone();
///
/// // Spawn a thread that waits for a notification from another thread
/// let handle = thread::spawn(move || {
///     println!("Thread 1 is waiting");
///     wait_queue_clone.wait_sync();
///     println!("Thread 1 is notified");
/// });
///
/// // Wait for a short time before notifying the other thread
/// thread::sleep(Duration::from_millis(100));
///
/// // Notify the other thread
/// wait_queue.notify_one();
///
/// // Wait for the other thread to finish
/// handle.join().unwrap();
/// ```
#[derive(Debug)]
pub struct HybridFutex {
    counter: AtomicIsize,
    queue: SegQueue<Waiter>,
}

impl Default for HybridFutex {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridFutex {
    /// Creates a new HybridFutex with an initial counter of 0 and an empty
    /// queue of waiters.
    ///
    /// # Examples
    ///
    /// ```
    /// use hybridfutex::HybridFutex;
    ///
    /// let wait_queue = HybridFutex::new();
    /// ```
    pub fn new() -> Self {
        Self {
            counter: AtomicIsize::new(0),
            queue: SegQueue::new(),
        }
    }
    /// Returns the current value of the counter of this HybridFutex.
    ///
    /// # Examples
    ///
    /// ```
    /// use hybridfutex::HybridFutex;
    ///
    /// let wait_queue = HybridFutex::new();
    ///
    /// assert_eq!(wait_queue.get_counter(), 0);
    /// ```
    pub fn get_counter(&self) -> isize {
        self.counter.load(Ordering::Relaxed)
    }
    /// Blocks the current thread until it is notified by another thread using
    /// the `notify_one` or `notify_many` method. The method increments the
    /// counter of the HybridFutex to indicate that the current thread is
    /// waiting. If the counter is already negative, the method does not
    /// block the thread and immediately returns.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::thread;
    /// use std::time::Duration;
    /// use hybridfutex::HybridFutex;
    ///
    /// let wait_queue = Arc::new(HybridFutex::new());
    /// let wait_queue_clone = wait_queue.clone();
    ///
    /// // Spawn a thread that waits for a notification from another thread
    /// let handle = thread::spawn(move || {
    ///     println!("Thread 1 is waiting");
    ///     wait_queue_clone.wait_sync();
    ///     println!("Thread 1 is notified");
    /// });
    ///
    /// // Wait for a short time before notifying the other thread
    /// thread::sleep(Duration::from_millis(100));
    ///
    /// // Notify the other thread
    /// wait_queue.notify_one();
    ///
    /// // Wait for the other thread to finish
    /// handle.join().unwrap();
    /// ```
    pub fn wait_sync(&self) {
        let old_counter = self.counter.fetch_add(1, Ordering::SeqCst);
        if old_counter >= 0 {
            let awaken = AtomicBool::new(false);
            self.queue.push(Waiter::Sync(SyncWaiter {
                awaken: &awaken,
                thread: current(),
            }));
            while {
                thread::park();
                !awaken.load(Ordering::Acquire)
            } {}
        }
    }
    /// Returns a `WaitFuture` that represents a future that resolves when the
    /// current thread is notified by another thread using the `notify_one` or
    /// `notify_many` method. The method increments the counter of the
    /// HybridFutex to indicate that the current thread is waiting.
    /// If the counter is already negative, the future immediately resolves.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::thread;
    /// use std::time::Duration;
    /// use hybridfutex::HybridFutex;
    /// use futures::executor::block_on;
    ///
    /// let wait_queue = Arc::new(HybridFutex::new());
    ///
    /// // Spawn a thread that waits for a notification from another thread
    /// let wqc = wait_queue.clone();
    /// let handle = thread::spawn(move || {
    ///     let fut = wqc.wait_async();
    ///     let _ = block_on(fut);
    ///     println!("Thread 1 is notified");
    /// });
    ///
    /// // Wait for a short time before notifying the other thread
    /// thread::sleep(Duration::from_millis(100));
    ///
    /// // Notify the other thread
    /// wait_queue.notify_one();
    ///
    /// // Wait for the other thread to finish
    /// handle.join().unwrap();
    /// ```
    pub fn wait_async(&self) -> WaitFuture {
        WaitFuture {
            state: 0.into(),
            wq: self,
        }
    }

    /// Notifies one waiting thread that is waiting on this HybridFutex using
    /// the `wait_sync` or `wait_async` method. If there is no current waiting
    /// threads, this function call indirectly notifies future call to
    /// `wait_sync` or `wait_async` using the internal counter.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::thread;
    /// use std::time::Duration;
    /// use hybridfutex::HybridFutex;
    ///
    /// let wait_queue = Arc::new(HybridFutex::new());
    /// let wait_queue_clone = wait_queue.clone();
    ///
    /// // Spawn a thread that waits for a notification from another thread
    /// let handle = thread::spawn(move || {
    ///     println!("Thread 1 is waiting");
    ///     wait_queue_clone.wait_sync();
    ///     println!("Thread 1 is notified");
    /// });
    ///
    /// // Wait for a short time before notifying the other thread
    /// thread::sleep(Duration::from_millis(100));
    ///
    /// // Notify the other thread
    /// wait_queue.notify_one();
    ///
    /// // Wait for the other thread to finish
    /// handle.join().unwrap();
    /// ```
    pub fn notify_one(&self) {
        let old_counter = self.counter.fetch_sub(1, Ordering::SeqCst);
        if old_counter > 0 {
            loop {
                if let Some(waker) = self.queue.pop() {
                    waker.wake();
                    break;
                }
            }
        }
    }

    /// Notifies a specified number of waiting threads that are waiting on this
    /// HybridFutex using the `wait_sync` or `wait_async` method. If there are
    /// less waiting threads than provided count, it indirectly notifies
    /// futures calls to to `wait_sync` and `wait_async` using the internal
    /// counter.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::thread;
    /// use std::time::Duration;
    /// use hybridfutex::HybridFutex;
    ///
    /// let wait_queue = Arc::new(HybridFutex::new());
    ///
    /// // Spawn multiple threads that wait for a notification from another thread
    /// let handles: Vec<_> = (0..3).map(|i| {
    ///     let wait_queue_clone = wait_queue.clone();
    ///     thread::spawn(move || {
    ///         println!("Thread {} is waiting", i);
    ///         wait_queue_clone.wait_sync();
    ///         println!("Thread {} is notified", i);
    ///     })
    /// }).collect();
    ///
    /// // Wait for a short time before notifying the threads
    /// thread::sleep(Duration::from_millis(100));
    ///
    /// // Notify two threads
    /// wait_queue.notify_many(2);
    ///
    /// // Notify single thread
    /// wait_queue.notify_one();
    ///
    /// // Wait for the other threads to finish
    /// for handle in handles {
    ///     handle.join().unwrap();
    /// }
    /// ```
    pub fn notify_many(&self, count: usize) {
        let count = count as isize;
        let old_counter = self.counter.fetch_sub(count, Ordering::SeqCst);
        if old_counter > 0 {
            for _ in 0..old_counter.min(count) {
                loop {
                    if let Some(waker) = self.queue.pop() {
                        waker.wake();
                        break;
                    }
                }
            }
        }
    }
}

enum Waiter {
    Sync(SyncWaiter),
    Async(AsyncWaiter),
}

unsafe impl Send for Waiter {}

impl Waiter {
    fn wake(self) {
        match self {
            Waiter::Sync(w) => w.wake(),
            Waiter::Async(w) => w.wake(),
        }
    }
}

struct SyncWaiter {
    awaken: *const AtomicBool,
    thread: Thread,
}

impl SyncWaiter {
    fn wake(self) {
        unsafe {
            (*self.awaken).store(true, Ordering::Release);
        }
        self.thread.unpark();
    }
}

struct AsyncWaiter {
    state: *const AtomicIsize,
    waker: Waker,
}

impl AsyncWaiter {
    fn wake(self) {
        unsafe {
            (*self.state).store(!0, Ordering::Release);
        }
        self.waker.wake();
    }
}
#[derive(Debug)]
/// A future representing a thread that is waiting for a notification from
/// another thread using a `HybridFutex` synchronization primitive.
pub struct WaitFuture<'a> {
    /// The current state of the future, represented as an `AtomicIsize`.
    /// The value of this field is `0` if the future has not yet been polled,
    /// `1` if the future is waiting for a notification, and `!0` if the future
    /// has been notified.
    state: AtomicIsize,

    /// A reference to the `HybridFutex` that this future is waiting on.
    wq: &'a HybridFutex,
}

impl<'a> Future for WaitFuture<'a> {
    type Output = ();
    /// Polls the future, returning `Poll::Pending` if the future is still waiting
    /// for a notification, and `Poll::Ready(())` if the future has been notified.
    ///
    /// If the future has not yet been polled, this method increments the counter
    /// of the `HybridFutex` that the future is waiting on to indicate that the
    /// current thread is waiting. If the counter is already negative, the future
    /// immediately resolves and returns `Poll::Ready(())`. Otherwise, the method
    /// pushes a new `AsyncWaiter` onto the queue of waiters for the `HybridFutex`,
    /// and returns `Poll::Pending`.
    ///
    /// If the future has already been polled and the value of the `state` field is
    /// `1`, this method simply returns `Poll::Pending` without modifying the state
    /// or the queue of waiters.
    ///
    /// If the future has already been notified and the value of the `state` field
    /// is `!0`, this method returns `Poll::Ready(())` without modifying the state
    /// or the queue of waiters.
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        match self.state.load(Ordering::Acquire) {
            0 => {
                // If the future has not yet been polled, increment the counter
                // of the HybridFutex and push a new AsyncWaiter onto the queue.
                let old_counter = self.wq.counter.fetch_add(1, Ordering::SeqCst);
                if old_counter >= 0 {
                    self.state.store(1, Ordering::Relaxed);
                    self.wq.queue.push(Waiter::Async(AsyncWaiter {
                        state: &self.state,
                        waker: cx.waker().clone(),
                    }));
                    Poll::Pending
                } else {
                    // If the counter is negative, the future has already been
                    // notified, so set the state to !0 and return Poll::Ready(()).
                    self.state.store(!0, Ordering::Relaxed);
                    Poll::Ready(())
                }
            }
            1 => Poll::Pending,
            _ => Poll::Ready(()),
        }
    }
}

impl<'a> Drop for WaitFuture<'a> {
    /// Drops the future, checking whether it has been polled before and
    /// panicking if it has not. This is to prevent potential memory leaks
    /// if the future is dropped before being polled.
    fn drop(&mut self) {
        if self.state.load(Ordering::Relaxed) == 1 {
            abort();
        }
    }
}
