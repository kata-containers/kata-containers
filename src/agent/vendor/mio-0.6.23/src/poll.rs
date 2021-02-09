use {sys, Token};
use event_imp::{self as event, Ready, Event, Evented, PollOpt};
use std::{fmt, io, ptr, usize};
use std::cell::UnsafeCell;
use std::{mem, ops, isize};
#[cfg(all(unix, not(target_os = "fuchsia")))]
use std::os::unix::io::AsRawFd;
#[cfg(all(unix, not(target_os = "fuchsia")))]
use std::os::unix::io::RawFd;
use std::process;
use std::sync::{Arc, Mutex, Condvar};
use std::sync::atomic::{AtomicUsize, AtomicPtr, AtomicBool};
use std::sync::atomic::Ordering::{self, Acquire, Release, AcqRel, Relaxed, SeqCst};
use std::time::{Duration, Instant};

// Poll is backed by two readiness queues. The first is a system readiness queue
// represented by `sys::Selector`. The system readiness queue handles events
// provided by the system, such as TCP and UDP. The second readiness queue is
// implemented in user space by `ReadinessQueue`. It provides a way to implement
// purely user space `Evented` types.
//
// `ReadinessQueue` is backed by a MPSC queue that supports reuse of linked
// list nodes. This significantly reduces the number of required allocations.
// Each `Registration` / `SetReadiness` pair allocates a single readiness node
// that is used for the lifetime of the registration.
//
// The readiness node also includes a single atomic variable, `state` that
// tracks most of the state associated with the registration. This includes the
// current readiness, interest, poll options, and internal state. When the node
// state is mutated, it is queued in the MPSC channel. A call to
// `ReadinessQueue::poll` will dequeue and process nodes. The node state can
// still be mutated while it is queued in the channel for processing.
// Intermediate state values do not matter as long as the final state is
// included in the call to `poll`. This is the eventually consistent nature of
// the readiness queue.
//
// The readiness node is ref counted using the `ref_count` field. On creation,
// the ref_count is initialized to 3: one `Registration` handle, one
// `SetReadiness` handle, and one for the readiness queue. Since the readiness queue
// doesn't *always* hold a handle to the node, we don't use the Arc type for
// managing ref counts (this is to avoid constantly incrementing and
// decrementing the ref count when pushing & popping from the queue). When the
// `Registration` handle is dropped, the `dropped` flag is set on the node, then
// the node is pushed into the registration queue. When Poll::poll pops the
// node, it sees the drop flag is set, and decrements it's ref count.
//
// The MPSC queue is a modified version of the intrusive MPSC node based queue
// described by 1024cores [1].
//
// The first modification is that two markers are used instead of a single
// `stub`. The second marker is a `sleep_marker` which is used to signal to
// producers that the consumer is going to sleep. This sleep_marker is only used
// when the queue is empty, implying that the only node in the queue is
// `end_marker`.
//
// The second modification is an `until` argument passed to the dequeue
// function. When `poll` encounters a level-triggered node, the node will be
// immediately pushed back into the queue. In order to avoid an infinite loop,
// `poll` before pushing the node, the pointer is saved off and then passed
// again as the `until` argument. If the next node to pop is `until`, then
// `Dequeue::Empty` is returned.
//
// [1] http://www.1024cores.net/home/lock-free-algorithms/queues/intrusive-mpsc-node-based-queue


/// Polls for readiness events on all registered values.
///
/// `Poll` allows a program to monitor a large number of `Evented` types,
/// waiting until one or more become "ready" for some class of operations; e.g.
/// reading and writing. An `Evented` type is considered ready if it is possible
/// to immediately perform a corresponding operation; e.g. [`read`] or
/// [`write`].
///
/// To use `Poll`, an `Evented` type must first be registered with the `Poll`
/// instance using the [`register`] method, supplying readiness interest. The
/// readiness interest tells `Poll` which specific operations on the handle to
/// monitor for readiness. A `Token` is also passed to the [`register`]
/// function. When `Poll` returns a readiness event, it will include this token.
/// This associates the event with the `Evented` handle that generated the
/// event.
///
/// [`read`]: tcp/struct.TcpStream.html#method.read
/// [`write`]: tcp/struct.TcpStream.html#method.write
/// [`register`]: #method.register
///
/// # Examples
///
/// A basic example -- establishing a `TcpStream` connection.
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Events, Poll, Ready, PollOpt, Token};
/// use mio::net::TcpStream;
///
/// use std::net::{TcpListener, SocketAddr};
///
/// // Bind a server socket to connect to.
/// let addr: SocketAddr = "127.0.0.1:0".parse()?;
/// let server = TcpListener::bind(&addr)?;
///
/// // Construct a new `Poll` handle as well as the `Events` we'll store into
/// let poll = Poll::new()?;
/// let mut events = Events::with_capacity(1024);
///
/// // Connect the stream
/// let stream = TcpStream::connect(&server.local_addr()?)?;
///
/// // Register the stream with `Poll`
/// poll.register(&stream, Token(0), Ready::readable() | Ready::writable(), PollOpt::edge())?;
///
/// // Wait for the socket to become ready. This has to happens in a loop to
/// // handle spurious wakeups.
/// loop {
///     poll.poll(&mut events, None)?;
///
///     for event in &events {
///         if event.token() == Token(0) && event.readiness().is_writable() {
///             // The socket connected (probably, it could still be a spurious
///             // wakeup)
///             return Ok(());
///         }
///     }
/// }
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// # Edge-triggered and level-triggered
///
/// An [`Evented`] registration may request edge-triggered events or
/// level-triggered events. This is done by setting `register`'s
/// [`PollOpt`] argument to either [`edge`] or [`level`].
///
/// The difference between the two can be described as follows. Supposed that
/// this scenario happens:
///
/// 1. A [`TcpStream`] is registered with `Poll`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`Poll::poll`] returns the token associated with the socket
///    indicating readable readiness.
/// 4. 1kb is read from the socket.
/// 5. Another call to [`Poll::poll`] is made.
///
/// If when the socket was registered with `Poll`, edge triggered events were
/// requested, then the call to [`Poll::poll`] done in step **5** will
/// (probably) hang despite there being another 1kb still present in the socket
/// read buffer. The reason for this is that edge-triggered mode delivers events
/// only when changes occur on the monitored [`Evented`]. So, in step *5* the
/// caller might end up waiting for some data that is already present inside the
/// socket buffer.
///
/// With edge-triggered events, operations **must** be performed on the
/// `Evented` type until [`WouldBlock`] is returned. In other words, after
/// receiving an event indicating readiness for a certain operation, one should
/// assume that [`Poll::poll`] may never return another event for the same token
/// and readiness until the operation returns [`WouldBlock`].
///
/// By contrast, when level-triggered notifications was requested, each call to
/// [`Poll::poll`] will return an event for the socket as long as data remains
/// in the socket buffer. Generally, level-triggered events should be avoided if
/// high performance is a concern.
///
/// Since even with edge-triggered events, multiple events can be generated upon
/// receipt of multiple chunks of data, the caller has the option to set the
/// [`oneshot`] flag. This tells `Poll` to disable the associated [`Evented`]
/// after the event is returned from [`Poll::poll`]. The subsequent calls to
/// [`Poll::poll`] will no longer include events for [`Evented`] handles that
/// are disabled even if the readiness state changes. The handle can be
/// re-enabled by calling [`reregister`]. When handles are disabled, internal
/// resources used to monitor the handle are maintained until the handle is
/// dropped or deregistered. This makes re-registering the handle a fast
/// operation.
///
/// For example, in the following scenario:
///
/// 1. A [`TcpStream`] is registered with `Poll`.
/// 2. The socket receives 2kb of data.
/// 3. A call to [`Poll::poll`] returns the token associated with the socket
///    indicating readable readiness.
/// 4. 2kb is read from the socket.
/// 5. Another call to read is issued and [`WouldBlock`] is returned
/// 6. The socket receives another 2kb of data.
/// 7. Another call to [`Poll::poll`] is made.
///
/// Assuming the socket was registered with `Poll` with the [`edge`] and
/// [`oneshot`] options, then the call to [`Poll::poll`] in step 7 would block. This
/// is because, [`oneshot`] tells `Poll` to disable events for the socket after
/// returning an event.
///
/// In order to receive the event for the data received in step 6, the socket
/// would need to be reregistered using [`reregister`].
///
/// [`PollOpt`]: struct.PollOpt.html
/// [`edge`]: struct.PollOpt.html#method.edge
/// [`level`]: struct.PollOpt.html#method.level
/// [`Poll::poll`]: struct.Poll.html#method.poll
/// [`WouldBlock`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock
/// [`Evented`]: event/trait.Evented.html
/// [`TcpStream`]: tcp/struct.TcpStream.html
/// [`reregister`]: #method.reregister
/// [`oneshot`]: struct.PollOpt.html#method.oneshot
///
/// # Portability
///
/// Using `Poll` provides a portable interface across supported platforms as
/// long as the caller takes the following into consideration:
///
/// ### Spurious events
///
/// [`Poll::poll`] may return readiness events even if the associated
/// [`Evented`] handle is not actually ready. Given the same code, this may
/// happen more on some platforms than others. It is important to never assume
/// that, just because a readiness notification was received, that the
/// associated operation will succeed as well.
///
/// If operation fails with [`WouldBlock`], then the caller should not treat
/// this as an error, but instead should wait until another readiness event is
/// received.
///
/// ### Draining readiness
///
/// When using edge-triggered mode, once a readiness event is received, the
/// corresponding operation must be performed repeatedly until it returns
/// [`WouldBlock`]. Unless this is done, there is no guarantee that another
/// readiness event will be delivered, even if further data is received for the
/// [`Evented`] handle.
///
/// For example, in the first scenario described above, after step 5, even if
/// the socket receives more data there is no guarantee that another readiness
/// event will be delivered.
///
/// ### Readiness operations
///
/// The only readiness operations that are guaranteed to be present on all
/// supported platforms are [`readable`] and [`writable`]. All other readiness
/// operations may have false negatives and as such should be considered
/// **hints**. This means that if a socket is registered with [`readable`],
/// [`error`], and [`hup`] interest, and either an error or hup is received, a
/// readiness event will be generated for the socket, but it **may** only
/// include `readable` readiness. Also note that, given the potential for
/// spurious events, receiving a readiness event with `hup` or `error` doesn't
/// actually mean that a `read` on the socket will return a result matching the
/// readiness event.
///
/// In other words, portable programs that explicitly check for [`hup`] or
/// [`error`] readiness should be doing so as an **optimization** and always be
/// able to handle an error or HUP situation when performing the actual read
/// operation.
///
/// [`readable`]: struct.Ready.html#method.readable
/// [`writable`]: struct.Ready.html#method.writable
/// [`error`]: unix/struct.UnixReady.html#method.error
/// [`hup`]: unix/struct.UnixReady.html#method.hup
///
/// ### Registering handles
///
/// Unless otherwise noted, it should be assumed that types implementing
/// [`Evented`] will never become ready unless they are registered with `Poll`.
///
/// For example:
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Poll, Ready, PollOpt, Token};
/// use mio::net::TcpStream;
/// use std::time::Duration;
/// use std::thread;
///
/// let sock = TcpStream::connect(&"216.58.193.100:80".parse()?)?;
///
/// thread::sleep(Duration::from_secs(1));
///
/// let poll = Poll::new()?;
///
/// // The connect is not guaranteed to have started until it is registered at
/// // this point
/// poll.register(&sock, Token(0), Ready::readable() | Ready::writable(), PollOpt::edge())?;
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// # Implementation notes
///
/// `Poll` is backed by the selector provided by the operating system.
///
/// |      OS    |  Selector |
/// |------------|-----------|
/// | Linux      | [epoll]   |
/// | OS X, iOS  | [kqueue]  |
/// | Windows    | [IOCP]    |
/// | FreeBSD    | [kqueue]  |
/// | Android    | [epoll]   |
///
/// On all supported platforms, socket operations are handled by using the
/// system selector. Platform specific extensions (e.g. [`EventedFd`]) allow
/// accessing other features provided by individual system selectors. For
/// example, Linux's [`signalfd`] feature can be used by registering the FD with
/// `Poll` via [`EventedFd`].
///
/// On all platforms except windows, a call to [`Poll::poll`] is mostly just a
/// direct call to the system selector. However, [IOCP] uses a completion model
/// instead of a readiness model. In this case, `Poll` must adapt the completion
/// model Mio's API. While non-trivial, the bridge layer is still quite
/// efficient. The most expensive part being calls to `read` and `write` require
/// data to be copied into an intermediate buffer before it is passed to the
/// kernel.
///
/// Notifications generated by [`SetReadiness`] are handled by an internal
/// readiness queue. A single call to [`Poll::poll`] will collect events from
/// both from the system selector and the internal readiness queue.
///
/// [epoll]: http://man7.org/linux/man-pages/man7/epoll.7.html
/// [kqueue]: https://www.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
/// [IOCP]: https://msdn.microsoft.com/en-us/library/windows/desktop/aa365198(v=vs.85).aspx
/// [`signalfd`]: http://man7.org/linux/man-pages/man2/signalfd.2.html
/// [`EventedFd`]: unix/struct.EventedFd.html
/// [`SetReadiness`]: struct.SetReadiness.html
/// [`Poll::poll`]: struct.Poll.html#method.poll
pub struct Poll {
    // Platform specific IO selector
    selector: sys::Selector,

