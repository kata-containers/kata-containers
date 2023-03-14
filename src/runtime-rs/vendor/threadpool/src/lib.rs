// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A thread pool used to execute functions in parallel.
//!
//! Spawns a specified number of worker threads and replenishes the pool if any worker threads
//! panic.
//!
//! # Examples
//!
//! ## Synchronized with a channel
//!
//! Every thread sends one message over the channel, which then is collected with the `take()`.
//!
//! ```
//! use threadpool::ThreadPool;
//! use std::sync::mpsc::channel;
//!
//! let n_workers = 4;
//! let n_jobs = 8;
//! let pool = ThreadPool::new(n_workers);
//!
//! let (tx, rx) = channel();
//! for _ in 0..n_jobs {
//!     let tx = tx.clone();
//!     pool.execute(move|| {
//!         tx.send(1).expect("channel will be there waiting for the pool");
//!     });
//! }
//!
//! assert_eq!(rx.iter().take(n_jobs).fold(0, |a, b| a + b), 8);
//! ```
//!
//! ## Synchronized with a barrier
//!
//! Keep in mind, if a barrier synchronizes more jobs than you have workers in the pool,
//! you will end up with a [deadlock](https://en.wikipedia.org/wiki/Deadlock)
//! at the barrier which is [not considered unsafe](
//! https://doc.rust-lang.org/reference/behavior-not-considered-unsafe.html).
//!
//! ```
//! use threadpool::ThreadPool;
//! use std::sync::{Arc, Barrier};
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! // create at least as many workers as jobs or you will deadlock yourself
//! let n_workers = 42;
//! let n_jobs = 23;
//! let pool = ThreadPool::new(n_workers);
//! let an_atomic = Arc::new(AtomicUsize::new(0));
//!
//! assert!(n_jobs <= n_workers, "too many jobs, will deadlock");
//!
//! // create a barrier that waits for all jobs plus the starter thread
//! let barrier = Arc::new(Barrier::new(n_jobs + 1));
//! for _ in 0..n_jobs {
//!     let barrier = barrier.clone();
//!     let an_atomic = an_atomic.clone();
//!
//!     pool.execute(move|| {
//!         // do the heavy work
//!         an_atomic.fetch_add(1, Ordering::Relaxed);
//!
//!         // then wait for the other threads
//!         barrier.wait();
//!     });
//! }
//!
//! // wait for the threads to finish the work
//! barrier.wait();
//! assert_eq!(an_atomic.load(Ordering::SeqCst), /* n_jobs = */ 23);
//! ```

extern crate num_cpus;

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Thunk<'a> = Box<FnBox + Send + 'a>;

struct Sentinel<'a> {
    shared_data: &'a Arc<ThreadPoolSharedData>,
    active: bool,
}

impl<'a> Sentinel<'a> {
    fn new(shared_data: &'a Arc<ThreadPoolSharedData>) -> Sentinel<'a> {
        Sentinel {
            shared_data: shared_data,
            active: true,
        }
    }

    /// Cancel and destroy this sentinel.
    fn cancel(mut self) {
        self.active = false;
    }
}

impl<'a> Drop for Sentinel<'a> {
    fn drop(&mut self) {
        if self.active {
            self.shared_data.active_count.fetch_sub(1, Ordering::SeqCst);
            if thread::panicking() {
                self.shared_data.panic_count.fetch_add(1, Ordering::SeqCst);
            }
            self.shared_data.no_work_notify_all();
            spawn_in_pool(self.shared_data.clone())
        }
    }
}

/// [`ThreadPool`] factory, which can be used in order to configure the properties of the
/// [`ThreadPool`].
///
/// The three configuration options available:
///
/// * `num_threads`: maximum number of threads that will be alive at any given moment by the built
///   [`ThreadPool`]
/// * `thread_name`: thread name for each of the threads spawned by the built [`ThreadPool`]
/// * `thread_stack_size`: stack size (in bytes) for each of the threads spawned by the built
///   [`ThreadPool`]
///
/// [`ThreadPool`]: struct.ThreadPool.html
///
/// # Examples
///
/// Build a [`ThreadPool`] that uses a maximum of eight threads simultaneously and each thread has
/// a 8 MB stack size:
///
/// ```
/// let pool = threadpool::Builder::new()
///     .num_threads(8)
///     .thread_stack_size(8_000_000)
///     .build();
/// ```
#[derive(Clone, Default)]
pub struct Builder {
    num_threads: Option<usize>,
    thread_name: Option<String>,
    thread_stack_size: Option<usize>,
}

impl Builder {
    /// Initiate a new [`Builder`].
    ///
    /// [`Builder`]: struct.Builder.html
    ///
    /// # Examples
    ///
    /// ```
    /// let builder = threadpool::Builder::new();
    /// ```
    pub fn new() -> Builder {
        Builder {
            num_threads: None,
            thread_name: None,
            thread_stack_size: None,
        }
    }

