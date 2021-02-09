use {channel, Poll, Events, Token};
use event::Evented;
use deprecated::{Handler, NotifyError};
use event_imp::{Event, Ready, PollOpt};
use timer::{self, Timer, Timeout};
use std::{io, fmt, usize};
use std::default::Default;
use std::time::Duration;

#[derive(Debug, Default, Clone)]
pub struct EventLoopBuilder {
    config: Config,
}

/// `EventLoop` configuration details
#[derive(Clone, Debug)]
struct Config {
    // == Notifications ==
    notify_capacity: usize,
    messages_per_tick: usize,

    // == Timer ==
    timer_tick: Duration,
    timer_wheel_size: usize,
    timer_capacity: usize,
}

impl Default for Config {
    fn default() -> Config {
        // Default EventLoop configuration values
        Config {
            notify_capacity: 4_096,
            messages_per_tick: 256,
            timer_tick: Duration::from_millis(100),
            timer_wheel_size: 1_024,
            timer_capacity: 65_536,
        }
    }
}

impl EventLoopBuilder {
    /// Construct a new `EventLoopBuilder` with the default configuration
    /// values.
    pub fn new() -> EventLoopBuilder {
        EventLoopBuilder::default()
    }

    /// Sets the maximum number of messages that can be buffered on the event
    /// loop's notification channel before a send will fail.
    ///
    /// The default value for this is 4096.
    pub fn notify_capacity(&mut self, capacity: usize) -> &mut Self {
        self.config.notify_capacity = capacity;
        self
    }

    /// Sets the maximum number of messages that can be processed on any tick of
    /// the event loop.
    ///
    /// The default value for this is 256.
    pub fn messages_per_tick(&mut self, messages: usize) -> &mut Self {
        self.config.messages_per_tick = messages;
        self
    }

    pub fn timer_tick(&mut self, val: Duration) -> &mut Self {
        self.config.timer_tick = val;
        self
    }

    pub fn timer_wheel_size(&mut self, size: usize) -> &mut Self {
        self.config.timer_wheel_size = size;
        self
    }

    pub fn timer_capacity(&mut self, cap: usize) -> &mut Self {
        self.config.timer_capacity = cap;
        self
    }

    /// Constructs a new `EventLoop` using the configured values. The
    /// `EventLoop` will not be running.
    pub fn build<H: Handler>(self) -> io::Result<EventLoop<H>> {
        EventLoop::configured(self.config)
    }
}

/// Single threaded IO event loop.
pub struct EventLoop<H: Handler> {
    run: bool,
    poll: Poll,
    events: Events,
    timer: Timer<H::Timeout>,
    notify_tx: channel::SyncSender<H::Message>,
    notify_rx: channel::Receiver<H::Message>,
    config: Config,
}

// Token used to represent notifications
const NOTIFY: Token = Token(usize::MAX - 1);
const TIMER: Token = Token(usize::MAX - 2);

impl<H: Handler> EventLoop<H> {

    /// Constructs a new `EventLoop` using the default configuration values.
    /// The `EventLoop` will not be running.
    pub fn new() -> io::Result<EventLoop<H>> {
        EventLoop::configured(Config::default())
    }

    fn configured(config: Config) -> io::Result<EventLoop<H>> {
        // Create the IO poller
        let poll = Poll::new()?;

        let timer = timer::Builder::default()
            .tick_duration(config.timer_tick)
            .num_slots(config.timer_wheel_size)
            .capacity(config.timer_capacity)
            .build();

        // Create cross thread notification queue
        let (tx, rx) = channel::sync_channel(config.notify_capacity);

        // Register the notification wakeup FD with the IO poller
        poll.register(&rx, NOTIFY, Ready::readable(), PollOpt::edge() | PollOpt::oneshot())?;
        poll.register(&timer, TIMER, Ready::readable(), PollOpt::edge())?;

        Ok(EventLoop {
            run: true,
            poll,
            timer,
            notify_tx: tx,
            notify_rx: rx,
            config,
            events: Events::with_capacity(1024),
        })
    }

    /// Returns a sender that allows sending messages to the event loop in a
    /// thread-safe way, waking up the event loop if needed.
    ///
    /// # Implementation Details
    ///
    /// Each [EventLoop](#) contains a lock-free queue with a pre-allocated
    /// buffer size. The size can be changed by modifying
    /// [EventLoopConfig.notify_capacity](struct.EventLoopConfig.html#method.notify_capacity).
    /// When a message is sent to the EventLoop, it is first pushed on to the
    /// queue. Then, if the EventLoop is currently running, an atomic flag is
    /// set to indicate that the next loop iteration should be started without
    /// waiting.
    ///
    /// If the loop is blocked waiting for IO events, then it is woken up. The
    /// strategy for waking up the event loop is platform dependent. For
    /// example, on a modern Linux OS, eventfd is used. On older OSes, a pipe
    /// is used.
    ///
    /// The strategy of setting an atomic flag if the event loop is not already
    /// sleeping allows avoiding an expensive wakeup operation if at all possible.
    pub fn channel(&self) -> Sender<H::Message> {
        Sender::new(self.notify_tx.clone())
    }

    /// Schedules a timeout after the requested time interval. When the
    /// duration has been reached,
    /// [Handler::timeout](trait.Handler.html#method.timeout) will be invoked
    /// passing in the supplied token.
    ///
    /// Returns a handle to the timeout that can be used to cancel the timeout
    /// using [#clear_timeout](#method.clear_timeout).
    pub fn timeout(&mut self, token: H::Timeout, delay: Duration) -> timer::Result<Timeout> {
        self.timer.set_timeout(delay, token)
    }