    // Custom readiness queue
    readiness_queue: ReadinessQueue,

    // Use an atomic to first check if a full lock will be required. This is a
    // fast-path check for single threaded cases avoiding the extra syscall
    lock_state: AtomicUsize,

    // Sequences concurrent calls to `Poll::poll`
    lock: Mutex<()>,

    // Wakeup the next waiter
    condvar: Condvar,
}

/// Handle to a user space `Poll` registration.
///
/// `Registration` allows implementing [`Evented`] for types that cannot work
/// with the [system selector]. A `Registration` is always paired with a
/// `SetReadiness`, which allows updating the registration's readiness state.
/// When [`set_readiness`] is called and the `Registration` is associated with a
/// [`Poll`] instance, a readiness event will be created and eventually returned
/// by [`poll`].
///
/// A `Registration` / `SetReadiness` pair is created by calling
/// [`Registration::new2`]. At this point, the registration is not being
/// monitored by a [`Poll`] instance, so calls to `set_readiness` will not
/// result in any readiness notifications.
///
/// `Registration` implements [`Evented`], so it can be used with [`Poll`] using
/// the same [`register`], [`reregister`], and [`deregister`] functions used
/// with TCP, UDP, etc... types. Once registered with [`Poll`], readiness state
/// changes result in readiness events being dispatched to the [`Poll`] instance
/// with which `Registration` is registered.
///
/// **Note**, before using `Registration` be sure to read the
/// [`set_readiness`] documentation and the [portability] notes. The
/// guarantees offered by `Registration` may be weaker than expected.
///
/// For high level documentation, see [`Poll`].
///
/// # Examples
///
/// ```
/// use mio::{Ready, Registration, Poll, PollOpt, Token};
/// use mio::event::Evented;
///
/// use std::io;
/// use std::time::Instant;
/// use std::thread;
///
/// pub struct Deadline {
///     when: Instant,
///     registration: Registration,
/// }
///
/// impl Deadline {
///     pub fn new(when: Instant) -> Deadline {
///         let (registration, set_readiness) = Registration::new2();
///
///         thread::spawn(move || {
///             let now = Instant::now();
///
///             if now < when {
///                 thread::sleep(when - now);
///             }
///
///             set_readiness.set_readiness(Ready::readable());
///         });
///
///         Deadline {
///             when: when,
///             registration: registration,
///         }
///     }
///
///     pub fn is_elapsed(&self) -> bool {
///         Instant::now() >= self.when
///     }
/// }
///
/// impl Evented for Deadline {
///     fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
///         -> io::Result<()>
///     {
///         self.registration.register(poll, token, interest, opts)
///     }
///
///     fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
///         -> io::Result<()>
///     {
///         self.registration.reregister(poll, token, interest, opts)
///     }
///
///     fn deregister(&self, poll: &Poll) -> io::Result<()> {
///         poll.deregister(&self.registration)
///     }
/// }
/// ```
///
/// [system selector]: struct.Poll.html#implementation-notes
/// [`Poll`]: struct.Poll.html
/// [`Registration::new2`]: struct.Registration.html#method.new2
/// [`Evented`]: event/trait.Evented.html
/// [`set_readiness`]: struct.SetReadiness.html#method.set_readiness
/// [`register`]: struct.Poll.html#method.register
/// [`reregister`]: struct.Poll.html#method.reregister
/// [`deregister`]: struct.Poll.html#method.deregister
/// [portability]: struct.Poll.html#portability
pub struct Registration {
    inner: RegistrationInner,
}

unsafe impl Send for Registration {}
unsafe impl Sync for Registration {}

/// Updates the readiness state of the associated `Registration`.
///
/// See [`Registration`] for more documentation on using `SetReadiness` and
/// [`Poll`] for high level polling documentation.
///
/// [`Poll`]: struct.Poll.html
/// [`Registration`]: struct.Registration.html
#[derive(Clone)]
pub struct SetReadiness {
    inner: RegistrationInner,
}

unsafe impl Send for SetReadiness {}
unsafe impl Sync for SetReadiness {}

/// Used to associate an IO type with a Selector
#[derive(Debug)]
pub struct SelectorId {
    id: AtomicUsize,
}

struct RegistrationInner {
    // Unsafe pointer to the registration's node. The node is ref counted. This
    // cannot "simply" be tracked by an Arc because `Poll::poll` has an implicit
    // handle though it isn't stored anywhere. In other words, `Poll::poll`
    // needs to decrement the ref count before the node is freed.
    node: *mut ReadinessNode,
}

#[derive(Clone)]
struct ReadinessQueue {
    inner: Arc<ReadinessQueueInner>,
}

unsafe impl Send for ReadinessQueue {}
unsafe impl Sync for ReadinessQueue {}

struct ReadinessQueueInner {
    // Used to wake up `Poll` when readiness is set in another thread.
    awakener: sys::Awakener,

    // Head of the MPSC queue used to signal readiness to `Poll::poll`.
    head_readiness: AtomicPtr<ReadinessNode>,

    // Tail of the readiness queue.
    //
    // Only accessed by Poll::poll. Coordination will be handled by the poll fn
    tail_readiness: UnsafeCell<*mut ReadinessNode>,

    // Fake readiness node used to punctuate the end of the readiness queue.
    // Before attempting to read from the queue, this node is inserted in order
    // to partition the queue between nodes that are "owned" by the dequeue end
    // and nodes that will be pushed on by producers.
    end_marker: Box<ReadinessNode>,

    // Similar to `end_marker`, but this node signals to producers that `Poll`
    // has gone to sleep and must be woken up.
    sleep_marker: Box<ReadinessNode>,

    // Similar to `end_marker`, but the node signals that the queue is closed.
    // This happens when `ReadyQueue` is dropped and signals to producers that
    // the nodes should no longer be pushed into the queue.
    closed_marker: Box<ReadinessNode>,
}

/// Node shared by a `Registration` / `SetReadiness` pair as well as the node
/// queued into the MPSC channel.
struct ReadinessNode {
    // Node state, see struct docs for `ReadinessState`
    //
    // This variable is the primary point of coordination between all the
    // various threads concurrently accessing the node.
    state: AtomicState,

    // The registration token cannot fit into the `state` variable, so it is
    // broken out here. In order to atomically update both the state and token
    // we have to jump through a few hoops.
    //
    // First, `state` includes `token_read_pos` and `token_write_pos`. These can
    // either be 0, 1, or 2 which represent a token slot. `token_write_pos` is
    // the token slot that contains the most up to date registration token.
    // `token_read_pos` is the token slot that `poll` is currently reading from.
    //
    // When a call to `update` includes a different token than the one currently
    // associated with the registration (token_write_pos), first an unused token
    // slot is found. The unused slot is the one not represented by
    // `token_read_pos` OR `token_write_pos`. The new token is written to this
    // slot, then `state` is updated with the new `token_write_pos` value. This
    // requires that there is only a *single* concurrent call to `update`.
    //
    // When `poll` reads a node state, it checks that `token_read_pos` matches
    // `token_write_pos`. If they do not match, then it atomically updates
    // `state` such that `token_read_pos` is set to `token_write_pos`. It will
    // then read the token at the newly updated `token_read_pos`.
    token_0: UnsafeCell<Token>,
    token_1: UnsafeCell<Token>,
    token_2: UnsafeCell<Token>,