    /// Set the maximum number of worker-threads that will be alive at any given moment by the built
    /// [`ThreadPool`]. If not specified, defaults the number of threads to the number of CPUs.
    ///
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Panics
    ///
    /// This method will panic if `num_threads` is 0.
    ///
    /// # Examples
    ///
    /// No more than eight threads will be alive simultaneously for this pool:
    ///
    /// ```
    /// use std::thread;
    ///
    /// let pool = threadpool::Builder::new()
    ///     .num_threads(8)
    ///     .build();
    ///
    /// for _ in 0..100 {
    ///     pool.execute(|| {
    ///         println!("Hello from a worker thread!")
    ///     })
    /// }
    /// ```
    pub fn num_threads(mut self, num_threads: usize) -> Builder {
        assert!(num_threads > 0);
        self.num_threads = Some(num_threads);
        self
    }

    /// Set the thread name for each of the threads spawned by the built [`ThreadPool`]. If not
    /// specified, threads spawned by the thread pool will be unnamed.
    ///
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Examples
    ///
    /// Each thread spawned by this pool will have the name "foo":
    ///
    /// ```
    /// use std::thread;
    ///
    /// let pool = threadpool::Builder::new()
    ///     .thread_name("foo".into())
    ///     .build();
    ///
    /// for _ in 0..100 {
    ///     pool.execute(|| {
    ///         assert_eq!(thread::current().name(), Some("foo"));
    ///     })
    /// }
    /// ```
    pub fn thread_name(mut self, name: String) -> Builder {
        self.thread_name = Some(name);
        self
    }

    /// Set the stack size (in bytes) for each of the threads spawned by the built [`ThreadPool`].
    /// If not specified, threads spawned by the threadpool will have a stack size [as specified in
    /// the `std::thread` documentation][thread].
    ///
    /// [thread]: https://doc.rust-lang.org/nightly/std/thread/index.html#stack-size
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Examples
    ///
    /// Each thread spawned by this pool will have a 4 MB stack:
    ///
    /// ```
    /// let pool = threadpool::Builder::new()
    ///     .thread_stack_size(4_000_000)
    ///     .build();
    ///
    /// for _ in 0..100 {
    ///     pool.execute(|| {
    ///         println!("This thread has a 4 MB stack size!");
    ///     })
    /// }
    /// ```
    pub fn thread_stack_size(mut self, size: usize) -> Builder {
        self.thread_stack_size = Some(size);
        self
    }

    /// Finalize the [`Builder`] and build the [`ThreadPool`].
    ///
    /// [`Builder`]: struct.Builder.html
    /// [`ThreadPool`]: struct.ThreadPool.html
    ///
    /// # Examples
    ///
    /// ```
    /// let pool = threadpool::Builder::new()
    ///     .num_threads(8)
    ///     .thread_stack_size(4_000_000)
    ///     .build();
    /// ```
    pub fn build(self) -> ThreadPool {
        let (tx, rx) = channel::<Thunk<'static>>();

        let num_threads = self.num_threads.unwrap_or_else(num_cpus::get);

        let shared_data = Arc::new(ThreadPoolSharedData {
            name: self.thread_name,
            job_receiver: Mutex::new(rx),
            empty_condvar: Condvar::new(),
            empty_trigger: Mutex::new(()),
            join_generation: AtomicUsize::new(0),
            queued_count: AtomicUsize::new(0),
            active_count: AtomicUsize::new(0),
            max_thread_count: AtomicUsize::new(num_threads),
            panic_count: AtomicUsize::new(0),
            stack_size: self.thread_stack_size,
        });

        // Threadpool threads
        for _ in 0..num_threads {
            spawn_in_pool(shared_data.clone());
        }

        ThreadPool {
            jobs: tx,
            shared_data: shared_data,
        }
    }
}

struct ThreadPoolSharedData {
    name: Option<String>,
    job_receiver: Mutex<Receiver<Thunk<'static>>>,
    empty_trigger: Mutex<()>,
    empty_condvar: Condvar,
    join_generation: AtomicUsize,
    queued_count: AtomicUsize,
    active_count: AtomicUsize,
    max_thread_count: AtomicUsize,
    panic_count: AtomicUsize,
    stack_size: Option<usize>,
}

impl ThreadPoolSharedData {
    fn has_work(&self) -> bool {
        self.queued_count.load(Ordering::SeqCst) > 0 || self.active_count.load(Ordering::SeqCst) > 0
    }

