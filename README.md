# HybridFutex

[![Crates.io][crates-badge]][crates-url]
[![Documentation][doc-badge]][doc-url]
[![MIT licensed][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/crates/v/hybridfutex.svg?style=for-the-badge
[crates-url]: https://crates.io/crates/hybridfutex
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg?style=for-the-badge
[mit-url]: https://github.com/fereidani/hybridfutex/blob/master/LICENSE
[doc-badge]: https://img.shields.io/docsrs/hybridfutex?style=for-the-badge
[doc-url]: https://docs.rs/hybridfutex

`HybridFutex` is a Rust library that provides a synchronization primitive that allows threads to wait for a notification from another thread. It is designed to be low-overhead and scalable and supports both synchronous and asynchronous waiting and notification.

## Features

- Support Notify many operations on any target.
- Support for synchronous and asynchronous waiting.
- Built-in fairness and scalability for high-contention scenarios.
- Efficient kernel-assisted blocking using park API.
- Low-latency waiting and notification on both Windows and Unix platforms.
- Cross-platform compatibility with no external dependencies.
- Simple and easy-to-use API.

## Usage

To use `HybridFutex`, simply add it as a dependency in your `Cargo.toml` file:

```toml
[dependencies]
hybrid-futex = "0.1"
```

Then, you can use `HybridFutex` in your Rust code as follows:

```rust
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use hybridfutex::HybridFutex;

let wait_queue = Arc::new(HybridFutex::new());
let wait_queue_clone = wait_queue.clone();

// Spawn a thread that waits for a notification from another thread
let handle = thread::spawn(move || {
    println!("Thread 1 is waiting");
    wait_queue_clone.wait_sync();
    println!("Thread 1 is notified");
});

// Wait for a short time before notifying the other thread
thread::sleep(Duration::from_millis(100));

// Notify the other thread
wait_queue.notify_one();

// Wait for the other thread to finish
handle.join().unwrap();
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- This library is inspired by the [futex](https://man7.org/linux/man-pages/man2/futex.2.html) system-call in Linux.