    // Used when the node is queued in the readiness linked list. Accessing
    // this field requires winning the "queue" lock
    next_readiness: AtomicPtr<ReadinessNode>,

    // Ensures that there is only one concurrent call to `update`.
    //
    // Each call to `update` will attempt to swap `update_lock` from `false` to
    // `true`. If the CAS succeeds, the thread has obtained the update lock. If
    // the CAS fails, then the `update` call returns immediately and the update
    // is discarded.
    update_lock: AtomicBool,

    // Pointer to Arc<ReadinessQueueInner>
    readiness_queue: AtomicPtr<()>,

    // Tracks the number of `ReadyRef` pointers
    ref_count: AtomicUsize,
}

/// Stores the ReadinessNode state in an AtomicUsize. This wrapper around the
/// atomic variable handles encoding / decoding `ReadinessState` values.
struct AtomicState {
    inner: AtomicUsize,
}

const MASK_2: usize = 4 - 1;
const MASK_4: usize = 16 - 1;
const QUEUED_MASK: usize = 1 << QUEUED_SHIFT;
const DROPPED_MASK: usize = 1 << DROPPED_SHIFT;

const READINESS_SHIFT: usize = 0;
const INTEREST_SHIFT: usize = 4;
const POLL_OPT_SHIFT: usize = 8;
const TOKEN_RD_SHIFT: usize = 12;
const TOKEN_WR_SHIFT: usize = 14;
const QUEUED_SHIFT: usize = 16;
const DROPPED_SHIFT: usize = 17;

/// Tracks all state for a single `ReadinessNode`. The state is packed into a
/// `usize` variable from low to high bit as follows:
///
/// 4 bits: Registration current readiness
/// 4 bits: Registration interest
/// 4 bits: Poll options
/// 2 bits: Token position currently being read from by `poll`
/// 2 bits: Token position last written to by `update`
/// 1 bit:  Queued flag, set when node is being pushed into MPSC queue.
/// 1 bit:  Dropped flag, set when all `Registration` handles have been dropped.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct ReadinessState(usize);

/// Returned by `dequeue_node`. Represents the different states as described by
/// the queue documentation on 1024cores.net.
enum Dequeue {
    Data(*mut ReadinessNode),
    Empty,
    Inconsistent,
}

const AWAKEN: Token = Token(usize::MAX);
const MAX_REFCOUNT: usize = (isize::MAX) as usize;

/*
 *
 * ===== Poll =====
 *
 */

impl Poll {
    /// Return a new `Poll` handle.
    ///
    /// This function will make a syscall to the operating system to create the
    /// system selector. If this syscall fails, `Poll::new` will return with the
    /// error.
    ///
    /// See [struct] level docs for more details.
    ///
    /// [struct]: struct.Poll.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Poll, Events};
    /// use std::time::Duration;
    ///
    /// let poll = match Poll::new() {
    ///     Ok(poll) => poll,
    ///     Err(e) => panic!("failed to create Poll instance; err={:?}", e),
    /// };
    ///
    /// // Create a structure to receive polled events
    /// let mut events = Events::with_capacity(1024);
    ///
    /// // Wait for events, but none will be received because no `Evented`
    /// // handles have been registered with this `Poll` instance.
    /// let n = poll.poll(&mut events, Some(Duration::from_millis(500)))?;
    /// assert_eq!(n, 0);
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn new() -> io::Result<Poll> {
        is_send::<Poll>();
        is_sync::<Poll>();

        let poll = Poll {
            selector: sys::Selector::new()?,
            readiness_queue: ReadinessQueue::new()?,
            lock_state: AtomicUsize::new(0),
            lock: Mutex::new(()),
            condvar: Condvar::new(),
        };

        // Register the notification wakeup FD with the IO poller
        poll.readiness_queue.inner.awakener.register(&poll, AWAKEN, Ready::readable(), PollOpt::edge())?;

        Ok(poll)
    }

    /// Register an `Evented` handle with the `Poll` instance.
    ///
    /// Once registered, the `Poll` instance will monitor the `Evented` handle
    /// for readiness state changes. When it notices a state change, it will
    /// return a readiness event for the handle the next time [`poll`] is
    /// called.
    ///
    /// See the [`struct`] docs for a high level overview.
    ///
    /// # Arguments
    ///
    /// `handle: &E: Evented`: This is the handle that the `Poll` instance
    /// should monitor for readiness state changes.
    ///
    /// `token: Token`: The caller picks a token to associate with the socket.
    /// When [`poll`] returns an event for the handle, this token is included.
    /// This allows the caller to map the event to its handle. The token
    /// associated with the `Evented` handle can be changed at any time by
    /// calling [`reregister`].
    ///
    /// `token` cannot be `Token(usize::MAX)` as it is reserved for internal
    /// usage.
    ///
    /// See documentation on [`Token`] for an example showing how to pick
    /// [`Token`] values.
    ///
    /// `interest: Ready`: Specifies which operations `Poll` should monitor for
    /// readiness. `Poll` will only return readiness events for operations
    /// specified by this argument.
    ///
    /// If a socket is registered with readable interest and the socket becomes
    /// writable, no event will be returned from [`poll`].
    ///
    /// The readiness interest for an `Evented` handle can be changed at any
    /// time by calling [`reregister`].
    ///
    /// `opts: PollOpt`: Specifies the registration options. The most common
    /// options being [`level`] for level-triggered events, [`edge`] for
    /// edge-triggered events, and [`oneshot`].
    ///
    /// The registration options for an `Evented` handle can be changed at any
    /// time by calling [`reregister`].
    ///
    /// # Notes
    ///
    /// Unless otherwise specified, the caller should assume that once an
    /// `Evented` handle is registered with a `Poll` instance, it is bound to
    /// that `Poll` instance for the lifetime of the `Evented` handle. This
    /// remains true even if the `Evented` handle is deregistered from the poll
    /// instance using [`deregister`].
    ///
    /// This function is **thread safe**. It can be called concurrently from
    /// multiple threads.
    ///
    /// [`struct`]: #
    /// [`reregister`]: #method.reregister
    /// [`deregister`]: #method.deregister
    /// [`poll`]: #method.poll
    /// [`level`]: struct.PollOpt.html#method.level
    /// [`edge`]: struct.PollOpt.html#method.edge
    /// [`oneshot`]: struct.PollOpt.html#method.oneshot
    /// [`Token`]: struct.Token.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    /// use std::time::{Duration, Instant};
    ///
    /// let poll = Poll::new()?;
    /// let socket = TcpStream::connect(&"216.58.193.100:80".parse()?)?;
    ///
    /// // Register the socket with `poll`
    /// poll.register(&socket, Token(0), Ready::readable() | Ready::writable(), PollOpt::edge())?;
    ///
    /// let mut events = Events::with_capacity(1024);
    /// let start = Instant::now();
    /// let timeout = Duration::from_millis(500);
    ///
    /// loop {
    ///     let elapsed = start.elapsed();
    ///
    ///     if elapsed >= timeout {
    ///         // Connection timed out
    ///         return Ok(());
    ///     }
    ///
    ///     let remaining = timeout - elapsed;
    ///     poll.poll(&mut events, Some(remaining))?;
    ///
    ///     for event in &events {
    ///         if event.token() == Token(0) {
    ///             // Something (probably) happened on the socket.
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn register<E: ?Sized>(&self, handle: &E, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()>
        where E: Evented
    {
        validate_args(token)?;

        /*
         * Undefined behavior:
         * - Reusing a token with a different `Evented` without deregistering
         * (or closing) the original `Evented`.
         */
        trace!("registering with poller");

        // Register interests for this socket
        handle.register(self, token, interest, opts)?;

        Ok(())
    }

    /// Re-register an `Evented` handle with the `Poll` instance.
    ///
    /// Re-registering an `Evented` handle allows changing the details of the
    /// registration. Specifically, it allows updating the associated `token`,
    /// `interest`, and `opts` specified in previous `register` and `reregister`
    /// calls.
    ///
    /// The `reregister` arguments fully override the previous values. In other
    /// words, if a socket is registered with [`readable`] interest and the call
    /// to `reregister` specifies [`writable`], then read interest is no longer
    /// requested for the handle.
    ///
    /// The `Evented` handle must have previously been registered with this
    /// instance of `Poll` otherwise the call to `reregister` will return with
    /// an error.
    ///
    /// `token` cannot be `Token(usize::MAX)` as it is reserved for internal
    /// usage.
    ///
    /// See the [`register`] documentation for details about the function
    /// arguments and see the [`struct`] docs for a high level overview of
    /// polling.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    ///
    /// let poll = Poll::new()?;
    /// let socket = TcpStream::connect(&"216.58.193.100:80".parse()?)?;
    ///
    /// // Register the socket with `poll`, requesting readable
    /// poll.register(&socket, Token(0), Ready::readable(), PollOpt::edge())?;
    ///
    /// // Reregister the socket specifying a different token and write interest
    /// // instead. `PollOpt::edge()` must be specified even though that value
    /// // is not being changed.
    /// poll.reregister(&socket, Token(2), Ready::writable(), PollOpt::edge())?;
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`struct`]: #
    /// [`register`]: #method.register
    /// [`readable`]: struct.Ready.html#method.readable
    /// [`writable`]: struct.Ready.html#method.writable
    pub fn reregister<E: ?Sized>(&self, handle: &E, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()>
        where E: Evented
    {
        validate_args(token)?;

        trace!("registering with poller");

        // Register interests for this socket
        handle.reregister(self, token, interest, opts)?;

        Ok(())
    }

    /// Deregister an `Evented` handle with the `Poll` instance.
    ///
    /// When an `Evented` handle is deregistered, the `Poll` instance will
    /// no longer monitor it for readiness state changes. Unlike disabling
    /// handles with oneshot, deregistering clears up any internal resources
    /// needed to track the handle.
    ///
    /// A handle can be passed back to `register` after it has been
    /// deregistered; however, it must be passed back to the **same** `Poll`
    /// instance.
    ///
    /// `Evented` handles are automatically deregistered when they are dropped.
    /// It is common to never need to explicitly call `deregister`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    /// use std::time::Duration;
    ///
    /// let poll = Poll::new()?;
    /// let socket = TcpStream::connect(&"216.58.193.100:80".parse()?)?;
    ///
    /// // Register the socket with `poll`
    /// poll.register(&socket, Token(0), Ready::readable(), PollOpt::edge())?;
    ///
    /// poll.deregister(&socket)?;
    ///
    /// let mut events = Events::with_capacity(1024);
    ///
    /// // Set a timeout because this poll should never receive any events.
    /// let n = poll.poll(&mut events, Some(Duration::from_secs(1)))?;
    /// assert_eq!(0, n);
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn deregister<E: ?Sized>(&self, handle: &E) -> io::Result<()>
        where E: Evented
    {
        trace!("deregistering handle with poller");

        // Deregister interests for this socket
        handle.deregister(self)?;

        Ok(())
    }

    /// Wait for readiness events
    ///
    /// Blocks the current thread and waits for readiness events for any of the
    /// `Evented` handles that have been registered with this `Poll` instance.
    /// The function will block until either at least one readiness event has
    /// been received or `timeout` has elapsed. A `timeout` of `None` means that
    /// `poll` will block until a readiness event has been received.
    ///
    /// The supplied `events` will be cleared and newly received readiness events
    /// will be pushed onto the end. At most `events.capacity()` events will be
    /// returned. If there are further pending readiness events, they will be
    /// returned on the next call to `poll`.
    ///
    /// A single call to `poll` may result in multiple readiness events being
    /// returned for a single `Evented` handle. For example, if a TCP socket
    /// becomes both readable and writable, it may be possible for a single
    /// readiness event to be returned with both [`readable`] and [`writable`]
    /// readiness **OR** two separate events may be returned, one with
    /// [`readable`] set and one with [`writable`] set.
    ///
    /// Note that the `timeout` will be rounded up to the system clock
    /// granularity (usually 1ms), and kernel scheduling delays mean that
    /// the blocking interval may be overrun by a small amount.
    ///
    /// `poll` returns the number of readiness events that have been pushed into
    /// `events` or `Err` when an error has been encountered with the system
    /// selector.  The value returned is deprecated and will be removed in 0.7.0.
    /// Accessing the events by index is also deprecated.  Events can be
    /// inserted by other events triggering, thus making sequential access
    /// problematic.  Use the iterator API instead.  See [`iter`].
    ///
    /// See the [struct] level documentation for a higher level discussion of
    /// polling.
    ///
    /// [`readable`]: struct.Ready.html#method.readable
    /// [`writable`]: struct.Ready.html#method.writable
    /// [struct]: #
    /// [`iter`]: struct.Events.html#method.iter
    ///
    /// # Examples
    ///
    /// A basic example -- establishing a `TcpStream` connection.
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll, Ready, PollOpt, Token};
    /// use mio::net::TcpStream;
    ///
    /// use std::net::{TcpListener, SocketAddr};
    /// use std::thread;
    ///
    /// // Bind a server socket to connect to.
    /// let addr: SocketAddr = "127.0.0.1:0".parse()?;
    /// let server = TcpListener::bind(&addr)?;
    /// let addr = server.local_addr()?.clone();
    ///
    /// // Spawn a thread to accept the socket
    /// thread::spawn(move || {
    ///     let _ = server.accept();
    /// });
    ///
    /// // Construct a new `Poll` handle as well as the `Events` we'll store into
    /// let poll = Poll::new()?;
    /// let mut events = Events::with_capacity(1024);
    ///
    /// // Connect the stream
    /// let stream = TcpStream::connect(&addr)?;
    ///
    /// // Register the stream with `Poll`
    /// poll.register(&stream, Token(0), Ready::readable() | Ready::writable(), PollOpt::edge())?;
    ///
    /// // Wait for the socket to become ready. This has to happens in a loop to
    /// // handle spurious wakeups.
    /// loop {
    ///     poll.poll(&mut events, None)?;
    ///
    ///     for event in &events {
    ///         if event.token() == Token(0) && event.readiness().is_writable() {
    ///             // The socket connected (probably, it could still be a spurious
    ///             // wakeup)
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [struct]: #
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<usize> {
        self.poll1(events, timeout, false)
    }