    /// Notify all observers joining this pool if there is no more work to do.
    fn no_work_notify_all(&self) {
        if !self.has_work() {
            *self
                .empty_trigger
                .lock()
                .expect("Unable to notify all joining threads");
            self.empty_condvar.notify_all();
        }
    }
}

/// Abstraction of a thread pool for basic parallelism.
pub struct ThreadPool {
    // How the threadpool communicates with subthreads.
    //
    // This is the only such Sender, so when it is dropped all subthreads will
    // quit.
    jobs: Sender<Thunk<'static>>,
    shared_data: Arc<ThreadPoolSharedData>,
}

impl ThreadPool {
    /// Creates a new thread pool capable of executing `num_threads` number of jobs concurrently.
    ///
    /// # Panics
    ///
    /// This function will panic if `num_threads` is 0.
    ///
    /// # Examples
    ///
    /// Create a new thread pool capable of executing four jobs concurrently:
    ///
    /// ```
    /// use threadpool::ThreadPool;
    ///
    /// let pool = ThreadPool::new(4);
    /// ```
    pub fn new(num_threads: usize) -> ThreadPool {
        Builder::new().num_threads(num_threads).build()
    }

    /// Creates a new thread pool capable of executing `num_threads` number of jobs concurrently.
    /// Each thread will have the [name][thread name] `name`.
    ///
    /// # Panics
    ///
    /// This function will panic if `num_threads` is 0.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::thread;
    /// use threadpool::ThreadPool;
    ///
    /// let pool = ThreadPool::with_name("worker".into(), 2);
    /// for _ in 0..2 {
    ///     pool.execute(|| {
    ///         assert_eq!(
    ///             thread::current().name(),
    ///             Some("worker")
    ///         );
    ///     });
    /// }
    /// pool.join();
    /// ```
    ///
    /// [thread name]: https://doc.rust-lang.org/std/thread/struct.Thread.html#method.name
    pub fn with_name(name: String, num_threads: usize) -> ThreadPool {
        Builder::new()
            .num_threads(num_threads)
            .thread_name(name)
            .build()
    }

    /// **Deprecated: Use [`ThreadPool::with_name`](#method.with_name)**
    #[inline(always)]
    #[deprecated(since = "1.4.0", note = "use ThreadPool::with_name")]
    pub fn new_with_name(name: String, num_threads: usize) -> ThreadPool {
        Self::with_name(name, num_threads)
    }

    /// Executes the function `job` on a thread in the pool.
    ///
    /// # Examples
    ///
    /// Execute four jobs on a thread pool that can run two jobs concurrently:
    ///
    /// ```
    /// use threadpool::ThreadPool;
    ///
    /// let pool = ThreadPool::new(2);
    /// pool.execute(|| println!("hello"));
    /// pool.execute(|| println!("world"));
    /// pool.execute(|| println!("foo"));
    /// pool.execute(|| println!("bar"));
    /// pool.join();
    /// ```
    pub fn execute<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.shared_data.queued_count.fetch_add(1, Ordering::SeqCst);
        self.jobs
            .send(Box::new(job))
            .expect("ThreadPool::execute unable to send job into queue.");
    }

    /// Returns the number of jobs waiting to executed in the pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let pool = ThreadPool::new(2);
    /// for _ in 0..10 {
    ///     pool.execute(|| {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// }
    ///
    /// sleep(Duration::from_secs(1)); // wait for threads to start
    /// assert_eq!(8, pool.queued_count());
    /// ```
    pub fn queued_count(&self) -> usize {
        self.shared_data.queued_count.load(Ordering::Relaxed)
    }

    /// Returns the number of currently active threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let pool = ThreadPool::new(4);
    /// for _ in 0..10 {
    ///     pool.execute(move || {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// }
    ///
    /// sleep(Duration::from_secs(1)); // wait for threads to start
    /// assert_eq!(4, pool.active_count());
    /// ```
    pub fn active_count(&self) -> usize {
        self.shared_data.active_count.load(Ordering::SeqCst)
    }

    /// Returns the maximum number of threads the pool will execute concurrently.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    ///
    /// let mut pool = ThreadPool::new(4);
    /// assert_eq!(4, pool.max_count());
    ///
    /// pool.set_num_threads(8);
    /// assert_eq!(8, pool.max_count());
    /// ```
    pub fn max_count(&self) -> usize {
        self.shared_data.max_thread_count.load(Ordering::Relaxed)
    }

    /// Returns the number of panicked threads over the lifetime of the pool.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    ///
    /// let pool = ThreadPool::new(4);
    /// for n in 0..10 {
    ///     pool.execute(move || {
    ///         // simulate a panic
    ///         if n % 2 == 0 {
    ///             panic!()
    ///         }
    ///     });
    /// }
    /// pool.join();
    ///
    /// assert_eq!(5, pool.panic_count());
    /// ```
    pub fn panic_count(&self) -> usize {
        self.shared_data.panic_count.load(Ordering::Relaxed)
    }

