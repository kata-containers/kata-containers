# AwaitGroup

[![Documentation](https://img.shields.io/badge/docs-0.6.0-4d76ae?style=for-the-badge)](https://docs.rs/awaitgroup/0.6.0)
[![Version](https://img.shields.io/crates/v/awaitgroup?style=for-the-badge)](https://crates.io/crates/awaitgroup)
[![License](https://img.shields.io/crates/l/awaitgroup?style=for-the-badge)](https://crates.io/crates/awaitgroup)
[![Actions](https://img.shields.io/github/workflow/status/ibraheemdev/awaitgroup/Rust/master?style=for-the-badge)](https://github.com/ibraheemdev/awaitgroup/actions)

 An asynchronous implementation of a `WaitGroup`.

 A `WaitGroup` waits for a collection of tasks to finish. The main task can create new workers and
 pass them to each of the tasks it wants to wait for. Then, each of the tasks calls `done` when
 it finishes executing. The main task can call `wait` to block until all registered workers are done.

 ```rust
 use awaitgroup::WaitGroup;

 #[tokio::main]
 async fn main() {
    let mut wg = WaitGroup::new();

    for _ in 0..5 {
        // Create a new worker.
        let worker = wg.worker();

        tokio::spawn(async {
            // Do some work...

            // This task is done all of its work.
            worker.done();
        });
    }

    // Block until all other tasks have finished their work.
    wg.wait().await;
}
 ```
See [the documentation](https://docs.rs/awaitgroup) for more details.