    /// Like `poll`, but may be interrupted by a signal
    ///
    /// If `poll` is inturrupted while blocking, it will transparently retry the syscall.  If you
    /// want to handle signals yourself, however, use `poll_interruptible`.
    pub fn poll_interruptible(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<usize> {
        self.poll1(events, timeout, true)
    }

    fn poll1(&self, events: &mut Events, mut timeout: Option<Duration>, interruptible: bool) -> io::Result<usize> {
        let zero = Some(Duration::from_millis(0));

        // At a high level, the synchronization strategy is to acquire access to
        // the critical section by transitioning the atomic from unlocked ->
        // locked. If the attempt fails, the thread will wait on the condition
        // variable.
        //
        // # Some more detail
        //
        // The `lock_state` atomic usize combines:
        //
        // - locked flag, stored in the least significant bit
        // - number of waiting threads, stored in the rest of the bits.
        //
        // When a thread transitions the locked flag from 0 -> 1, it has
        // obtained access to the critical section.
        //
        // When entering `poll`, a compare-and-swap from 0 -> 1 is attempted.
        // This is a fast path for the case when there are no concurrent calls
        // to poll, which is very common.
        //
        // On failure, the mutex is locked, and the thread attempts to increment
        // the number of waiting threads component of `lock_state`. If this is
        // successfully done while the locked flag is set, then the thread can
        // wait on the condition variable.
        //
        // When a thread exits the critical section, it unsets the locked flag.
        // If there are any waiters, which is atomically determined while
        // unsetting the locked flag, then the condvar is notified.

        let mut curr = self.lock_state.compare_and_swap(0, 1, SeqCst);

        if 0 != curr {
            // Enter slower path
            let mut lock = self.lock.lock().unwrap();
            let mut inc = false;

            loop {
                if curr & 1 == 0 {
                    // The lock is currently free, attempt to grab it
                    let mut next = curr | 1;

                    if inc {
                        // The waiter count has previously been incremented, so
                        // decrement it here
                        next -= 2;
                    }

                    let actual = self.lock_state.compare_and_swap(curr, next, SeqCst);

                    if actual != curr {
                        curr = actual;
                        continue;
                    }

                    // Lock acquired, break from the loop
                    break;
                }

                if timeout == zero {
                    if inc {
                        self.lock_state.fetch_sub(2, SeqCst);
                    }

                    return Ok(0);
                }

                // The lock is currently held, so wait for it to become
                // free. If the waiter count hasn't been incremented yet, do
                // so now
                if !inc {
                    let next = curr.checked_add(2).expect("overflow");
                    let actual = self.lock_state.compare_and_swap(curr, next, SeqCst);

                    if actual != curr {
                        curr = actual;
                        continue;
                    }

                    // Track that the waiter count has been incremented for
                    // this thread and fall through to the condvar waiting
                    inc = true;
                }

                lock = match timeout {
                    Some(to) => {
                        let now = Instant::now();

                        // Wait to be notified
                        let (l, _) = self.condvar.wait_timeout(lock, to).unwrap();

                        // See how much time was elapsed in the wait
                        let elapsed = now.elapsed();

                        // Update `timeout` to reflect how much time is left to
                        // wait.
                        if elapsed >= to {
                            timeout = zero;
                        } else {
                            // Update the timeout
                            timeout = Some(to - elapsed);
                        }

                        l
                    }
                    None => {
                        self.condvar.wait(lock).unwrap()
                    }
                };

                // Reload the state
                curr = self.lock_state.load(SeqCst);

                // Try to lock again...
            }
        }

        let ret = self.poll2(events, timeout, interruptible);

        // Release the lock
        if 1 != self.lock_state.fetch_and(!1, Release) {
            // Acquire the mutex
            let _lock = self.lock.lock().unwrap();

            // There is at least one waiting thread, so notify one
            self.condvar.notify_one();
        }

        ret
    }

    #[inline]
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::if_same_then_else))]
    fn poll2(&self, events: &mut Events, mut timeout: Option<Duration>, interruptible: bool) -> io::Result<usize> {
        // Compute the timeout value passed to the system selector. If the
        // readiness queue has pending nodes, we still want to poll the system
        // selector for new events, but we don't want to block the thread to
        // wait for new events.
        if timeout == Some(Duration::from_millis(0)) {
            // If blocking is not requested, then there is no need to prepare
            // the queue for sleep
            //
            // The sleep_marker should be removed by readiness_queue.poll().
        } else if self.readiness_queue.prepare_for_sleep() {
            // The readiness queue is empty. The call to `prepare_for_sleep`
            // inserts `sleep_marker` into the queue. This signals to any
            // threads setting readiness that the `Poll::poll` is going to
            // sleep, so the awakener should be used.
        } else {
            // The readiness queue is not empty, so do not block the thread.
            timeout = Some(Duration::from_millis(0));
        }

        loop {
            let now = Instant::now();
            // First get selector events
            let res = self.selector.select(&mut events.inner, AWAKEN, timeout);
            match res {
                Ok(true) => {
                    // Some awakeners require reading from a FD.
                    self.readiness_queue.inner.awakener.cleanup();
                    break;
                }
                Ok(false) => break,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted && !interruptible => {
                    // Interrupted by a signal; update timeout if necessary and retry
                    if let Some(to) = timeout {
                        let elapsed = now.elapsed();
                        if elapsed >= to {
                            break;
                        } else {
                            timeout = Some(to - elapsed);
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // Poll custom event queue
        self.readiness_queue.poll(&mut events.inner);

        // Return number of polled events
        Ok(events.inner.len())
    }
}

fn validate_args(token: Token) -> io::Result<()> {
    if token == AWAKEN {
        return Err(io::Error::new(io::ErrorKind::Other, "invalid token"));
    }

    Ok(())
}

impl fmt::Debug for Poll {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Poll")
            .finish()
    }
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
impl AsRawFd for Poll {
    fn as_raw_fd(&self) -> RawFd {
        self.selector.as_raw_fd()
    }
}

/// A collection of readiness events.
///
/// `Events` is passed as an argument to [`Poll::poll`] and will be used to
/// receive any new readiness events received since the last poll. Usually, a
/// single `Events` instance is created at the same time as a [`Poll`] and
/// reused on each call to [`Poll::poll`].
///
/// See [`Poll`] for more documentation on polling.
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Events, Poll};
/// use std::time::Duration;
///
/// let mut events = Events::with_capacity(1024);
/// let poll = Poll::new()?;
///
/// assert_eq!(0, events.len());
///
/// // Register `Evented` handles with `poll`
///
/// poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///
/// for event in &events {
///     println!("event={:?}", event);
/// }
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// [`Poll::poll`]: struct.Poll.html#method.poll
/// [`Poll`]: struct.Poll.html
pub struct Events {
    inner: sys::Events,
}

/// [`Events`] iterator.
///
/// This struct is created by the [`iter`] method on [`Events`].
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Events, Poll};
/// use std::time::Duration;
///
/// let mut events = Events::with_capacity(1024);
/// let poll = Poll::new()?;
///
/// // Register handles with `poll`
///
/// poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///
/// for event in events.iter() {
///     println!("event={:?}", event);
/// }
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// [`Events`]: struct.Events.html
/// [`iter`]: struct.Events.html#method.iter
#[derive(Debug, Clone)]
pub struct Iter<'a> {
    inner: &'a Events,
    pos: usize,
}

/// Owned [`Events`] iterator.
///
/// This struct is created by the `into_iter` method on [`Events`].
///
/// # Examples
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Events, Poll};
/// use std::time::Duration;
///
/// let mut events = Events::with_capacity(1024);
/// let poll = Poll::new()?;
///
/// // Register handles with `poll`
///
/// poll.poll(&mut events, Some(Duration::from_millis(100)))?;
///
/// for event in events {
///     println!("event={:?}", event);
/// }
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
/// [`Events`]: struct.Events.html
#[derive(Debug)]
pub struct IntoIter {
    inner: Events,
    pos: usize,
}