    /// **Deprecated: Use [`ThreadPool::set_num_threads`](#method.set_num_threads)**
    #[deprecated(since = "1.3.0", note = "use ThreadPool::set_num_threads")]
    pub fn set_threads(&mut self, num_threads: usize) {
        self.set_num_threads(num_threads)
    }

    /// Sets the number of worker-threads to use as `num_threads`.
    /// Can be used to change the threadpool size during runtime.
    /// Will not abort already running or waiting threads.
    ///
    /// # Panics
    ///
    /// This function will panic if `num_threads` is 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    /// use std::time::Duration;
    /// use std::thread::sleep;
    ///
    /// let mut pool = ThreadPool::new(4);
    /// for _ in 0..10 {
    ///     pool.execute(move || {
    ///         sleep(Duration::from_secs(100));
    ///     });
    /// }
    ///
    /// sleep(Duration::from_secs(1)); // wait for threads to start
    /// assert_eq!(4, pool.active_count());
    /// assert_eq!(6, pool.queued_count());
    ///
    /// // Increase thread capacity of the pool
    /// pool.set_num_threads(8);
    ///
    /// sleep(Duration::from_secs(1)); // wait for new threads to start
    /// assert_eq!(8, pool.active_count());
    /// assert_eq!(2, pool.queued_count());
    ///
    /// // Decrease thread capacity of the pool
    /// // No active threads are killed
    /// pool.set_num_threads(4);
    ///
    /// assert_eq!(8, pool.active_count());
    /// assert_eq!(2, pool.queued_count());
    /// ```
    pub fn set_num_threads(&mut self, num_threads: usize) {
        assert!(num_threads >= 1);
        let prev_num_threads = self
            .shared_data
            .max_thread_count
            .swap(num_threads, Ordering::Release);
        if let Some(num_spawn) = num_threads.checked_sub(prev_num_threads) {
            // Spawn new threads
            for _ in 0..num_spawn {
                spawn_in_pool(self.shared_data.clone());
            }
        }
    }

    /// Block the current thread until all jobs in the pool have been executed.
    ///
    /// Calling `join` on an empty pool will cause an immediate return.
    /// `join` may be called from multiple threads concurrently.
    /// A `join` is an atomic point in time. All threads joining before the join
    /// event will exit together even if the pool is processing new jobs by the
    /// time they get scheduled.
    ///
    /// Calling `join` from a thread within the pool will cause a deadlock. This
    /// behavior is considered safe.
    ///
    /// # Examples
    ///
    /// ```
    /// use threadpool::ThreadPool;
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// let pool = ThreadPool::new(8);
    /// let test_count = Arc::new(AtomicUsize::new(0));
    ///
    /// for _ in 0..42 {
    ///     let test_count = test_count.clone();
    ///     pool.execute(move || {
    ///         test_count.fetch_add(1, Ordering::Relaxed);
    ///     });
    /// }
    ///
    /// pool.join();
    /// assert_eq!(42, test_count.load(Ordering::Relaxed));
    /// ```
    pub fn join(&self) {
        // fast path requires no mutex
        if self.shared_data.has_work() == false {
            return ();
        }

        let generation = self.shared_data.join_generation.load(Ordering::SeqCst);
        let mut lock = self.shared_data.empty_trigger.lock().unwrap();

        while generation == self.shared_data.join_generation.load(Ordering::Relaxed)
            && self.shared_data.has_work()
        {
            lock = self.shared_data.empty_condvar.wait(lock).unwrap();
        }

        // increase generation if we are the first thread to come out of the loop
        self.shared_data.join_generation.compare_and_swap(
            generation,
            generation.wrapping_add(1),
            Ordering::SeqCst,
        );
    }
}

impl Clone for ThreadPool {
    /// Cloning a pool will create a new handle to the pool.
    /// The behavior is similar to [Arc](https://doc.rust-lang.org/stable/std/sync/struct.Arc.html).
    ///
    /// We could for example submit jobs from multiple threads concurrently.
    ///
    /// ```
    /// use threadpool::ThreadPool;
    /// use std::thread;
    /// use std::sync::mpsc::channel;
    ///
    /// let pool = ThreadPool::with_name("clone example".into(), 2);
    ///
    /// let results = (0..2)
    ///     .map(|i| {
    ///         let pool = pool.clone();
    ///         thread::spawn(move || {
    ///             let (tx, rx) = channel();
    ///             for i in 1..12 {
    ///                 let tx = tx.clone();
    ///                 pool.execute(move || {
    ///                     tx.send(i).expect("channel will be waiting");
    ///                 });
    ///             }
    ///             drop(tx);
    ///             if i == 0 {
    ///                 rx.iter().fold(0, |accumulator, element| accumulator + element)
    ///             } else {
    ///                 rx.iter().fold(1, |accumulator, element| accumulator * element)
    ///             }
    ///         })
    ///     })
    ///     .map(|join_handle| join_handle.join().expect("collect results from threads"))
    ///     .collect::<Vec<usize>>();
    ///
    /// assert_eq!(vec![66, 39916800], results);
    /// ```
    fn clone(&self) -> ThreadPool {
        ThreadPool {
            jobs: self.jobs.clone(),
            shared_data: self.shared_data.clone(),
        }
    }
}

