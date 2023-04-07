use futures::executor::block_on;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use hybridfutex::*;

fn sleep() {
    thread::sleep(Duration::from_millis(500));
}

#[test]
fn test_wait_sync() {
    let wq = Arc::new(HybridFutex::new());

    let wq1 = wq.clone();
    let handle = thread::spawn(move || {
        wq1.wait_sync();
    });

    sleep();
    wq.notify_one();

    handle.join().unwrap();
}

#[test]
fn test_wait_sync_delayed() {
    let wq = Arc::new(HybridFutex::new());

    let wq1 = wq.clone();
    let handle = thread::spawn(move || {
        sleep();
        wq1.wait_sync();
    });

    wq.notify_one();

    handle.join().unwrap();
}

#[test]
fn test_wait_async() {
    let wq = Arc::new(HybridFutex::new());

    let wq1 = wq.clone();
    let handle = thread::spawn(move || {
        block_on(wq1.wait_async());
    });

    sleep();
    wq.notify_one();

    handle.join().unwrap();
}

#[test]
fn test_wait_async_delayed() {
    let wq = Arc::new(HybridFutex::new());

    let wq1 = wq.clone();
    let handle = thread::spawn(move || {
        sleep();
        block_on(wq1.wait_async());
    });

    wq.notify_one();

    handle.join().unwrap();
}

#[test]
fn test_wait_async_drop() {
    let wq = Arc::new(HybridFutex::new());

    let handle = thread::spawn(move || {
        // Wait for a bit to make sure the future is waiting.
        thread::sleep(std::time::Duration::from_millis(100));
    });

    drop(wq.wait_async());

    handle.join().unwrap();
}

#[test]
fn test_wake_empty() {
    let wq = Arc::new(HybridFutex::new());

    wq.notify_one();
}

#[test]
fn test_notify_many() {
    let queue = Arc::new(HybridFutex::new());
    let num_waiters = 5;
    let mut threads = Vec::new();

    for _ in 0..num_waiters {
        let q = queue.clone();
        threads.push(thread::spawn(move || {
            q.wait_sync();
        }));
    }

    queue.notify_many(num_waiters);

    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(queue.get_counter(), 0);
}

#[test]
fn test_notify_many_with_more_count_than_waiters() {
    let queue = Arc::new(HybridFutex::new());
    let num_waiters = 5;
    let mut threads = Vec::new();

    for _ in 0..num_waiters {
        let q = queue.clone();
        threads.push(thread::spawn(move || {
            q.wait_sync();
        }));
    }

    queue.notify_many(num_waiters + 2);

    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(queue.get_counter(), -2);
}

#[test]
fn test_notify_many_with_dropped_waiters() {
    let queue = Arc::new(HybridFutex::new());
    let num_waiters = 5;
    let mut threads = Vec::new();

    for _ in 0..num_waiters {
        let q = queue.clone();
        threads.push(thread::spawn(move || {
            let fut = q.wait_async();
            drop(fut);
        }));
    }

    queue.notify_many(num_waiters);

    for t in threads {
        t.join().unwrap();
    }

    assert_eq!(queue.get_counter(), -(num_waiters as isize));
}