impl Events {
    /// Return a new `Events` capable of holding up to `capacity` events.
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::Events;
    ///
    /// let events = Events::with_capacity(1024);
    ///
    /// assert_eq!(1024, events.capacity());
    /// ```
    pub fn with_capacity(capacity: usize) -> Events {
        Events {
            inner: sys::Events::with_capacity(capacity),
        }
    }

    #[deprecated(since="0.6.10", note="Index access removed in favor of iterator only API.")]
    #[doc(hidden)]
    pub fn get(&self, idx: usize) -> Option<Event> {
        self.inner.get(idx)
    }

    #[doc(hidden)]
    #[deprecated(since="0.6.10", note="Index access removed in favor of iterator only API.")]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns the number of `Event` values that `self` can hold.
    ///
    /// ```
    /// use mio::Events;
    ///
    /// let events = Events::with_capacity(1024);
    ///
    /// assert_eq!(1024, events.capacity());
    /// ```
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Returns `true` if `self` contains no `Event` values.
    ///
    /// # Examples
    ///
    /// ```
    /// use mio::Events;
    ///
    /// let events = Events::with_capacity(1024);
    ///
    /// assert!(events.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns an iterator over the `Event` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll};
    /// use std::time::Duration;
    ///
    /// let mut events = Events::with_capacity(1024);
    /// let poll = Poll::new()?;
    ///
    /// // Register handles with `poll`
    ///
    /// poll.poll(&mut events, Some(Duration::from_millis(100)))?;
    ///
    /// for event in events.iter() {
    ///     println!("event={:?}", event);
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn iter(&self) -> Iter {
        Iter {
            inner: self,
            pos: 0
        }
    }

    /// Clearing all `Event` values from container explicitly.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Poll};
    /// use std::time::Duration;
    ///
    /// let mut events = Events::with_capacity(1024);
    /// let poll = Poll::new()?;
    ///
    /// // Register handles with `poll`
    /// for _ in 0..2 {
    ///     events.clear();
    ///     poll.poll(&mut events, Some(Duration::from_millis(100)))?;
    ///
    ///     for event in events.iter() {
    ///         println!("event={:?}", event);
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<'a> IntoIterator for &'a Events {
    type Item = Event;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let ret = self.inner.inner.get(self.pos);
        self.pos += 1;
        ret
    }
}

impl IntoIterator for Events {
    type Item = Event;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self,
            pos: 0,
        }
    }
}

impl Iterator for IntoIter {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let ret = self.inner.inner.get(self.pos);
        self.pos += 1;
        ret
    }
}

impl fmt::Debug for Events {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Events")
            .field("capacity", &self.capacity())
            .finish()
    }
}

// ===== Accessors for internal usage =====

pub fn selector(poll: &Poll) -> &sys::Selector {
    &poll.selector
}

/*
 *
 * ===== Registration =====
 *
 */

// TODO: get rid of this, windows depends on it for now
#[allow(dead_code)]
pub fn new_registration(poll: &Poll, token: Token, ready: Ready, opt: PollOpt)
        -> (Registration, SetReadiness)
{
    Registration::new_priv(poll, token, ready, opt)
}

impl Registration {
    /// Create and return a new `Registration` and the associated
    /// `SetReadiness`.
    ///
    /// See [struct] documentation for more detail and [`Poll`]
    /// for high level documentation on polling.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Ready, Registration, Poll, PollOpt, Token};
    /// use std::thread;
    ///
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// thread::spawn(move || {
    ///     use std::time::Duration;
    ///     thread::sleep(Duration::from_millis(500));
    ///
    ///     set_readiness.set_readiness(Ready::readable());
    /// });
    ///
    /// let poll = Poll::new()?;
    /// poll.register(&registration, Token(0), Ready::readable() | Ready::writable(), PollOpt::edge())?;
    ///
    /// let mut events = Events::with_capacity(256);
    ///
    /// loop {
    ///     poll.poll(&mut events, None);
    ///
    ///     for event in &events {
    ///         if event.token() == Token(0) && event.readiness().is_readable() {
    ///             return Ok(());
    ///         }
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    /// [struct]: #
    /// [`Poll`]: struct.Poll.html
    pub fn new2() -> (Registration, SetReadiness) {
        // Allocate the registration node. The new node will have `ref_count`
        // set to 2: one SetReadiness, one Registration.
        let node = Box::into_raw(Box::new(ReadinessNode::new(
                    ptr::null_mut(), Token(0), Ready::empty(), PollOpt::empty(), 2)));

        let registration = Registration {
            inner: RegistrationInner {
                node,
            },
        };

        let set_readiness = SetReadiness {
            inner: RegistrationInner {
                node,
            },
        };

        (registration, set_readiness)
    }

    #[deprecated(since = "0.6.5", note = "use `new2` instead")]
    #[cfg(feature = "with-deprecated")]
    #[doc(hidden)]
    pub fn new(poll: &Poll, token: Token, interest: Ready, opt: PollOpt)
        -> (Registration, SetReadiness)
    {
        Registration::new_priv(poll, token, interest, opt)
    }

    // TODO: Get rid of this (windows depends on it for now)
    fn new_priv(poll: &Poll, token: Token, interest: Ready, opt: PollOpt)
        -> (Registration, SetReadiness)
    {
        is_send::<Registration>();
        is_sync::<Registration>();
        is_send::<SetReadiness>();
        is_sync::<SetReadiness>();

        // Clone handle to the readiness queue, this bumps the ref count
        let queue = poll.readiness_queue.inner.clone();

        // Convert to a *mut () pointer
        let queue: *mut () = unsafe { mem::transmute(queue) };

        // Allocate the registration node. The new node will have `ref_count`
        // set to 3: one SetReadiness, one Registration, and one Poll handle.
        let node = Box::into_raw(Box::new(ReadinessNode::new(
                    queue, token, interest, opt, 3)));

        let registration = Registration {
            inner: RegistrationInner {
                node,
            },
        };

        let set_readiness = SetReadiness {
            inner: RegistrationInner {
                node,
            },
        };

        (registration, set_readiness)
    }

    #[deprecated(since = "0.6.5", note = "use `Evented` impl")]
    #[cfg(feature = "with-deprecated")]
    #[doc(hidden)]
    pub fn update(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.inner.update(poll, token, interest, opts)
    }

    #[deprecated(since = "0.6.5", note = "use `Poll::deregister` instead")]
    #[cfg(feature = "with-deprecated")]
    #[doc(hidden)]
    pub fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.inner.update(poll, Token(0), Ready::empty(), PollOpt::empty())
    }
}

impl Evented for Registration {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.inner.update(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.inner.update(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.inner.update(poll, Token(0), Ready::empty(), PollOpt::empty())
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // `flag_as_dropped` toggles the `dropped` flag and notifies
        // `Poll::poll` to release its handle (which is just decrementing
        // the ref count).
        if self.inner.state.flag_as_dropped() {
            // Can't do anything if the queuing fails
            let _ = self.inner.enqueue_with_wakeup();
        }
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Registration")
            .finish()
    }
}

impl SetReadiness {
    /// Returns the registration's current readiness.
    ///
    /// # Note
    ///
    /// There is no guarantee that `readiness` establishes any sort of memory
    /// ordering. Any concurrent data access must be synchronized using another
    /// strategy.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Registration, Ready};
    ///
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// assert!(set_readiness.readiness().is_empty());
    ///
    /// set_readiness.set_readiness(Ready::readable())?;
    /// assert!(set_readiness.readiness().is_readable());
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    pub fn readiness(&self) -> Ready {
        self.inner.readiness()
    }