/// Create a thread pool with one thread per CPU.
/// On machines with hyperthreading,
/// this will create one thread per hyperthread.
impl Default for ThreadPool {
    fn default() -> Self {
        ThreadPool::new(num_cpus::get())
    }
}

impl fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ThreadPool")
            .field("name", &self.shared_data.name)
            .field("queued_count", &self.queued_count())
            .field("active_count", &self.active_count())
            .field("max_count", &self.max_count())
            .finish()
    }
}

impl PartialEq for ThreadPool {
    /// Check if you are working with the same pool
    ///
    /// ```
    /// use threadpool::ThreadPool;
    ///
    /// let a = ThreadPool::new(2);
    /// let b = ThreadPool::new(2);
    ///
    /// assert_eq!(a, a);
    /// assert_eq!(b, b);
    ///
    /// # // TODO: change this to assert_ne in the future
    /// assert!(a != b);
    /// assert!(b != a);
    /// ```
    fn eq(&self, other: &ThreadPool) -> bool {
        let a: &ThreadPoolSharedData = &*self.shared_data;
        let b: &ThreadPoolSharedData = &*other.shared_data;
        a as *const ThreadPoolSharedData == b as *const ThreadPoolSharedData
        // with rust 1.17 and late:
        // Arc::ptr_eq(&self.shared_data, &other.shared_data)
    }
}
impl Eq for ThreadPool {}

fn spawn_in_pool(shared_data: Arc<ThreadPoolSharedData>) {
    let mut builder = thread::Builder::new();
    if let Some(ref name) = shared_data.name {
        builder = builder.name(name.clone());
    }
    if let Some(ref stack_size) = shared_data.stack_size {
        builder = builder.stack_size(stack_size.to_owned());
    }
    builder
        .spawn(move || {
            // Will spawn a new thread on panic unless it is cancelled.
            let sentinel = Sentinel::new(&shared_data);

            loop {
                // Shutdown this thread if the pool has become smaller
                let thread_counter_val = shared_data.active_count.load(Ordering::Acquire);
                let max_thread_count_val = shared_data.max_thread_count.load(Ordering::Relaxed);
                if thread_counter_val >= max_thread_count_val {
                    break;
                }
                let message = {
                    // Only lock jobs for the time it takes
                    // to get a job, not run it.
                    let lock = shared_data
                        .job_receiver
                        .lock()
                        .expect("Worker thread unable to lock job_receiver");
                    lock.recv()
                };

                let job = match message {
                    Ok(job) => job,
                    // The ThreadPool was dropped.
                    Err(..) => break,
                };
                // Do not allow IR around the job execution
                shared_data.active_count.fetch_add(1, Ordering::SeqCst);
                shared_data.queued_count.fetch_sub(1, Ordering::SeqCst);

                job.call_box();

                shared_data.active_count.fetch_sub(1, Ordering::SeqCst);
                shared_data.no_work_notify_all();
            }

            sentinel.cancel();
        })
        .unwrap();
}

#[cfg(test)]
mod test {
    use super::{Builder, ThreadPool};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{channel, sync_channel};
    use std::sync::{Arc, Barrier};
    use std::thread::{self, sleep};
    use std::time::Duration;

    const TEST_TASKS: usize = 4;

    #[test]
    fn test_set_num_threads_increasing() {
        let new_thread_amount = TEST_TASKS + 8;
        let mut pool = ThreadPool::new(TEST_TASKS);
        for _ in 0..TEST_TASKS {
            pool.execute(move || sleep(Duration::from_secs(23)));
        }
        sleep(Duration::from_secs(1));
        assert_eq!(pool.active_count(), TEST_TASKS);

        pool.set_num_threads(new_thread_amount);

        for _ in 0..(new_thread_amount - TEST_TASKS) {
            pool.execute(move || sleep(Duration::from_secs(23)));
        }
        sleep(Duration::from_secs(1));
        assert_eq!(pool.active_count(), new_thread_amount);

        pool.join();
    }

    #[test]
    fn test_set_num_threads_decreasing() {
        let new_thread_amount = 2;
        let mut pool = ThreadPool::new(TEST_TASKS);
        for _ in 0..TEST_TASKS {
            pool.execute(move || {
                assert_eq!(1, 1);
            });
        }
        pool.set_num_threads(new_thread_amount);
        for _ in 0..new_thread_amount {
            pool.execute(move || sleep(Duration::from_secs(23)));
        }
        sleep(Duration::from_secs(1));
        assert_eq!(pool.active_count(), new_thread_amount);

        pool.join();
    }