    /// If the supplied timeout has not been triggered, cancel it such that it
    /// will not be triggered in the future.
    pub fn clear_timeout(&mut self, timeout: &Timeout) -> bool {
        self.timer.cancel_timeout(&timeout).is_some()
    }

    /// Tells the event loop to exit after it is done handling all events in the
    /// current iteration.
    pub fn shutdown(&mut self) {
        self.run = false;
    }

    /// Indicates whether the event loop is currently running. If it's not it has either
    /// stopped or is scheduled to stop on the next tick.
    pub fn is_running(&self) -> bool {
        self.run
    }

    /// Registers an IO handle with the event loop.
    pub fn register<E: ?Sized>(&mut self, io: &E, token: Token, interest: Ready, opt: PollOpt) -> io::Result<()>
        where E: Evented
    {
        self.poll.register(io, token, interest, opt)
    }

    /// Re-Registers an IO handle with the event loop.
    pub fn reregister<E: ?Sized>(&mut self, io: &E, token: Token, interest: Ready, opt: PollOpt) -> io::Result<()>
        where E: Evented
    {
        self.poll.reregister(io, token, interest, opt)
    }

    /// Keep spinning the event loop indefinitely, and notify the handler whenever
    /// any of the registered handles are ready.
    pub fn run(&mut self, handler: &mut H) -> io::Result<()> {
        self.run = true;

        while self.run {
            // Execute ticks as long as the event loop is running
            self.run_once(handler, None)?;
        }

        Ok(())
    }

    /// Deregisters an IO handle with the event loop.
    ///
    /// Both kqueue and epoll will automatically clear any pending events when closing a
    /// file descriptor (socket). In that case, this method does not need to be called
    /// prior to dropping a connection from the slab.
    ///
    /// Warning: kqueue effectively builds in deregister when using edge-triggered mode with
    /// oneshot. Calling `deregister()` on the socket will cause a TcpStream error.
    pub fn deregister<E: ?Sized>(&mut self, io: &E) -> io::Result<()> where E: Evented {
        self.poll.deregister(io)
    }

    /// Spin the event loop once, with a given timeout (forever if `None`),
    /// and notify the handler if any of the registered handles become ready
    /// during that time.
    pub fn run_once(&mut self, handler: &mut H, timeout: Option<Duration>) -> io::Result<()> {
        trace!("event loop tick");

        // Check the registered IO handles for any new events. Each poll
        // is for one second, so a shutdown request can last as long as
        // one second before it takes effect.
        let events = match self.io_poll(timeout) {
            Ok(e) => e,
            Err(err) => {
                if err.kind() == io::ErrorKind::Interrupted {
                    handler.interrupted(self);
                    0
                } else {
                    return Err(err);
                }
            }
        };

        self.io_process(handler, events);
        handler.tick(self);
        Ok(())
    }

    #[inline]
    fn io_poll(&mut self, timeout: Option<Duration>) -> io::Result<usize> {
        self.poll.poll(&mut self.events, timeout)
    }

    // Process IO events that have been previously polled
    fn io_process(&mut self, handler: &mut H, cnt: usize) {
        let mut i = 0;

        trace!("io_process(..); cnt={}; len={}", cnt, self.events.len());

        // Iterate over the notifications. Each event provides the token
        // it was registered with (which usually represents, at least, the
        // handle that the event is about) as well as information about
        // what kind of event occurred (readable, writable, signal, etc.)
        while i < cnt {
            let evt = self.events.get(i).unwrap();

            trace!("event={:?}; idx={:?}", evt, i);

            match evt.token() {
                NOTIFY => self.notify(handler),
                TIMER => self.timer_process(handler),
                _ => self.io_event(handler, evt)
            }

            i += 1;
        }
    }

    fn io_event(&mut self, handler: &mut H, evt: Event) {
        handler.ready(self, evt.token(), evt.readiness());
    }

    fn notify(&mut self, handler: &mut H) {
        for _ in 0..self.config.messages_per_tick {
            match self.notify_rx.try_recv() {
                Ok(msg) => handler.notify(self, msg),
                _ => break,
            }
        }

        // Re-register
        let _ = self.poll.reregister(&self.notify_rx, NOTIFY, Ready::readable(), PollOpt::edge() | PollOpt::oneshot());
    }

    fn timer_process(&mut self, handler: &mut H) {
        while let Some(t) = self.timer.poll() {
            handler.timeout(self, t);
        }
    }
}

impl<H: Handler> fmt::Debug for EventLoop<H> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("EventLoop")
            .field("run", &self.run)
            .field("poll", &self.poll)
            .field("config", &self.config)
            .finish()
    }
}

/// Sends messages to the EventLoop from other threads.
pub struct Sender<M> {
    tx: channel::SyncSender<M>
}

impl<M> fmt::Debug for Sender<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Sender<?> {{ ... }}")
    }
}

impl<M> Clone for Sender <M> {
    fn clone(&self) -> Sender<M> {
        Sender { tx: self.tx.clone() }
    }
}

impl<M> Sender<M> {
    fn new(tx: channel::SyncSender<M>) -> Sender<M> {
        Sender { tx }
    }

    pub fn send(&self, msg: M) -> Result<(), NotifyError<M>> {
        self.tx.try_send(msg)?;
        Ok(())
    }
}