    /// Set the registration's readiness
    ///
    /// If the associated `Registration` is registered with a [`Poll`] instance
    /// and has requested readiness events that include `ready`, then a future
    /// call to [`Poll::poll`] will receive a readiness event representing the
    /// readiness state change.
    ///
    /// # Note
    ///
    /// There is no guarantee that `readiness` establishes any sort of memory
    /// ordering. Any concurrent data access must be synchronized using another
    /// strategy.
    ///
    /// There is also no guarantee as to when the readiness event will be
    /// delivered to poll. A best attempt will be made to make the delivery in a
    /// "timely" fashion. For example, the following is **not** guaranteed to
    /// work:
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Events, Registration, Ready, Poll, PollOpt, Token};
    ///
    /// let poll = Poll::new()?;
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// poll.register(&registration,
    ///               Token(0),
    ///               Ready::readable(),
    ///               PollOpt::edge())?;
    ///
    /// // Set the readiness, then immediately poll to try to get the readiness
    /// // event
    /// set_readiness.set_readiness(Ready::readable())?;
    ///
    /// let mut events = Events::with_capacity(1024);
    /// poll.poll(&mut events, None)?;
    ///
    /// // There is NO guarantee that the following will work. It is possible
    /// // that the readiness event will be delivered at a later time.
    /// let event = events.get(0).unwrap();
    /// assert_eq!(event.token(), Token(0));
    /// assert!(event.readiness().is_readable());
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// # Examples
    ///
    /// A simple example, for a more elaborate example, see the [`Evented`]
    /// documentation.
    ///
    /// ```
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// use mio::{Registration, Ready};
    ///
    /// let (registration, set_readiness) = Registration::new2();
    ///
    /// assert!(set_readiness.readiness().is_empty());
    ///
    /// set_readiness.set_readiness(Ready::readable())?;
    /// assert!(set_readiness.readiness().is_readable());
    /// #     Ok(())
    /// # }
    /// #
    /// # fn main() {
    /// #     try_main().unwrap();
    /// # }
    /// ```
    ///
    /// [`Registration`]: struct.Registration.html
    /// [`Evented`]: event/trait.Evented.html#examples
    /// [`Poll`]: struct.Poll.html
    /// [`Poll::poll`]: struct.Poll.html#method.poll
    pub fn set_readiness(&self, ready: Ready) -> io::Result<()> {
        self.inner.set_readiness(ready)
    }
}

impl fmt::Debug for SetReadiness {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SetReadiness")
            .finish()
    }
}

impl RegistrationInner {
    /// Get the registration's readiness.
    fn readiness(&self) -> Ready {
        self.state.load(Relaxed).readiness()
    }

    /// Set the registration's readiness.
    ///
    /// This function can be called concurrently by an arbitrary number of
    /// SetReadiness handles.
    fn set_readiness(&self, ready: Ready) -> io::Result<()> {
        // Load the current atomic state.
        let mut state = self.state.load(Acquire);
        let mut next;

        loop {
            next = state;

            if state.is_dropped() {
                // Node is dropped, no more notifications
                return Ok(());
            }

            // Update the readiness
            next.set_readiness(ready);

            // If the readiness is not blank, try to obtain permission to
            // push the node into the readiness queue.
            if !next.effective_readiness().is_empty() {
                next.set_queued();
            }

            let actual = self.state.compare_and_swap(state, next, AcqRel);

            if state == actual {
                break;
            }

            state = actual;
        }

        if !state.is_queued() && next.is_queued() {
            // We toggled the queued flag, making us responsible for queuing the
            // node in the MPSC readiness queue.
            self.enqueue_with_wakeup()?;
        }

        Ok(())
    }

    /// Update the registration details associated with the node
    fn update(&self, poll: &Poll, token: Token, interest: Ready, opt: PollOpt) -> io::Result<()> {
        // First, ensure poll instances match
        //
        // Load the queue pointer, `Relaxed` is sufficient here as only the
        // pointer is being operated on. The actual memory is guaranteed to be
        // visible the `poll: &Poll` ref passed as an argument to the function.
        let mut queue = self.readiness_queue.load(Relaxed);
        let other: &*mut () = unsafe {
            &*(&poll.readiness_queue.inner as *const _ as *const *mut ())
        };
        let other = *other;

        debug_assert!(mem::size_of::<Arc<ReadinessQueueInner>>() == mem::size_of::<*mut ()>());

        if queue.is_null() {
            // Attempt to set the queue pointer. `Release` ordering synchronizes
            // with `Acquire` in `ensure_with_wakeup`.
            let actual = self.readiness_queue.compare_and_swap(
                queue, other, Release);

            if actual.is_null() {
                // The CAS succeeded, this means that the node's ref count
                // should be incremented to reflect that the `poll` function
                // effectively owns the node as well.
                //
                // `Relaxed` ordering used for the same reason as in
                // RegistrationInner::clone
                self.ref_count.fetch_add(1, Relaxed);

                // Note that the `queue` reference stored in our
                // `readiness_queue` field is intended to be a strong reference,
                // so now that we've successfully claimed the reference we bump
                // the refcount here.
                //
                // Down below in `release_node` when we deallocate this
                // `RegistrationInner` is where we'll transmute this back to an
                // arc and decrement the reference count.
                mem::forget(poll.readiness_queue.clone());
            } else {
                // The CAS failed, another thread set the queue pointer, so ensure
                // that the pointer and `other` match
                if actual != other {
                    return Err(io::Error::new(io::ErrorKind::Other, "registration handle associated with another `Poll` instance"));
                }
            }

            queue = other;
        } else if queue != other {
            return Err(io::Error::new(io::ErrorKind::Other, "registration handle associated with another `Poll` instance"));
        }

        unsafe {
            let actual = &poll.readiness_queue.inner as *const _ as *const usize;
            debug_assert_eq!(queue as usize, *actual);
        }

        // The `update_lock` atomic is used as a flag ensuring only a single
        // thread concurrently enters the `update` critical section. Any
        // concurrent calls to update are discarded. If coordinated updates are
        // required, the Mio user is responsible for handling that.
        //
        // Acquire / Release ordering is used on `update_lock` to ensure that
        // data access to the `token_*` variables are scoped to the critical
        // section.

        // Acquire the update lock.
        if self.update_lock.compare_and_swap(false, true, Acquire) {
            // The lock is already held. Discard the update
            return Ok(());
        }

        // Relaxed ordering is acceptable here as the only memory that needs to
        // be visible as part of the update are the `token_*` variables, and
        // ordering has already been handled by the `update_lock` access.
        let mut state = self.state.load(Relaxed);
        let mut next;

        // Read the current token, again this memory has been ordered by the
        // acquire on `update_lock`.
        let curr_token_pos = state.token_write_pos();
        let curr_token = unsafe { self::token(self, curr_token_pos) };

        let mut next_token_pos = curr_token_pos;

        // If the `update` call is changing the token, then compute the next
        // available token slot and write the token there.
        //
        // Note that this computation is happening *outside* of the
        // compare-and-swap loop. The update lock ensures that only a single
        // thread could be mutating the write_token_position, so the
        // `next_token_pos` will never need to be recomputed even if
        // `token_read_pos` concurrently changes. This is because
        // `token_read_pos` can ONLY concurrently change to the current value of
        // `token_write_pos`, so `next_token_pos` will always remain valid.
        if token != curr_token {
            next_token_pos = state.next_token_pos();

            // Update the token
            match next_token_pos {
                0 => unsafe { *self.token_0.get() = token },
                1 => unsafe { *self.token_1.get() = token },
                2 => unsafe { *self.token_2.get() = token },
                _ => unreachable!(),
            }
        }

        // Now enter the compare-and-swap loop
        loop {
            next = state;

            // The node is only dropped once all `Registration` handles are
            // dropped. Only `Registration` can call `update`.
            debug_assert!(!state.is_dropped());

            // Update the write token position, this will also release the token
            // to Poll::poll.
            next.set_token_write_pos(next_token_pos);

            // Update readiness and poll opts
            next.set_interest(interest);
            next.set_poll_opt(opt);

            // If there is effective readiness, the node will need to be queued
            // for processing. This exact behavior is still TBD, so we are
            // conservative for now and always fire.
            //
            // See https://github.com/carllerche/mio/issues/535.
            if !next.effective_readiness().is_empty() {
                next.set_queued();
            }

            // compare-and-swap the state values. Only `Release` is needed here.
            // The `Release` ensures that `Poll::poll` will see the token
            // update and the update function doesn't care about any other
            // memory visibility.
            let actual = self.state.compare_and_swap(state, next, Release);

            if actual == state {
                break;
            }

            // CAS failed, but `curr_token_pos` should not have changed given
            // that we still hold the update lock.
            debug_assert_eq!(curr_token_pos, actual.token_write_pos());

            state = actual;
        }

        // Release the lock
        self.update_lock.store(false, Release);

        if !state.is_queued() && next.is_queued() {
            // We are responsible for enqueing the node.
            enqueue_with_wakeup(queue, self)?;
        }

        Ok(())
    }
}

impl ops::Deref for RegistrationInner {
    type Target = ReadinessNode;

    fn deref(&self) -> &ReadinessNode {
        unsafe { &*self.node }
    }
}

impl Clone for RegistrationInner {
    fn clone(&self) -> RegistrationInner {
        // Using a relaxed ordering is alright here, as knowledge of the
        // original reference prevents other threads from erroneously deleting
        // the object.
        //
        // As explained in the [Boost documentation][1], Increasing the
        // reference counter can always be done with memory_order_relaxed: New
        // references to an object can only be formed from an existing
        // reference, and passing an existing reference from one thread to
        // another must already provide any required synchronization.
        //
        // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
        let old_size = self.ref_count.fetch_add(1, Relaxed);

        // However we need to guard against massive refcounts in case someone
        // is `mem::forget`ing Arcs. If we don't do this the count can overflow
        // and users will use-after free. We racily saturate to `isize::MAX` on
        // the assumption that there aren't ~2 billion threads incrementing
        // the reference count at once. This branch will never be taken in
        // any realistic program.
        //
        // We abort because such a program is incredibly degenerate, and we
        // don't care to support it.
        if old_size & !MAX_REFCOUNT != 0 {
            process::abort();
        }

        RegistrationInner {
            node: self.node,
        }
    }
}