    #[test]
    fn test_active_count() {
        let pool = ThreadPool::new(TEST_TASKS);
        for _ in 0..2 * TEST_TASKS {
            pool.execute(move || loop {
                sleep(Duration::from_secs(10))
            });
        }
        sleep(Duration::from_secs(1));
        let active_count = pool.active_count();
        assert_eq!(active_count, TEST_TASKS);
        let initialized_count = pool.max_count();
        assert_eq!(initialized_count, TEST_TASKS);
    }

    #[test]
    fn test_works() {
        let pool = ThreadPool::new(TEST_TASKS);

        let (tx, rx) = channel();
        for _ in 0..TEST_TASKS {
            let tx = tx.clone();
            pool.execute(move || {
                tx.send(1).unwrap();
            });
        }

        assert_eq!(rx.iter().take(TEST_TASKS).fold(0, |a, b| a + b), TEST_TASKS);
    }

    #[test]
    #[should_panic]
    fn test_zero_tasks_panic() {
        ThreadPool::new(0);
    }

    #[test]
    fn test_recovery_from_subtask_panic() {
        let pool = ThreadPool::new(TEST_TASKS);

        // Panic all the existing threads.
        for _ in 0..TEST_TASKS {
            pool.execute(move || panic!("Ignore this panic, it must!"));
        }
        pool.join();

        assert_eq!(pool.panic_count(), TEST_TASKS);

        // Ensure new threads were spawned to compensate.
        let (tx, rx) = channel();
        for _ in 0..TEST_TASKS {
            let tx = tx.clone();
            pool.execute(move || {
                tx.send(1).unwrap();
            });
        }

        assert_eq!(rx.iter().take(TEST_TASKS).fold(0, |a, b| a + b), TEST_TASKS);
    }

    #[test]
    fn test_should_not_panic_on_drop_if_subtasks_panic_after_drop() {
        let pool = ThreadPool::new(TEST_TASKS);
        let waiter = Arc::new(Barrier::new(TEST_TASKS + 1));

        // Panic all the existing threads in a bit.
        for _ in 0..TEST_TASKS {
            let waiter = waiter.clone();
            pool.execute(move || {
                waiter.wait();
                panic!("Ignore this panic, it should!");
            });
        }

        drop(pool);

        // Kick off the failure.
        waiter.wait();
    }

    #[test]
    fn test_massive_task_creation() {
        let test_tasks = 4_200_000;

        let pool = ThreadPool::new(TEST_TASKS);
        let b0 = Arc::new(Barrier::new(TEST_TASKS + 1));
        let b1 = Arc::new(Barrier::new(TEST_TASKS + 1));

        let (tx, rx) = channel();

        for i in 0..test_tasks {
            let tx = tx.clone();
            let (b0, b1) = (b0.clone(), b1.clone());

            pool.execute(move || {
                // Wait until the pool has been filled once.
                if i < TEST_TASKS {
                    b0.wait();
                    // wait so the pool can be measured
                    b1.wait();
                }

                tx.send(1).is_ok();
            });
        }

        b0.wait();
        assert_eq!(pool.active_count(), TEST_TASKS);
        b1.wait();

        assert_eq!(rx.iter().take(test_tasks).fold(0, |a, b| a + b), test_tasks);
        pool.join();

        let atomic_active_count = pool.active_count();
        assert!(
            atomic_active_count == 0,
            "atomic_active_count: {}",
            atomic_active_count
        );
    }

    #[test]
    fn test_shrink() {
        let test_tasks_begin = TEST_TASKS + 2;

        let mut pool = ThreadPool::new(test_tasks_begin);
        let b0 = Arc::new(Barrier::new(test_tasks_begin + 1));
        let b1 = Arc::new(Barrier::new(test_tasks_begin + 1));

        for _ in 0..test_tasks_begin {
            let (b0, b1) = (b0.clone(), b1.clone());
            pool.execute(move || {
                b0.wait();
                b1.wait();
            });
        }

        let b2 = Arc::new(Barrier::new(TEST_TASKS + 1));
        let b3 = Arc::new(Barrier::new(TEST_TASKS + 1));

        for _ in 0..TEST_TASKS {
            let (b2, b3) = (b2.clone(), b3.clone());
            pool.execute(move || {
                b2.wait();
                b3.wait();
            });
        }

        b0.wait();
        pool.set_num_threads(TEST_TASKS);

        assert_eq!(pool.active_count(), test_tasks_begin);
        b1.wait();

        b2.wait();
        assert_eq!(pool.active_count(), TEST_TASKS);
        b3.wait();
    }

