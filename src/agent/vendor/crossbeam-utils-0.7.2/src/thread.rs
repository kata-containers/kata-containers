//! Threads that can borrow variables from the stack.
//!
//! Create a scope when spawned threads need to access variables on the stack:
//!
//! ```
//! use crossbeam_utils::thread;
//!
//! let people = vec![
//!     "Alice".to_string(),
//!     "Bob".to_string(),
//!     "Carol".to_string(),
//! ];
//!
//! thread::scope(|s| {
//!     for person in &people {
//!         s.spawn(move |_| {
//!             println!("Hello, {}!", person);
//!         });
//!     }
//! }).unwrap();
//! ```
//!
//! # Why scoped threads?
//!
//! Suppose we wanted to re-write the previous example using plain threads:
//!
//! ```ignore
//! use std::thread;
//!
//! let people = vec![
//!     "Alice".to_string(),
//!     "Bob".to_string(),
//!     "Carol".to_string(),
//! ];
//!
//! let mut threads = Vec::new();
//!
//! for person in &people {
//!     threads.push(thread::spawn(move |_| {
//!         println!("Hello, {}!", person);
//!     }));
//! }
//!
//! for thread in threads {
//!     thread.join().unwrap();
//! }
//! ```
//!
//! This doesn't work because the borrow checker complains about `people` not living long enough:
//!
//! ```text
//! error[E0597]: `people` does not live long enough
//!   --> src/main.rs:12:20
//!    |
//! 12 |     for person in &people {
//!    |                    ^^^^^^ borrowed value does not live long enough
//! ...
//! 21 | }
//!    | - borrowed value only lives until here
//!    |
//!    = note: borrowed value must be valid for the static lifetime...
//! ```
//!
//! The problem here is that spawned threads are not allowed to borrow variables on stack because
//! the compiler cannot prove they will be joined before `people` is destroyed.
//!
//! Scoped threads are a mechanism to guarantee to the compiler that spawned threads will be joined
//! before the scope ends.
//!
//! # How scoped threads work
//!
//! If a variable is borrowed by a thread, the thread must complete before the variable is
//! destroyed. Threads spawned using [`std::thread::spawn`] can only borrow variables with the
//! `'static` lifetime because the borrow checker cannot be sure when the thread will complete.
//!
//! A scope creates a clear boundary between variables outside the scope and threads inside the
//! scope. Whenever a scope spawns a thread, it promises to join the thread before the scope ends.
//! This way we guarantee to the borrow checker that scoped threads only live within the scope and
//! can safely access variables outside it.
//!
//! # Nesting scoped threads
//!
//! Sometimes scoped threads need to spawn more threads within the same scope. This is a little
//! tricky because argument `s` lives *inside* the invocation of `thread::scope()` and as such
//! cannot be borrowed by scoped threads:
//!
//! ```ignore
//! use crossbeam_utils::thread;
//!
//! thread::scope(|s| {
//!     s.spawn(|_| {
//!         // Not going to compile because we're trying to borrow `s`,
//!         // which lives *inside* the scope! :(
//!         s.spawn(|_| println!("nested thread"));
//!     });
//! });
//! ```
//!
//! Fortunately, there is a solution. Every scoped thread is passed a reference to its scope as an
//! argument, which can be used for spawning nested threads:
//!
//! ```
//! use crossbeam_utils::thread;
//!
//! thread::scope(|s| {
//!     // Note the `|s|` here.
//!     s.spawn(|s| {
//!         // Yay, this works because we're using a fresh argument `s`! :)
//!         s.spawn(|_| println!("nested thread"));
//!     });
//! });
//! ```
//!
//! [`std::thread::spawn`]: https://doc.rust-lang.org/std/thread/fn.spawn.html

use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::panic;
use std::sync::{Arc, Mutex};
use std::thread;

use sync::WaitGroup;

type SharedVec<T> = Arc<Mutex<Vec<T>>>;
type SharedOption<T> = Arc<Mutex<Option<T>>>;