impl Drop for RegistrationInner {
    fn drop(&mut self) {
        // Only handles releasing from `Registration` and `SetReadiness`
        // handles. Poll has to call this itself.
        release_node(self.node);
    }
}

/*
 *
 * ===== ReadinessQueue =====
 *
 */

impl ReadinessQueue {
    /// Create a new `ReadinessQueue`.
    fn new() -> io::Result<ReadinessQueue> {
        is_send::<Self>();
        is_sync::<Self>();

        let end_marker = Box::new(ReadinessNode::marker());
        let sleep_marker = Box::new(ReadinessNode::marker());
        let closed_marker = Box::new(ReadinessNode::marker());

        let ptr = &*end_marker as *const _ as *mut _;

        Ok(ReadinessQueue {
            inner: Arc::new(ReadinessQueueInner {
                awakener: sys::Awakener::new()?,
                head_readiness: AtomicPtr::new(ptr),
                tail_readiness: UnsafeCell::new(ptr),
                end_marker,
                sleep_marker,
                closed_marker,
            })
        })
    }

    /// Poll the queue for new events
    fn poll(&self, dst: &mut sys::Events) {
        // `until` is set with the first node that gets re-enqueued due to being
        // set to have level-triggered notifications. This prevents an infinite
        // loop where `Poll::poll` will keep dequeuing nodes it enqueues.
        let mut until = ptr::null_mut();

        if dst.len() == dst.capacity() {
            // If `dst` is already full, the readiness queue won't be drained.
            // This might result in `sleep_marker` staying in the queue and
            // unecessary pipe writes occuring.
            self.inner.clear_sleep_marker();
        }

        'outer:
        while dst.len() < dst.capacity() {
            // Dequeue a node. If the queue is in an inconsistent state, then
            // stop polling. `Poll::poll` will be called again shortly and enter
            // a syscall, which should be enough to enable the other thread to
            // finish the queuing process.
            let ptr = match unsafe { self.inner.dequeue_node(until) } {
                Dequeue::Empty | Dequeue::Inconsistent => break,
                Dequeue::Data(ptr) => ptr,
            };

            let node = unsafe { &*ptr };

            // Read the node state with Acquire ordering. This allows reading
            // the token variables.
            let mut state = node.state.load(Acquire);
            let mut next;
            let mut readiness;
            let mut opt;

            loop {
                // Build up any changes to the readiness node's state and
                // attempt the CAS at the end
                next = state;

                // Given that the node was just read from the queue, the
                // `queued` flag should still be set.
                debug_assert!(state.is_queued());

                // The dropped flag means we need to release the node and
                // perform no further processing on it.
                if state.is_dropped() {
                    // Release the node and continue
                    release_node(ptr);
                    continue 'outer;
                }

                // Process the node
                readiness = state.effective_readiness();
                opt = state.poll_opt();

                if opt.is_edge() {
                    // Mark the node as dequeued
                    next.set_dequeued();

                    if opt.is_oneshot() && !readiness.is_empty() {
                        next.disarm();
                    }
                } else if readiness.is_empty() {
                    next.set_dequeued();
                }

                // Ensure `token_read_pos` is set to `token_write_pos` so that
                // we read the most up to date token value.
                next.update_token_read_pos();

                if state == next {
                    break;
                }

                let actual = node.state.compare_and_swap(state, next, AcqRel);

                if actual == state {
                    break;
                }

                state = actual;
            }

            // If the queued flag is still set, then the node must be requeued.
            // This typically happens when using level-triggered notifications.
            if next.is_queued() {
                if until.is_null() {
                    // We never want to see the node again
                    until = ptr;
                }

                // Requeue the node
                self.inner.enqueue_node(node);
            }

            if !readiness.is_empty() {
                // Get the token
                let token = unsafe { token(node, next.token_read_pos()) };

                // Push the event
                dst.push_event(Event::new(readiness, token));
            }
        }
    }

    /// Prepare the queue for the `Poll::poll` thread to block in the system
    /// selector. This involves changing `head_readiness` to `sleep_marker`.
    /// Returns true if successful and `poll` can block.
    fn prepare_for_sleep(&self) -> bool {
        let end_marker = self.inner.end_marker();
        let sleep_marker = self.inner.sleep_marker();

        let tail = unsafe { *self.inner.tail_readiness.get() };

        // If the tail is currently set to the sleep_marker, then check if the
        // head is as well. If it is, then the queue is currently ready to
        // sleep. If it is not, then the queue is not empty and there should be
        // no sleeping.
        if tail == sleep_marker {
            return self.inner.head_readiness.load(Acquire) == sleep_marker;
        }

        // If the tail is not currently set to `end_marker`, then the queue is
        // not empty.
        if tail != end_marker {
            return false;
        }

        // The sleep marker is *not* currently in the readiness queue.
        //
        // The sleep marker is only inserted in this function. It is also only
        // inserted in the tail position. This is guaranteed by first checking
        // that the end marker is in the tail position, pushing the sleep marker
        // after the end marker, then removing the end marker.
        //
        // Before inserting a node into the queue, the next pointer has to be
        // set to null. Again, this is only safe to do when the node is not
        // currently in the queue, but we already have ensured this.
        self.inner.sleep_marker.next_readiness.store(ptr::null_mut(), Relaxed);

        let actual = self.inner.head_readiness.compare_and_swap(
            end_marker, sleep_marker, AcqRel);

        debug_assert!(actual != sleep_marker);

        if actual != end_marker {
            // The readiness queue is not empty
            return false;
        }

        // The current tail should be pointing to `end_marker`
        debug_assert!(unsafe { *self.inner.tail_readiness.get() == end_marker });
        // The `end_marker` next pointer should be null
        debug_assert!(self.inner.end_marker.next_readiness.load(Relaxed).is_null());

        // Update tail pointer.
        unsafe { *self.inner.tail_readiness.get() = sleep_marker; }
        true
    }
}

impl Drop for ReadinessQueue {
    fn drop(&mut self) {
        // Close the queue by enqueuing the closed node
        self.inner.enqueue_node(&*self.inner.closed_marker);

        loop {
            // Free any nodes that happen to be left in the readiness queue
            let ptr = match unsafe { self.inner.dequeue_node(ptr::null_mut()) } {
                Dequeue::Empty => break,
                Dequeue::Inconsistent => {
                    // This really shouldn't be possible as all other handles to
                    // `ReadinessQueueInner` are dropped, but handle this by
                    // spinning I guess?
                    continue;
                }
                Dequeue::Data(ptr) => ptr,
            };

            let node = unsafe { &*ptr };

            let state = node.state.load(Acquire);

            debug_assert!(state.is_queued());

            release_node(ptr);
        }
    }
}

impl ReadinessQueueInner {
    fn wakeup(&self) -> io::Result<()> {
        self.awakener.wakeup()
    }

    /// Prepend the given node to the head of the readiness queue. This is done
    /// with relaxed ordering. Returns true if `Poll` needs to be woken up.
    fn enqueue_node_with_wakeup(&self, node: &ReadinessNode) -> io::Result<()> {
        if self.enqueue_node(node) {
            self.wakeup()?;
        }

        Ok(())
    }

    /// Push the node into the readiness queue
    fn enqueue_node(&self, node: &ReadinessNode) -> bool {
        // This is the 1024cores.net intrusive MPSC queue [1] "push" function.
        let node_ptr = node as *const _ as *mut _;

        // Relaxed used as the ordering is "released" when swapping
        // `head_readiness`
        node.next_readiness.store(ptr::null_mut(), Relaxed);

        unsafe {
            let mut prev = self.head_readiness.load(Acquire);

            loop {
                if prev == self.closed_marker() {
                    debug_assert!(node_ptr != self.closed_marker());
                    // debug_assert!(node_ptr != self.end_marker());
                    debug_assert!(node_ptr != self.sleep_marker());

                    if node_ptr != self.end_marker() {
                        // The readiness queue is shutdown, but the enqueue flag was
                        // set. This means that we are responsible for decrementing
                        // the ready queue's ref count
                        debug_assert!(node.ref_count.load(Relaxed) >= 2);
                        release_node(node_ptr);
                    }

                    return false;
                }

                let act = self.head_readiness.compare_and_swap(prev, node_ptr, AcqRel);

                if prev == act {
                    break;
                }

                prev = act;
            }

            debug_assert!((*prev).next_readiness.load(Relaxed).is_null());

            (*prev).next_readiness.store(node_ptr, Release);

            prev == self.sleep_marker()
        }
    }

    fn clear_sleep_marker(&self) {
        let end_marker = self.end_marker();
        let sleep_marker = self.sleep_marker();

        unsafe {
            let tail = *self.tail_readiness.get();

            if tail != self.sleep_marker() {
                return;
            }

            // The empty markeer is *not* currently in the readiness queue
            // (since the sleep markeris).
            self.end_marker.next_readiness.store(ptr::null_mut(), Relaxed);

            let actual = self.head_readiness.compare_and_swap(
                sleep_marker, end_marker, AcqRel);

            debug_assert!(actual != end_marker);

            if actual != sleep_marker {
                // The readiness queue is not empty, we cannot remove the sleep
                // markeer
                return;
            }

            // Update the tail pointer.
            *self.tail_readiness.get() = end_marker;
        }
    }

    /// Must only be called in `poll` or `drop`
    unsafe fn dequeue_node(&self, until: *mut ReadinessNode) -> Dequeue {
        // This is the 1024cores.net intrusive MPSC queue [1] "pop" function
        // with the modifications mentioned at the top of the file.
        let mut tail = *self.tail_readiness.get();
        let mut next = (*tail).next_readiness.load(Acquire);

        if tail == self.end_marker() || tail == self.sleep_marker() || tail == self.closed_marker() {
            if next.is_null() {
                // Make sure the sleep marker is removed (as we are no longer
                // sleeping
                self.clear_sleep_marker();

                return Dequeue::Empty;
            }

            *self.tail_readiness.get() = next;
            tail = next;
            next = (*next).next_readiness.load(Acquire);
        }

        // Only need to check `until` at this point. `until` is either null,
        // which will never match tail OR it is a node that was pushed by
        // the current thread. This means that either:
        //
        // 1) The queue is inconsistent, which is handled explicitly
        // 2) We encounter `until` at this point in dequeue
        // 3) we will pop a different node
        if tail == until {
            return Dequeue::Empty;
        }

        if !next.is_null() {
            *self.tail_readiness.get() = next;
            return Dequeue::Data(tail);
        }

        if self.head_readiness.load(Acquire) != tail {
            return Dequeue::Inconsistent;
        }

        // Push the stub node
        self.enqueue_node(&*self.end_marker);

        next = (*tail).next_readiness.load(Acquire);

        if !next.is_null() {
            *self.tail_readiness.get() = next;
            return Dequeue::Data(tail);
        }

        Dequeue::Inconsistent
    }