    #[test]
    fn test_name() {
        let name = "test";
        let mut pool = ThreadPool::with_name(name.to_owned(), 2);
        let (tx, rx) = sync_channel(0);

        // initial thread should share the name "test"
        for _ in 0..2 {
            let tx = tx.clone();
            pool.execute(move || {
                let name = thread::current().name().unwrap().to_owned();
                tx.send(name).unwrap();
            });
        }

        // new spawn thread should share the name "test" too.
        pool.set_num_threads(3);
        let tx_clone = tx.clone();
        pool.execute(move || {
            let name = thread::current().name().unwrap().to_owned();
            tx_clone.send(name).unwrap();
            panic!();
        });

        // recover thread should share the name "test" too.
        pool.execute(move || {
            let name = thread::current().name().unwrap().to_owned();
            tx.send(name).unwrap();
        });

        for thread_name in rx.iter().take(4) {
            assert_eq!(name, thread_name);
        }
    }

    #[test]
    fn test_debug() {
        let pool = ThreadPool::new(4);
        let debug = format!("{:?}", pool);
        assert_eq!(
            debug,
            "ThreadPool { name: None, queued_count: 0, active_count: 0, max_count: 4 }"
        );

        let pool = ThreadPool::with_name("hello".into(), 4);
        let debug = format!("{:?}", pool);
        assert_eq!(
            debug,
            "ThreadPool { name: Some(\"hello\"), queued_count: 0, active_count: 0, max_count: 4 }"
        );

        let pool = ThreadPool::new(4);
        pool.execute(move || sleep(Duration::from_secs(5)));
        sleep(Duration::from_secs(1));
        let debug = format!("{:?}", pool);
        assert_eq!(
            debug,
            "ThreadPool { name: None, queued_count: 0, active_count: 1, max_count: 4 }"
        );
    }

    #[test]
    fn test_repeate_join() {
        let pool = ThreadPool::with_name("repeate join test".into(), 8);
        let test_count = Arc::new(AtomicUsize::new(0));

        for _ in 0..42 {
            let test_count = test_count.clone();
            pool.execute(move || {
                sleep(Duration::from_secs(2));
                test_count.fetch_add(1, Ordering::Release);
            });
        }

        println!("{:?}", pool);
        pool.join();
        assert_eq!(42, test_count.load(Ordering::Acquire));

        for _ in 0..42 {
            let test_count = test_count.clone();
            pool.execute(move || {
                sleep(Duration::from_secs(2));
                test_count.fetch_add(1, Ordering::Relaxed);
            });
        }
        pool.join();
        assert_eq!(84, test_count.load(Ordering::Relaxed));
    }

    #[test]
    fn test_multi_join() {
        use std::sync::mpsc::TryRecvError::*;

        // Toggle the following lines to debug the deadlock
        fn error(_s: String) {
            //use ::std::io::Write;
            //let stderr = ::std::io::stderr();
            //let mut stderr = stderr.lock();
            //stderr.write(&_s.as_bytes()).is_ok();
        }

        let pool0 = ThreadPool::with_name("multi join pool0".into(), 4);
        let pool1 = ThreadPool::with_name("multi join pool1".into(), 4);
        let (tx, rx) = channel();

        for i in 0..8 {
            let pool1 = pool1.clone();
            let pool0_ = pool0.clone();
            let tx = tx.clone();
            pool0.execute(move || {
                pool1.execute(move || {
                    error(format!("p1: {} -=- {:?}\n", i, pool0_));
                    pool0_.join();
                    error(format!("p1: send({})\n", i));
                    tx.send(i).expect("send i from pool1 -> main");
                });
                error(format!("p0: {}\n", i));
            });
        }
        drop(tx);

        assert_eq!(rx.try_recv(), Err(Empty));
        error(format!("{:?}\n{:?}\n", pool0, pool1));
        pool0.join();
        error(format!("pool0.join() complete =-= {:?}", pool1));
        pool1.join();
        error("pool1.join() complete\n".into());
        assert_eq!(
            rx.iter().fold(0, |acc, i| acc + i),
            0 + 1 + 2 + 3 + 4 + 5 + 6 + 7
        );
    }

    #[test]
    fn test_empty_pool() {
        // Joining an empty pool must return imminently
        let pool = ThreadPool::new(4);

        pool.join();

        assert!(true);
    }

    #[test]
    fn test_no_fun_or_joy() {
        // What happens when you keep adding jobs after a join

        fn sleepy_function() {
            sleep(Duration::from_secs(6));
        }

        let pool = ThreadPool::with_name("no fun or joy".into(), 8);

        pool.execute(sleepy_function);

        let p_t = pool.clone();
        thread::spawn(move || {
            (0..23).map(|_| p_t.execute(sleepy_function)).count();
        });

        pool.join();
    }