/// Creates a new scope for spawning threads.
///
/// All child threads that haven't been manually joined will be automatically joined just before
/// this function invocation ends. If all joined threads have successfully completed, `Ok` is
/// returned with the return value of `f`. If any of the joined threads has panicked, an `Err` is
/// returned containing errors from panicked threads.
///
/// # Examples
///
/// ```
/// use crossbeam_utils::thread;
///
/// let var = vec![1, 2, 3];
///
/// thread::scope(|s| {
///     s.spawn(|_| {
///         println!("A child thread borrowing `var`: {:?}", var);
///     });
/// }).unwrap();
/// ```
pub fn scope<'env, F, R>(f: F) -> thread::Result<R>
where
    F: FnOnce(&Scope<'env>) -> R,
{
    let wg = WaitGroup::new();
    let scope = Scope::<'env> {
        handles: SharedVec::default(),
        wait_group: wg.clone(),
        _marker: PhantomData,
    };

    // Execute the scoped function, but catch any panics.
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| f(&scope)));

    // Wait until all nested scopes are dropped.
    drop(scope.wait_group);
    wg.wait();

    // Join all remaining spawned threads.
    let panics: Vec<_> = {
        let mut handles = scope.handles.lock().unwrap();

        // Filter handles that haven't been joined, join them, and collect errors.
        let panics = handles
            .drain(..)
            .filter_map(|handle| handle.lock().unwrap().take())
            .filter_map(|handle| handle.join().err())
            .collect();

        panics
    };

    // If `f` has panicked, resume unwinding.
    // If any of the child threads have panicked, return the panic errors.
    // Otherwise, everything is OK and return the result of `f`.
    match result {
        Err(err) => panic::resume_unwind(err),
        Ok(res) => {
            if panics.is_empty() {
                Ok(res)
            } else {
                Err(Box::new(panics))
            }
        }
    }
}

/// A scope for spawning threads.
pub struct Scope<'env> {
    /// The list of the thread join handles.
    handles: SharedVec<SharedOption<thread::JoinHandle<()>>>,

    /// Used to wait until all subscopes all dropped.
    wait_group: WaitGroup,

    /// Borrows data with invariant lifetime `'env`.
    _marker: PhantomData<&'env mut &'env ()>,
}

unsafe impl<'env> Sync for Scope<'env> {}

impl<'env> Scope<'env> {
    /// Spawns a scoped thread.
    ///
    /// This method is similar to the [`spawn`] function in Rust's standard library. The difference
    /// is that this thread is scoped, meaning it's guaranteed to terminate before the scope exits,
    /// allowing it to reference variables outside the scope.
    ///
    /// The scoped thread is passed a reference to this scope as an argument, which can be used for
    /// spawning nested threads.
    ///
    /// The returned handle can be used to manually join the thread before the scope exits.
    ///
    /// [`spawn`]: https://doc.rust-lang.org/std/thread/fn.spawn.html
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::thread;
    ///
    /// thread::scope(|s| {
    ///     let handle = s.spawn(|_| {
    ///         println!("A child thread is running");
    ///         42
    ///     });
    ///
    ///     // Join the thread and retrieve its result.
    ///     let res = handle.join().unwrap();
    ///     assert_eq!(res, 42);
    /// }).unwrap();
    /// ```
    pub fn spawn<'scope, F, T>(&'scope self, f: F) -> ScopedJoinHandle<'scope, T>
    where
        F: FnOnce(&Scope<'env>) -> T,
        F: Send + 'env,
        T: Send + 'env,
    {
        self.builder().spawn(f).unwrap()
    }

    /// Creates a builder that can configure a thread before spawning.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::thread;
    /// use std::thread::current;
    ///
    /// thread::scope(|s| {
    ///     s.builder()
    ///         .spawn(|_| println!("A child thread is running"))
    ///         .unwrap();
    /// }).unwrap();
    /// ```
    pub fn builder<'scope>(&'scope self) -> ScopedThreadBuilder<'scope, 'env> {
        ScopedThreadBuilder {
            scope: self,
            builder: thread::Builder::new(),
        }
    }
}