    fn end_marker(&self) -> *mut ReadinessNode {
        &*self.end_marker as *const ReadinessNode as *mut ReadinessNode
    }

    fn sleep_marker(&self) -> *mut ReadinessNode {
        &*self.sleep_marker as *const ReadinessNode as *mut ReadinessNode
    }

    fn closed_marker(&self) -> *mut ReadinessNode {
        &*self.closed_marker as *const ReadinessNode as *mut ReadinessNode
    }
}

impl ReadinessNode {
    /// Return a new `ReadinessNode`, initialized with a ref_count of 3.
    fn new(queue: *mut (),
           token: Token,
           interest: Ready,
           opt: PollOpt,
           ref_count: usize) -> ReadinessNode
    {
        ReadinessNode {
            state: AtomicState::new(interest, opt),
            // Only the first token is set, the others are initialized to 0
            token_0: UnsafeCell::new(token),
            token_1: UnsafeCell::new(Token(0)),
            token_2: UnsafeCell::new(Token(0)),
            next_readiness: AtomicPtr::new(ptr::null_mut()),
            update_lock: AtomicBool::new(false),
            readiness_queue: AtomicPtr::new(queue),
            ref_count: AtomicUsize::new(ref_count),
        }
    }

    fn marker() -> ReadinessNode {
        ReadinessNode {
            state: AtomicState::new(Ready::empty(), PollOpt::empty()),
            token_0: UnsafeCell::new(Token(0)),
            token_1: UnsafeCell::new(Token(0)),
            token_2: UnsafeCell::new(Token(0)),
            next_readiness: AtomicPtr::new(ptr::null_mut()),
            update_lock: AtomicBool::new(false),
            readiness_queue: AtomicPtr::new(ptr::null_mut()),
            ref_count: AtomicUsize::new(0),
        }
    }

    fn enqueue_with_wakeup(&self) -> io::Result<()> {
        let queue = self.readiness_queue.load(Acquire);

        if queue.is_null() {
            // Not associated with a queue, nothing to do
            return Ok(());
        }

        enqueue_with_wakeup(queue, self)
    }
}

fn enqueue_with_wakeup(queue: *mut (), node: &ReadinessNode) -> io::Result<()> {
    debug_assert!(!queue.is_null());
    // This is ugly... but we don't want to bump the ref count.
    let queue: &Arc<ReadinessQueueInner> = unsafe {
        &*(&queue as *const *mut () as *const Arc<ReadinessQueueInner>)
    };
    queue.enqueue_node_with_wakeup(node)
}

unsafe fn token(node: &ReadinessNode, pos: usize) -> Token {
    match pos {
        0 => *node.token_0.get(),
        1 => *node.token_1.get(),
        2 => *node.token_2.get(),
        _ => unreachable!(),
    }
}

fn release_node(ptr: *mut ReadinessNode) {
    unsafe {
        // `AcqRel` synchronizes with other `release_node` functions and ensures
        // that the drop happens after any reads / writes on other threads.
        if (*ptr).ref_count.fetch_sub(1, AcqRel) != 1 {
            return;
        }

        let node = Box::from_raw(ptr);

        // Decrement the readiness_queue Arc
        let queue = node.readiness_queue.load(Acquire);

        if queue.is_null() {
            return;
        }

        let _: Arc<ReadinessQueueInner> = mem::transmute(queue);
    }
}

impl AtomicState {
    fn new(interest: Ready, opt: PollOpt) -> AtomicState {
        let state = ReadinessState::new(interest, opt);

        AtomicState {
            inner: AtomicUsize::new(state.into()),
        }
    }

    /// Loads the current `ReadinessState`
    fn load(&self, order: Ordering) -> ReadinessState {
        self.inner.load(order).into()
    }

    /// Stores a state if the current state is the same as `current`.
    fn compare_and_swap(&self, current: ReadinessState, new: ReadinessState, order: Ordering) -> ReadinessState {
        self.inner.compare_and_swap(current.into(), new.into(), order).into()
    }

    // Returns `true` if the node should be queued
    fn flag_as_dropped(&self) -> bool {
        let prev: ReadinessState = self.inner.fetch_or(DROPPED_MASK | QUEUED_MASK, Release).into();
        // The flag should not have been previously set
        debug_assert!(!prev.is_dropped());

        !prev.is_queued()
    }
}

impl ReadinessState {
    // Create a `ReadinessState` initialized with the provided arguments
    #[inline]
    fn new(interest: Ready, opt: PollOpt) -> ReadinessState {
        let interest = event::ready_as_usize(interest);
        let opt = event::opt_as_usize(opt);

        debug_assert!(interest <= MASK_4);
        debug_assert!(opt <= MASK_4);

        let mut val = interest << INTEREST_SHIFT;
        val |= opt << POLL_OPT_SHIFT;

        ReadinessState(val)
    }

    #[inline]
    fn get(self, mask: usize, shift: usize) -> usize{
        (self.0 >> shift) & mask
    }

    #[inline]
    fn set(&mut self, val: usize, mask: usize, shift: usize) {
        self.0 = (self.0 & !(mask << shift)) | (val << shift)
    }

    /// Get the readiness
    #[inline]
    fn readiness(self) -> Ready {
        let v = self.get(MASK_4, READINESS_SHIFT);
        event::ready_from_usize(v)
    }

    #[inline]
    fn effective_readiness(self) -> Ready {
        self.readiness() & self.interest()
    }

    /// Set the readiness
    #[inline]
    fn set_readiness(&mut self, v: Ready) {
        self.set(event::ready_as_usize(v), MASK_4, READINESS_SHIFT);
    }

    /// Get the interest
    #[inline]
    fn interest(self) -> Ready {
        let v = self.get(MASK_4, INTEREST_SHIFT);
        event::ready_from_usize(v)
    }

    /// Set the interest
    #[inline]
    fn set_interest(&mut self, v: Ready) {
        self.set(event::ready_as_usize(v), MASK_4, INTEREST_SHIFT);
    }

    #[inline]
    fn disarm(&mut self) {
        self.set_interest(Ready::empty());
    }

    /// Get the poll options
    #[inline]
    fn poll_opt(self) -> PollOpt {
        let v = self.get(MASK_4, POLL_OPT_SHIFT);
        event::opt_from_usize(v)
    }

    /// Set the poll options
    #[inline]
    fn set_poll_opt(&mut self, v: PollOpt) {
        self.set(event::opt_as_usize(v), MASK_4, POLL_OPT_SHIFT);
    }

    #[inline]
    fn is_queued(self) -> bool {
        self.0 & QUEUED_MASK == QUEUED_MASK
    }

    /// Set the queued flag
    #[inline]
    fn set_queued(&mut self) {
        // Dropped nodes should never be queued
        debug_assert!(!self.is_dropped());
        self.0 |= QUEUED_MASK;
    }

    #[inline]
    fn set_dequeued(&mut self) {
        debug_assert!(self.is_queued());
        self.0 &= !QUEUED_MASK
    }

    #[inline]
    fn is_dropped(self) -> bool {
        self.0 & DROPPED_MASK == DROPPED_MASK
    }

    #[inline]
    fn token_read_pos(self) -> usize {
        self.get(MASK_2, TOKEN_RD_SHIFT)
    }

    #[inline]
    fn token_write_pos(self) -> usize {
        self.get(MASK_2, TOKEN_WR_SHIFT)
    }

    #[inline]
    fn next_token_pos(self) -> usize {
        let rd = self.token_read_pos();
        let wr = self.token_write_pos();

        match wr {
            0 => {
                match rd {
                    1 => 2,
                    2 => 1,
                    0 => 1,
                    _ => unreachable!(),
                }
            }
            1 => {
                match rd {
                    0 => 2,
                    2 => 0,
                    1 => 2,
                    _ => unreachable!(),
                }
            }
            2 => {
                match rd {
                    0 => 1,
                    1 => 0,
                    2 => 0,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    #[inline]
    fn set_token_write_pos(&mut self, val: usize) {
        self.set(val, MASK_2, TOKEN_WR_SHIFT);
    }

    #[inline]
    fn update_token_read_pos(&mut self) {
        let val = self.token_write_pos();
        self.set(val, MASK_2, TOKEN_RD_SHIFT);
    }
}

impl From<ReadinessState> for usize {
    fn from(src: ReadinessState) -> usize {
        src.0
    }
}

impl From<usize> for ReadinessState {
    fn from(src: usize) -> ReadinessState {
        ReadinessState(src)
    }
}

fn is_send<T: Send>() {}
fn is_sync<T: Sync>() {}

impl SelectorId {
    pub fn new() -> SelectorId {
        SelectorId {
            id: AtomicUsize::new(0),
        }
    }

    pub fn associate_selector(&self, poll: &Poll) -> io::Result<()> {
        let selector_id = self.id.load(Ordering::SeqCst);

        if selector_id != 0 && selector_id != poll.selector.id() {
            Err(io::Error::new(io::ErrorKind::Other, "socket already registered"))
        } else {
            self.id.store(poll.selector.id(), Ordering::SeqCst);
            Ok(())
        }
    }
}

impl Clone for SelectorId {
    fn clone(&self) -> SelectorId {
        SelectorId {
            id: AtomicUsize::new(self.id.load(Ordering::SeqCst)),
        }
    }
}

#[test]
#[cfg(all(unix, not(target_os = "fuchsia")))]
pub fn as_raw_fd() {
    let poll = Poll::new().unwrap();
    assert!(poll.as_raw_fd() > 0);
}