    #[test]
    fn test_clone() {
        let pool = ThreadPool::with_name("clone example".into(), 2);

        // This batch of jobs will occupy the pool for some time
        for _ in 0..6 {
            pool.execute(move || {
                sleep(Duration::from_secs(2));
            });
        }

        // The following jobs will be inserted into the pool in a random fashion
        let t0 = {
            let pool = pool.clone();
            thread::spawn(move || {
                // wait for the first batch of tasks to finish
                pool.join();

                let (tx, rx) = channel();
                for i in 0..42 {
                    let tx = tx.clone();
                    pool.execute(move || {
                        tx.send(i).expect("channel will be waiting");
                    });
                }
                drop(tx);
                rx.iter()
                    .fold(0, |accumulator, element| accumulator + element)
            })
        };
        let t1 = {
            let pool = pool.clone();
            thread::spawn(move || {
                // wait for the first batch of tasks to finish
                pool.join();

                let (tx, rx) = channel();
                for i in 1..12 {
                    let tx = tx.clone();
                    pool.execute(move || {
                        tx.send(i).expect("channel will be waiting");
                    });
                }
                drop(tx);
                rx.iter()
                    .fold(1, |accumulator, element| accumulator * element)
            })
        };

        assert_eq!(
            861,
            t0.join()
                .expect("thread 0 will return after calculating additions",)
        );
        assert_eq!(
            39916800,
            t1.join()
                .expect("thread 1 will return after calculating multiplications",)
        );
    }

    #[test]
    fn test_sync_shared_data() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<super::ThreadPoolSharedData>();
    }

    #[test]
    fn test_send_shared_data() {
        fn assert_send<T: Send>() {}
        assert_send::<super::ThreadPoolSharedData>();
    }

    #[test]
    fn test_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ThreadPool>();
    }

    #[test]
    fn test_cloned_eq() {
        let a = ThreadPool::new(2);

        assert_eq!(a, a.clone());
    }

    #[test]
    /// The scenario is joining threads should not be stuck once their wave
    /// of joins has completed. So once one thread joining on a pool has
    /// succeded other threads joining on the same pool must get out even if
    /// the thread is used for other jobs while the first group is finishing
    /// their join
    ///
    /// In this example this means the waiting threads will exit the join in
    /// groups of four because the waiter pool has four workers.
    fn test_join_wavesurfer() {
        let n_cycles = 4;
        let n_workers = 4;
        let (tx, rx) = channel();
        let builder = Builder::new()
            .num_threads(n_workers)
            .thread_name("join wavesurfer".into());
        let p_waiter = builder.clone().build();
        let p_clock = builder.build();

        let barrier = Arc::new(Barrier::new(3));
        let wave_clock = Arc::new(AtomicUsize::new(0));
        let clock_thread = {
            let barrier = barrier.clone();
            let wave_clock = wave_clock.clone();
            thread::spawn(move || {
                barrier.wait();
                for wave_num in 0..n_cycles {
                    wave_clock.store(wave_num, Ordering::SeqCst);
                    sleep(Duration::from_secs(1));
                }
            })
        };

        {
            let barrier = barrier.clone();
            p_clock.execute(move || {
                barrier.wait();
                // this sleep is for stabilisation on weaker platforms
                sleep(Duration::from_millis(100));
            });
        }

        // prepare three waves of jobs
        for i in 0..3 * n_workers {
            let p_clock = p_clock.clone();
            let tx = tx.clone();
            let wave_clock = wave_clock.clone();
            p_waiter.execute(move || {
                let now = wave_clock.load(Ordering::SeqCst);
                p_clock.join();
                // submit jobs for the second wave
                p_clock.execute(|| sleep(Duration::from_secs(1)));
                let clock = wave_clock.load(Ordering::SeqCst);
                tx.send((now, clock, i)).unwrap();
            });
        }
        println!("all scheduled at {}", wave_clock.load(Ordering::SeqCst));
        barrier.wait();

        p_clock.join();
        //p_waiter.join();

        drop(tx);
        let mut hist = vec![0; n_cycles];
        let mut data = vec![];
        for (now, after, i) in rx.iter() {
            let mut dur = after - now;
            if dur >= n_cycles - 1 {
                dur = n_cycles - 1;
            }
            hist[dur] += 1;

            data.push((now, after, i));
        }
        for (i, n) in hist.iter().enumerate() {
            println!(
                "\t{}: {} {}",
                i,
                n,
                &*(0..*n).fold("".to_owned(), |s, _| s + "*")
            );
        }
        assert!(data.iter().all(|&(cycle, stop, i)| if i < n_workers {
            cycle == stop
        } else {
            cycle < stop
        }));

        clock_thread.join().unwrap();
    }
}