impl<'env> fmt::Debug for Scope<'env> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Scope { .. }")
    }
}

/// Configures the properties of a new thread.
///
/// The two configurable properties are:
///
/// - [`name`]: Specifies an [associated name for the thread][naming-threads].
/// - [`stack_size`]: Specifies the [desired stack size for the thread][stack-size].
///
/// The [`spawn`] method will take ownership of the builder and return an [`io::Result`] of the
/// thread handle with the given configuration.
///
/// The [`Scope::spawn`] method uses a builder with default configuration and unwraps its return
/// value. You may want to use this builder when you want to recover from a failure to launch a
/// thread.
///
/// # Examples
///
/// ```
/// use crossbeam_utils::thread;
///
/// thread::scope(|s| {
///     s.builder()
///         .spawn(|_| println!("Running a child thread"))
///         .unwrap();
/// }).unwrap();
/// ```
///
/// [`name`]: struct.ScopedThreadBuilder.html#method.name
/// [`stack_size`]: struct.ScopedThreadBuilder.html#method.stack_size
/// [`spawn`]: struct.ScopedThreadBuilder.html#method.spawn
/// [`Scope::spawn`]: struct.Scope.html#method.spawn
/// [`io::Result`]: https://doc.rust-lang.org/std/io/type.Result.html
/// [naming-threads]: https://doc.rust-lang.org/std/thread/index.html#naming-threads
/// [stack-size]: https://doc.rust-lang.org/std/thread/index.html#stack-size
#[derive(Debug)]
pub struct ScopedThreadBuilder<'scope, 'env: 'scope> {
    scope: &'scope Scope<'env>,
    builder: thread::Builder,
}

impl<'scope, 'env> ScopedThreadBuilder<'scope, 'env> {
    /// Sets the name for the new thread.
    ///
    /// The name must not contain null bytes. For more information about named threads, see
    /// [here][naming-threads].
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::thread;
    /// use std::thread::current;
    ///
    /// thread::scope(|s| {
    ///     s.builder()
    ///         .name("my thread".to_string())
    ///         .spawn(|_| assert_eq!(current().name(), Some("my thread")))
    ///         .unwrap();
    /// }).unwrap();
    /// ```
    ///
    /// [naming-threads]: https://doc.rust-lang.org/std/thread/index.html#naming-threads
    pub fn name(mut self, name: String) -> ScopedThreadBuilder<'scope, 'env> {
        self.builder = self.builder.name(name);
        self
    }

    /// Sets the size of the stack for the new thread.
    ///
    /// The stack size is measured in bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::thread;
    ///
    /// thread::scope(|s| {
    ///     s.builder()
    ///         .stack_size(32 * 1024)
    ///         .spawn(|_| println!("Running a child thread"))
    ///         .unwrap();
    /// }).unwrap();
    /// ```
    pub fn stack_size(mut self, size: usize) -> ScopedThreadBuilder<'scope, 'env> {
        self.builder = self.builder.stack_size(size);
        self
    }

    /// Spawns a scoped thread with this configuration.
    ///
    /// The scoped thread is passed a reference to this scope as an argument, which can be used for
    /// spawning nested threads.
    ///
    /// The returned handle can be used to manually join the thread before the scope exits.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::thread;
    ///
    /// thread::scope(|s| {
    ///     let handle = s.builder()
    ///         .spawn(|_| {
    ///             println!("A child thread is running");
    ///             42
    ///         })
    ///         .unwrap();
    ///
    ///     // Join the thread and retrieve its result.
    ///     let res = handle.join().unwrap();
    ///     assert_eq!(res, 42);
    /// }).unwrap();
    /// ```
    pub fn spawn<F, T>(self, f: F) -> io::Result<ScopedJoinHandle<'scope, T>>
    where
        F: FnOnce(&Scope<'env>) -> T,
        F: Send + 'env,
        T: Send + 'env,
    {
        // The result of `f` will be stored here.
        let result = SharedOption::default();

        // Spawn the thread and grab its join handle and thread handle.
        let (handle, thread) = {
            let result = Arc::clone(&result);

            // A clone of the scope that will be moved into the new thread.
            let scope = Scope::<'env> {
                handles: Arc::clone(&self.scope.handles),
                wait_group: self.scope.wait_group.clone(),
                _marker: PhantomData,
            };

            // Spawn the thread.
            let handle = {
                let closure = move || {
                    // Make sure the scope is inside the closure with the proper `'env` lifetime.
                    let scope: Scope<'env> = scope;

                    // Run the closure.
                    let res = f(&scope);

                    // Store the result if the closure didn't panic.
                    *result.lock().unwrap() = Some(res);
                };

                // Change the type of `closure` from `FnOnce() -> T` to `FnMut() -> T`.
                let mut closure = Some(closure);
                let closure = move || closure.take().unwrap()();

                // Allocate `clsoure` on the heap and erase the `'env` bound.
                let closure: Box<dyn FnMut() + Send + 'env> = Box::new(closure);
                let closure: Box<dyn FnMut() + Send + 'static> = unsafe { mem::transmute(closure) };

                // Finally, spawn the closure.
                let mut closure = closure;
                self.builder.spawn(move || closure())?
            };

            let thread = handle.thread().clone();
            let handle = Arc::new(Mutex::new(Some(handle)));
            (handle, thread)
        };

        // Add the handle to the shared list of join handles.
        self.scope.handles.lock().unwrap().push(Arc::clone(&handle));

        Ok(ScopedJoinHandle {
            handle,
            result,
            thread,
            _marker: PhantomData,
        })
    }
}

unsafe impl<'scope, T> Send for ScopedJoinHandle<'scope, T> {}
unsafe impl<'scope, T> Sync for ScopedJoinHandle<'scope, T> {}

/// A handle that can be used to join its scoped thread.
pub struct ScopedJoinHandle<'scope, T> {
    /// A join handle to the spawned thread.
    handle: SharedOption<thread::JoinHandle<()>>,

    /// Holds the result of the inner closure.
    result: SharedOption<T>,

    /// A handle to the the spawned thread.
    thread: thread::Thread,

    /// Borrows the parent scope with lifetime `'scope`.
    _marker: PhantomData<&'scope ()>,
}

impl<'scope, T> ScopedJoinHandle<'scope, T> {
    /// Waits for the thread to finish and returns its result.
    ///
    /// If the child thread panics, an error is returned.
    ///
    /// # Panics
    ///
    /// This function may panic on some platforms if a thread attempts to join itself or otherwise
    /// may create a deadlock with joining threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::thread;
    ///
    /// thread::scope(|s| {
    ///     let handle1 = s.spawn(|_| println!("I'm a happy thread :)"));
    ///     let handle2 = s.spawn(|_| panic!("I'm a sad thread :("));
    ///
    ///     // Join the first thread and verify that it succeeded.
    ///     let res = handle1.join();
    ///     assert!(res.is_ok());
    ///
    ///     // Join the second thread and verify that it panicked.
    ///     let res = handle2.join();
    ///     assert!(res.is_err());
    /// }).unwrap();
    /// ```
    pub fn join(self) -> thread::Result<T> {
        // Take out the handle. The handle will surely be available because the root scope waits
        // for nested scopes before joining remaining threads.
        let handle = self.handle.lock().unwrap().take().unwrap();

        // Join the thread and then take the result out of its inner closure.
        handle
            .join()
            .map(|()| self.result.lock().unwrap().take().unwrap())
    }

    /// Returns a handle to the underlying thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::thread;
    ///
    /// thread::scope(|s| {
    ///     let handle = s.spawn(|_| println!("A child thread is running"));
    ///     println!("The child thread ID: {:?}", handle.thread().id());
    /// }).unwrap();
    /// ```
    pub fn thread(&self) -> &thread::Thread {
        &self.thread
    }
}

impl<'scope, T> fmt::Debug for ScopedJoinHandle<'scope, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ScopedJoinHandle { .. }")
    }
}
