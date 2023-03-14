use async_broadcast::{broadcast, InactiveReceiver, Sender as Broadcaster};
use async_channel::{bounded, Receiver, Sender};
use async_executor::Executor;
use async_lock::Mutex;
use async_task::Task;
use event_listener::EventListener;
use once_cell::sync::OnceCell;
use ordered_stream::{OrderedFuture, OrderedStream, PollResult};
use static_assertions::assert_impl_all;
use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    io::{self, ErrorKind},
    ops::Deref,
    pin::Pin,
    sync::{
        self,
        atomic::{AtomicU32, Ordering::SeqCst},
        Arc, Weak,
    },
    task::{Context, Poll},
};
use tracing::{debug, instrument, trace, trace_span, Instrument};
use zbus_names::{BusName, ErrorName, InterfaceName, MemberName, OwnedUniqueName, WellKnownName};
use zvariant::ObjectPath;

use futures_core::{ready, Future};
use futures_sink::Sink;
use futures_util::{
    future::{select, Either},
    sink::SinkExt,
    StreamExt, TryFutureExt,
};

use crate::{
    blocking, fdo,
    raw::{Connection as RawConnection, Socket},
    Authenticated, CacheProperties, ConnectionBuilder, DBusError, Error, Guid, Message,
    MessageStream, MessageType, ObjectServer, Result,
};

const DEFAULT_MAX_QUEUED: usize = 64;

/// Inner state shared by Connection and WeakConnection
#[derive(Debug)]
pub(crate) struct ConnectionInner {
    server_guid: Guid,
    #[cfg(unix)]
    cap_unix_fd: bool,
    bus_conn: bool,
    unique_name: OnceCell<OwnedUniqueName>,
    registered_names: Mutex<HashSet<WellKnownName<'static>>>,

    raw_conn: Arc<sync::Mutex<RawConnection<Box<dyn Socket>>>>,

    // Serial number for next outgoing message
    serial: AtomicU32,

    // Our executor
    executor: Arc<Executor<'static>>,

    // Message receiver task
    #[allow(unused)]
    msg_receiver_task: Task<()>,

    signal_matches: Mutex<HashMap<String, u64>>,

    object_server: OnceCell<blocking::ObjectServer>,
    object_server_dispatch_task: OnceCell<Task<()>>,
}

// FIXME: Should really use [`AsyncDrop`] for `ConnectionInner` when we've something like that to
//        cancel `msg_receiver_task` manually to ensure task is gone before the connection is. Same
//        goes for the registered well-known names.
//
// [`AsyncDrop`]: https://github.com/rust-lang/wg-async-foundations/issues/65

#[derive(Debug)]
struct MessageReceiverTask {
    raw_conn: Arc<sync::Mutex<RawConnection<Box<dyn Socket>>>>,

    // Message broadcaster.
    msg_sender: Broadcaster<Arc<Message>>,

    // Sender side of the error channel
    error_sender: Sender<Error>,
}

impl MessageReceiverTask {
    fn new(
        raw_conn: Arc<sync::Mutex<RawConnection<Box<dyn Socket>>>>,
        msg_sender: Broadcaster<Arc<Message>>,
        error_sender: Sender<Error>,
    ) -> Arc<Self> {
        Arc::new(Self {
            raw_conn,
            msg_sender,
            error_sender,
        })
    }

    fn spawn(self: Arc<Self>, executor: &Executor<'_>) -> Task<()> {
        executor.spawn(async move {
            self.receive_msg().await;
        })
    }

    // Keep receiving messages and put them on the queue.
    #[instrument(skip(self))]
    async fn receive_msg(self: Arc<Self>) {
        loop {
            trace!("Waiting for message on the socket..");
            let receive_msg = ReceiveMessage {
                raw_conn: &self.raw_conn,
            };
            let msg = match receive_msg.await {
                Ok(msg) => msg,
                Err(e) => {
                    trace!("Error reading from the socket: {:?}", e);
                    if let Err(e) = self.error_sender.send(e).await {
                        // This happens if the channel is being dropped, which only happens when the
                        // receive_msg task is running at the time the last Connection is dropped.
                        // So it's unlikely that it'd be interesting to the user. Hence debug not
                        // warn.
                        debug!("Error sending error: {:?}", e);
                    }
                    self.msg_sender.close();
                    self.error_sender.close();
                    trace!("Socket reading task stopped");
                    return;
                }
            };
            trace!("Message received on the socket: {:?}", msg);

            let msg = Arc::new(msg);
            if let Err(e) = self.msg_sender.broadcast(msg.clone()).await {
                // An error would be due to the channel being closed, which only happens when the
                // connection is dropped, so just stop the task.
                debug!("Error broadcasting message to streams: {:?}", e);
                return;
            }
            trace!("Message broadcasted to all streams: {:?}", msg);
        }
    }
}

/// A D-Bus connection.
///
/// A connection to a D-Bus bus, or a direct peer.
///
/// Once created, the connection is authenticated and negotiated and messages can be sent or
/// received, such as [method calls] or [signals].
///
/// For higher-level message handling (typed functions, introspection, documentation reasons etc),
/// it is recommended to wrap the low-level D-Bus messages into Rust functions with the
/// [`dbus_proxy`] and [`dbus_interface`] macros instead of doing it directly on a `Connection`.
///
/// Typically, a connection is made to the session bus with [`Connection::session`], or to the
/// system bus with [`Connection::system`]. Then the connection is used with [`crate::Proxy`]
/// instances or the on-demand [`ObjectServer`] instance that can be accessed through
/// [`Connection::object_server`].
///
/// `Connection` implements [`Clone`] and cloning it is a very cheap operation, as the underlying
/// data is not cloned. This makes it very convenient to share the connection between different
/// parts of your code. `Connection` also implements [`std::marker::Sync`] and[`std::marker::Send`]
/// so you can send and share a connection instance across threads as well.
///
/// `Connection` keeps an internal queue of incoming message. The maximum capacity of this queue
/// is configurable through the [`set_max_queued`] method. The default size is 64. When the queue is
/// full, no more messages can be received until room is created for more. This is why it's
/// important to ensure that all [`crate::MessageStream`] and [`crate::blocking::MessageIterator`]
/// instances are continuously polled and iterated on, respectively.
///
/// For sending messages you can either use [`Connection::send_message`] method or make use of the
/// [`Sink`] implementation. For latter, you might find [`SinkExt`] API very useful. Keep in mind
/// that [`Connection`] will not manage the serial numbers (cookies) on the messages for you when
/// they are sent through the [`Sink`] implementation. You can manually assign unique serial numbers
/// to them using the [`Connection::assign_serial_num`] method before sending them off, if needed.
/// Having said that, the [`Sink`] is mainly useful for sending out signals, as they do not expect
/// a reply, and serial numbers are not very useful for signals either for the same reason.
///
/// Since you do not need exclusive access to a `zbus::Connection` to send messages on the bus,
/// [`Sink`] is also implemented on `&Connection`.
///
/// # Caveats
///
/// At the moment, a simultaneous [flush request] from multiple tasks/threads could
/// potentially create a busy loop, thus wasting CPU time. This limitation may be removed in the
/// future.
///
/// [flush request]: https://docs.rs/futures/0.3.15/futures/sink/trait.SinkExt.html#method.flush
///
/// [method calls]: struct.Connection.html#method.call_method
/// [signals]: struct.Connection.html#method.emit_signal
/// [`dbus_proxy`]: attr.dbus_proxy.html
/// [`dbus_interface`]: attr.dbus_interface.html
/// [`Clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html
/// [`set_max_queued`]: struct.Connection.html#method.set_max_queued
///
/// ### Examples
///
/// #### Get the session bus ID
///
/// ```
///# use zvariant::Type;
///#
///# async_io::block_on(async {
/// use zbus::Connection;
///
/// let mut connection = Connection::session().await?;
///
/// let reply = connection
///     .call_method(
///         Some("org.freedesktop.DBus"),
///         "/org/freedesktop/DBus",
///         Some("org.freedesktop.DBus"),
///         "GetId",
///         &(),
///     )
///     .await?;
///
/// let id: &str = reply.body()?;
/// println!("Unique ID of the bus: {}", id);
///# Ok::<(), zbus::Error>(())
///# });
/// ```
///
/// #### Monitoring all messages
///
/// Let's eavesdrop on the session bus 😈 using the [Monitor] interface:
///
/// ```rust,no_run
///# async_io::block_on(async {
/// use futures_util::stream::TryStreamExt;
/// use zbus::{Connection, MessageStream};
///
/// let connection = Connection::session().await?;
///
/// connection
///     .call_method(
///         Some("org.freedesktop.DBus"),
///         "/org/freedesktop/DBus",
///         Some("org.freedesktop.DBus.Monitoring"),
///         "BecomeMonitor",
///         &(&[] as &[&str], 0u32),
///     )
///     .await?;
///
/// let mut stream = MessageStream::from(connection);
/// while let Some(msg) = stream.try_next().await? {
///     println!("Got message: {}", msg);
/// }
///
///# Ok::<(), zbus::Error>(())
///# });
/// ```
///
/// This should print something like:
///
/// ```console
/// Got message: Signal NameAcquired from org.freedesktop.DBus
/// Got message: Signal NameLost from org.freedesktop.DBus
/// Got message: Method call GetConnectionUnixProcessID from :1.1324
/// Got message: Error org.freedesktop.DBus.Error.NameHasNoOwner:
///              Could not get PID of name ':1.1332': no such name from org.freedesktop.DBus
/// Got message: Method call AddMatch from :1.918
/// Got message: Method return from org.freedesktop.DBus
/// ```
///
/// [Monitor]: https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-become-monitor
#[derive(Clone, Debug)]
pub struct Connection {
    pub(crate) inner: Arc<ConnectionInner>,

    pub(crate) msg_receiver: InactiveReceiver<Arc<Message>>,

    // Receiver side of the error channel
    pub(crate) error_receiver: Receiver<Error>,
}

assert_impl_all!(Connection: Send, Sync, Unpin);

/// A method call whose completion can be awaited or joined with other streams.
///
/// This is useful for cache population method calls, where joining the [`JoinableStream`] with
/// an update signal stream can be used to ensure that cache updates are not overwritten by a cache
/// population whose task is scheduled later.
#[derive(Debug)]
pub(crate) struct PendingMethodCall {
    stream: Option<MessageStream>,
    serial: u32,
}

impl Future for PendingMethodCall {
    type Output = Result<Arc<Message>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_before(cx, None).map(|ret| {
            ret.map(|(_, r)| r).unwrap_or_else(|| {
                Err(crate::Error::Io(io::Error::new(
                    ErrorKind::BrokenPipe,
                    "socket closed",
                )))
            })
        })
    }
}

impl OrderedFuture for PendingMethodCall {
    type Output = Result<Arc<Message>>;
    type Ordering = zbus::MessageSequence;

    fn poll_before(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        before: Option<&Self::Ordering>,
    ) -> Poll<Option<(Self::Ordering, Self::Output)>> {
        let this = self.get_mut();
        if let Some(stream) = &mut this.stream {
            loop {
                match Pin::new(&mut *stream).poll_next_before(cx, before) {
                    Poll::Ready(PollResult::Item {
                        data: Ok(msg),
                        ordering,
                    }) => {
                        if msg.reply_serial() != Some(this.serial) {
                            continue;
                        }
                        let res = match msg.message_type() {
                            MessageType::Error => Err(msg.into()),
                            MessageType::MethodReturn => Ok(msg),
                            _ => continue,
                        };
                        this.stream = None;
                        return Poll::Ready(Some((ordering, res)));
                    }
                    Poll::Ready(PollResult::Item {
                        data: Err(e),
                        ordering,
                    }) => {
                        return Poll::Ready(Some((ordering, Err(e))));
                    }

                    Poll::Ready(PollResult::NoneBefore) => {
                        return Poll::Ready(None);
                    }
                    Poll::Ready(PollResult::Terminated) => {
                        return Poll::Ready(None);
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
        Poll::Ready(None)
    }
}

impl Connection {
    /// Send `msg` to the peer.
    ///
    /// Unlike our [`Sink`] implementation, this method sets a unique (to this connection) serial
    /// number on the message before sending it off, for you.
    ///
    /// On successfully sending off `msg`, the assigned serial number is returned.
    pub async fn send_message(&self, mut msg: Message) -> Result<u32> {
        let serial = self.assign_serial_num(&mut msg)?;

        trace!("Sending message: {:?}", msg);
        (&mut &*self).send(msg).await?;
        trace!("Sent message with serial: {}", serial);

        Ok(serial)
    }

    /// Send a method call.
    ///
    /// Create a method-call message, send it over the connection, then wait for the reply.
    ///
    /// On successful reply, an `Ok(Message)` is returned. On error, an `Err` is returned. D-Bus
    /// error replies are returned as [`Error::MethodError`].
    pub async fn call_method<'d, 'p, 'i, 'm, D, P, I, M, B>(
        &self,
        destination: Option<D>,
        path: P,
        interface: Option<I>,
        method_name: M,
        body: &B,
    ) -> Result<Arc<Message>>
    where
        D: TryInto<BusName<'d>>,
        P: TryInto<ObjectPath<'p>>,
        I: TryInto<InterfaceName<'i>>,
        M: TryInto<MemberName<'m>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        let m = Message::method(
            self.unique_name(),
            destination,
            path,
            interface,
            method_name,
            body,
        )?;
        self.call_method_raw(m).await?.await
    }

    /// Send a method call.
    ///
    /// Send the given message, which must be a method call, over the connection and return an
    /// object that allows the reply to be retrieved.  Typically you'd want to use
    /// [`Connection::call_method`] instead.
    pub(crate) async fn call_method_raw(&self, msg: Message) -> Result<PendingMethodCall> {
        debug_assert_eq!(msg.message_type(), MessageType::MethodCall);

        let stream = Some(MessageStream::from(self.clone()));
        let serial = self.send_message(msg).await?;

        Ok(PendingMethodCall { stream, serial })
    }

    /// Emit a signal.
    ///
    /// Create a signal message, and send it over the connection.
    pub async fn emit_signal<'d, 'p, 'i, 'm, D, P, I, M, B>(
        &self,
        destination: Option<D>,
        path: P,
        interface: I,
        signal_name: M,
        body: &B,
    ) -> Result<()>
    where
        D: TryInto<BusName<'d>>,
        P: TryInto<ObjectPath<'p>>,
        I: TryInto<InterfaceName<'i>>,
        M: TryInto<MemberName<'m>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        let m = Message::signal(
            self.unique_name(),
            destination,
            path,
            interface,
            signal_name,
            body,
        )?;

        self.send_message(m).await.map(|_| ())
    }

    /// Reply to a message.
    ///
    /// Given an existing message (likely a method call), send a reply back to the caller with the
    /// given `body`.
    ///
    /// Returns the message serial number.
    pub async fn reply<B>(&self, call: &Message, body: &B) -> Result<u32>
    where
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        let m = Message::method_reply(self.unique_name(), call, body)?;
        self.send_message(m).await
    }

    /// Reply an error to a message.
    ///
    /// Given an existing message (likely a method call), send an error reply back to the caller
    /// with the given `error_name` and `body`.
    ///
    /// Returns the message serial number.
    pub async fn reply_error<'e, E, B>(
        &self,
        call: &Message,
        error_name: E,
        body: &B,
    ) -> Result<u32>
    where
        B: serde::ser::Serialize + zvariant::DynamicType,
        E: TryInto<ErrorName<'e>>,
        E::Error: Into<Error>,
    {
        let m = Message::method_error(self.unique_name(), call, error_name, body)?;
        self.send_message(m).await
    }

    /// Reply an error to a message.
    ///
    /// Given an existing message (likely a method call), send an error reply back to the caller
    /// using one of the standard interface reply types.
    ///
    /// Returns the message serial number.
    pub async fn reply_dbus_error(
        &self,
        call: &zbus::MessageHeader<'_>,
        err: impl DBusError,
    ) -> Result<u32> {
        let m = err.create_reply(call);
        self.send_message(m?).await
    }

    /// Register a well-known name for this service on the bus.
    ///
    /// You can request multiple names for the same `ObjectServer`. Use [`Connection::release_name`]
    /// for deregistering names registered through this method.
    ///
    /// Note that exclusive ownership without queueing is requested (using
    /// [`fdo::RequestNameFlags::ReplaceExisting`] and [`fdo::RequestNameFlags::DoNotQueue`] flags)
    /// since that is the most typical case. If that is not what you want, you should use
    /// [`fdo::DBusProxy::request_name`] instead (but make sure then that name is requested
    /// **after** you've setup your service implementation with the `ObjectServer`).
    ///
    /// # Errors
    ///
    /// Fails with `zbus::Error::NameTaken` if the name is already owned by another peer.
    pub async fn request_name<'w, W>(&self, well_known_name: W) -> Result<()>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        let well_known_name = well_known_name.try_into().map_err(Into::into)?;
        let mut names = self.inner.registered_names.lock().await;

        if names.contains(&well_known_name) {
            return Ok(());
        }

        let reply = fdo::DBusProxy::builder(self)
            .cache_properties(CacheProperties::No)
            .build()
            .await?
            .request_name(
                well_known_name.clone(),
                fdo::RequestNameFlags::ReplaceExisting | fdo::RequestNameFlags::DoNotQueue,
            )
            .await?;
        if let fdo::RequestNameReply::Exists = reply {
            Err(Error::NameTaken)
        } else {
            names.insert(well_known_name.to_owned());
            Ok(())
        }
    }

    /// Deregister a previously registered well-known name for this service on the bus.
    ///
    /// Use this method to deregister a well-known name, registered through
    /// [`Connection::request_name`].
    ///
    /// Unless an error is encountered, returns `Ok(true)` if name was previously registered with
    /// the bus through `self` and it has now been successfully deregistered, `Ok(false)` if name
    /// was not previously registered or already deregistered.
    pub async fn release_name<'w, W>(&self, well_known_name: W) -> Result<bool>
    where
        W: TryInto<WellKnownName<'w>>,
        W::Error: Into<Error>,
    {
        let well_known_name: WellKnownName<'w> = well_known_name.try_into().map_err(Into::into)?;
        let mut names = self.inner.registered_names.lock().await;
        // FIXME: Should be possible to avoid cloning/allocation here
        if !names.remove(&well_known_name.to_owned()) {
            return Ok(false);
        }

        fdo::DBusProxy::builder(self)
            .cache_properties(CacheProperties::No)
            .build()
            .await?
            .release_name(well_known_name)
            .await
            .map(|_| true)
            .map_err(Into::into)
    }

    /// Checks if `self` is a connection to a message bus.
    ///
    /// This will return `false` for p2p connections.
    pub fn is_bus(&self) -> bool {
        self.inner.bus_conn
    }

    /// Assigns a serial number to `msg` that is unique to this connection.
    ///
    /// This method can fail if `msg` is corrupted.
    pub fn assign_serial_num(&self, msg: &mut Message) -> Result<u32> {
        let mut serial = 0;
        msg.modify_primary_header(|primary| {
            serial = *primary.serial_num_or_init(|| self.next_serial());
            Ok(())
        })?;

        Ok(serial)
    }

    /// The unique name as assigned by the message bus or `None` if not a message bus connection.
    pub fn unique_name(&self) -> Option<&OwnedUniqueName> {
        self.inner.unique_name.get()
    }

    /// Max number of messages to queue.
    pub fn max_queued(&self) -> usize {
        self.msg_receiver.capacity()
    }

    /// Set the max number of messages to queue.
    pub fn set_max_queued(&mut self, max: usize) {
        self.msg_receiver.set_capacity(max);
    }

    /// The server's GUID.
    pub fn server_guid(&self) -> &str {
        self.inner.server_guid.as_str()
    }

    /// The underlying executor.
    ///
    /// When a connection is built with internal_executor set to false, zbus will not spawn a
    /// thread to run the executor. You're responsible to continuously [tick the executor][tte].
    /// Failure to do so will result in hangs.
    ///
    /// # Examples
    ///
    /// Here is how one would typically run the zbus executor through tokio's single-threaded
    /// scheduler:
    ///
    /// ```
    /// use zbus::ConnectionBuilder;
    /// use tokio::runtime;
    ///
    /// runtime::Builder::new_current_thread()
    ///        .build()
    ///        .unwrap()
    ///        .block_on(async {
    ///     let conn = ConnectionBuilder::session()
    ///         .unwrap()
    ///         .internal_executor(false)
    ///         .build()
    ///         .await
    ///         .unwrap();
    ///     {
    ///        let conn = conn.clone();
    ///        tokio::task::spawn(async move {
    ///            loop {
    ///                conn.executor().tick().await;
    ///            }
    ///        });
    ///     }
    ///
    ///     // All your other async code goes here.
    /// });
    /// ```
    ///
    /// **Note**: zbus 2.1 added support for tight integration with tokio. This means, if you use
    /// zbus with tokio, you do not need to worry about this at all. All you need to do is enable
    /// `tokio` feature and disable the (default) `async-io` feature in your `Cargo.toml`.
    ///
    /// [tte]: https://docs.rs/async-executor/1.4.1/async_executor/struct.Executor.html#method.tick
    pub fn executor(&self) -> &Executor<'static> {
        &self.inner.executor
    }

    /// Get a reference to the associated [`ObjectServer`].
    ///
    /// The `ObjectServer` is created on-demand.
    ///
    /// **Note**: Once the `ObjectServer` is created, it will be replying to all method calls
    /// received on `self`. If you want to manually reply to method calls, do not use this
    /// method (or any of the `ObjectServer` related API).
    pub fn object_server(&self) -> impl Deref<Target = ObjectServer> + '_ {
        // FIXME: Maybe it makes sense after all to implement Deref<Target= ObjectServer> for
        // crate::ObjectServer instead of this wrapper?
        struct Wrapper<'a>(&'a blocking::ObjectServer);
        impl<'a> Deref for Wrapper<'a> {
            type Target = ObjectServer;

            fn deref(&self) -> &Self::Target {
                self.0.inner()
            }
        }

        Wrapper(self.sync_object_server(true))
    }

    pub(crate) fn sync_object_server(&self, start: bool) -> &blocking::ObjectServer {
        self.inner
            .object_server
            .get_or_init(|| self.setup_object_server(start))
    }

    fn setup_object_server(&self, start: bool) -> blocking::ObjectServer {
        if start {
            self.start_object_server();
        }

        blocking::ObjectServer::new(self)
    }

    #[instrument(skip(self))]
    pub(crate) fn start_object_server(&self) {
        self.inner.object_server_dispatch_task.get_or_init(|| {
            let weak_conn = WeakConnection::from(self);
            let mut stream = MessageStream::from(self.clone());

            self.inner.executor.spawn(
                async move {
                    while let Some(msg) = stream.next().await.and_then(|m| {
                        if let Err(e) = &m {
                            debug!("Error while reading from object server stream: {:?}", e);
                        }
                        m.ok()
                    }) {
                        if let Some(conn) = weak_conn.upgrade() {
                            let executor = conn.inner.executor.clone();
                            executor
                                .spawn(
                                    async move {
                                        let server = conn.object_server();
                                        if let Err(e) = server.dispatch_message(&msg).await {
                                            debug!(
                                                "Error dispatching message. Message: {:?}, error: {:?}",
                                                msg, e
                                            );
                                        }
                                    }
                                    .instrument(trace_span!("ObjectServer method task"))
                                )
                                .detach();
                        } else {
                            // If connection is completely gone, no reason to keep running the task anymore.
                            trace!("Connection is gone, stopping associated object server task");
                            break;
                        }
                    }
                }
                .instrument(trace_span!("ObjectServer task")),
            )
        });
    }

    pub(crate) async fn add_match(&self, expr: String) -> Result<()> {
        use std::collections::hash_map::Entry;
        if !self.is_bus() {
            return Ok(());
        }
        let mut subscriptions = self.inner.signal_matches.lock().await;
        match subscriptions.entry(expr) {
            Entry::Vacant(e) => {
                fdo::DBusProxy::builder(self)
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?
                    .add_match(e.key())
                    .await?;
                e.insert(1);
            }
            Entry::Occupied(mut e) => {
                *e.get_mut() += 1;
            }
        }
        Ok(())
    }

    pub(crate) async fn remove_match(&self, expr: String) -> Result<bool> {
        use std::collections::hash_map::Entry;
        if !self.is_bus() {
            return Ok(true);
        }
        let mut subscriptions = self.inner.signal_matches.lock().await;
        // TODO when it becomes stable, use HashMap::raw_entry and only require expr: &str
        // (both here and in add_match)
        match subscriptions.entry(expr) {
            Entry::Vacant(_) => Ok(false),
            Entry::Occupied(mut e) => {
                *e.get_mut() -= 1;
                if *e.get() == 0 {
                    fdo::DBusProxy::builder(self)
                        .cache_properties(CacheProperties::No)
                        .build()
                        .await?
                        .remove_match(e.key())
                        .await?;
                    e.remove();
                }
                Ok(true)
            }
        }
    }

    pub(crate) fn queue_remove_match(&self, expr: String) {
        let conn = self.clone();
        self.inner
            .executor
            .spawn(async move { conn.remove_match(expr).await })
            .detach()
    }

    async fn hello_bus(&self) -> Result<()> {
        let dbus_proxy = fdo::DBusProxy::builder(self)
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        let future = dbus_proxy.hello().map_err(Into::into);
        let name = self.run_future_at_init(future).await?;

        self.inner
            .unique_name
            .set(name)
            // programmer (probably our) error if this fails.
            .expect("Attempted to set unique_name twice");

        Ok(())
    }

    // With external executor, our executor is only run after the connection construction is
    // completed and some futures need to run to completion before that is done so we need to tick
    // the executor ourselves in parallel to making the method call. With the internal executor,
    /// this is not needed but harmless.
    pub(crate) async fn run_future_at_init<F, O>(&self, future: F) -> Result<O>
    where
        F: Future<Output = Result<O>>,
    {
        let executor = self.inner.executor.clone();
        let ticking_future = async move {
            // Keep running as long as this task/future is not cancelled.
            loop {
                executor.tick().await;
            }
        };

        futures_util::pin_mut!(future);
        futures_util::pin_mut!(ticking_future);

        match select(future, ticking_future).await {
            Either::Left((res, _)) => res,
            Either::Right((_, _)) => unreachable!("ticking task future shouldn't finish"),
        }
    }

    pub(crate) async fn new(
        auth: Authenticated<Box<dyn Socket>>,
        bus_connection: bool,
        internal_executor: bool,
    ) -> Result<Self> {
        let auth = auth.into_inner();
        #[cfg(unix)]
        let cap_unix_fd = auth.cap_unix_fd;

        let (msg_sender, msg_receiver) = broadcast(DEFAULT_MAX_QUEUED);
        let msg_receiver = msg_receiver.deactivate();
        let (error_sender, error_receiver) = bounded(1);
        let executor = Arc::new(Executor::new());
        let raw_conn = Arc::new(sync::Mutex::new(auth.conn));

        // Start the message receiver task.
        let msg_receiver_task =
            MessageReceiverTask::new(raw_conn.clone(), msg_sender, error_sender).spawn(&executor);

        let connection = Self {
            error_receiver,
            msg_receiver,
            inner: Arc::new(ConnectionInner {
                raw_conn,
                server_guid: auth.server_guid,
                #[cfg(unix)]
                cap_unix_fd,
                bus_conn: bus_connection,
                serial: AtomicU32::new(1),
                unique_name: OnceCell::new(),
                signal_matches: Mutex::new(HashMap::new()),
                object_server: OnceCell::new(),
                object_server_dispatch_task: OnceCell::new(),
                executor: executor.clone(),
                msg_receiver_task,
                registered_names: Mutex::new(HashSet::new()),
            }),
        };

        if internal_executor {
            let ticker_future = async move {
                // Run as long as there is a task to run.
                while !executor.is_empty() {
                    executor.tick().await;
                }
            };
            #[cfg(feature = "async-io")]
            std::thread::Builder::new()
                .name("zbus::Connection executor".into())
                .spawn(move || crate::utils::block_on(ticker_future))?;

            #[cfg(not(feature = "async-io"))]
            tokio::task::spawn(ticker_future);
        }

        if !bus_connection {
            return Ok(connection);
        }

        // Now that the server has approved us, we must send the bus Hello, as per specs
        connection.hello_bus().await?;

        Ok(connection)
    }

    fn next_serial(&self) -> u32 {
        self.inner.serial.fetch_add(1, SeqCst)
    }

    /// Create a `Connection` to the session/user message bus.
    pub async fn session() -> Result<Self> {
        ConnectionBuilder::session()?.build().await
    }

    /// Create a `Connection` to the system-wide message bus.
    pub async fn system() -> Result<Self> {
        ConnectionBuilder::system()?.build().await
    }

    /// Returns a listener, notified on various connection activity.
    ///
    /// This function is meant for the caller to implement idle or timeout on inactivity.
    pub fn monitor_activity(&self) -> EventListener {
        self.inner
            .raw_conn
            .lock()
            .expect("poisoned lock")
            .monitor_activity()
    }
}

impl Sink<Message> for Connection {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut &*self).poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, msg: Message) -> Result<()> {
        Pin::new(&mut &*self).start_send(msg)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut &*self).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut &*self).poll_close(cx)
    }
}

impl<'a> Sink<Message> for &'a Connection {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        // TODO: We should have a max queue length in raw::Socket for outgoing messages.
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, msg: Message) -> Result<()> {
        #[cfg(unix)]
        if !msg.fds().is_empty() && !self.inner.cap_unix_fd {
            return Err(Error::Unsupported);
        }

        self.inner
            .raw_conn
            .lock()
            .expect("poisoned lock")
            .enqueue_message(msg);

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.inner.raw_conn.lock().expect("poisoned lock").flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut raw_conn = self.inner.raw_conn.lock().expect("poisoned lock");
        let res = raw_conn.flush(cx);
        match ready!(res) {
            Ok(_) => (),
            Err(e) => return Poll::Ready(Err(e)),
        }

        Poll::Ready(raw_conn.close())
    }
}

struct ReceiveMessage<'r> {
    raw_conn: &'r sync::Mutex<RawConnection<Box<dyn Socket>>>,
}

impl<'r> Future for ReceiveMessage<'r> {
    type Output = Result<Message>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut raw_conn = self.raw_conn.lock().expect("poisoned lock");
        raw_conn.try_receive_message(cx)
    }
}

impl From<crate::blocking::Connection> for Connection {
    fn from(conn: crate::blocking::Connection) -> Self {
        conn.into_inner()
    }
}

// Internal API that allows keeping a weak connection ref around.
#[derive(Debug)]
pub(crate) struct WeakConnection {
    inner: Weak<ConnectionInner>,
    msg_receiver: InactiveReceiver<Arc<Message>>,
    error_receiver: Receiver<Error>,
}

impl WeakConnection {
    /// Upgrade to a Connection.
    pub fn upgrade(&self) -> Option<Connection> {
        self.inner.upgrade().map(|inner| Connection {
            inner,
            msg_receiver: self.msg_receiver.clone(),
            error_receiver: self.error_receiver.clone(),
        })
    }
}

impl From<&Connection> for WeakConnection {
    fn from(conn: &Connection) -> Self {
        Self {
            inner: Arc::downgrade(&conn.inner),
            msg_receiver: conn.msg_receiver.clone(),
            error_receiver: conn.error_receiver.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use futures_util::stream::TryStreamExt;
    use ntest::timeout;
    use test_log::test;

    #[cfg(all(unix, feature = "async-io"))]
    use crate::AuthMechanism;

    use super::*;

    async fn test_p2p(server: Connection, client: Connection) -> Result<()> {
        let server_future = async {
            let mut stream = MessageStream::from(&server);
            let method = loop {
                let m = stream.try_next().await?.unwrap();
                if m.to_string() == "Method call Test" {
                    break m;
                }
            };

            // Send another message first to check the queueing function on client side.
            server
                .emit_signal(None::<()>, "/", "org.zbus.p2p", "ASignalForYou", &())
                .await?;
            server.reply(&method, &("yay")).await
        };

        let client_future = async {
            let mut stream = MessageStream::from(&client);
            let reply = client
                .call_method(None::<()>, "/", Some("org.zbus.p2p"), "Test", &())
                .await?;
            assert_eq!(reply.to_string(), "Method return");
            // Check we didn't miss the signal that was sent during the call.
            let m = stream.try_next().await?.unwrap();
            assert_eq!(m.to_string(), "Signal ASignalForYou");
            reply.body::<String>()
        };

        let (val, _) = futures_util::try_join!(client_future, server_future)?;
        assert_eq!(val, "yay");

        Ok(())
    }

    // FIXME: Make it work with tokio as well.
    #[cfg(feature = "async-io")]
    #[test]
    #[timeout(15000)]
    fn tcp_p2p() {
        crate::utils::block_on(test_tcp_p2p()).unwrap();
    }

    #[cfg(feature = "async-io")]
    async fn test_tcp_p2p() -> Result<()> {
        let guid = Guid::generate();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::net::TcpStream::connect(addr).unwrap();
        let server = listener.incoming().next().unwrap().unwrap();

        let server = {
            let c = ConnectionBuilder::tcp_stream(server).server(&guid).p2p();

            // EXTERNAL is only implemented on win32 with TCP sockets
            #[cfg(unix)]
            let c = c.auth_mechanisms(&[AuthMechanism::Anonymous]);

            c.build()
        };
        let client = ConnectionBuilder::tcp_stream(client).p2p().build();
        let (client, server) = futures_util::try_join!(client, server)?;

        test_p2p(server, client).await
    }

    #[cfg(any(unix, feature = "async-io"))]
    #[test]
    #[timeout(15000)]
    fn unix_p2p() {
        crate::utils::block_on(test_unix_p2p()).unwrap();
    }

    #[cfg(any(unix, feature = "async-io"))]
    async fn test_unix_p2p() -> Result<()> {
        #[cfg(all(unix, feature = "async-io"))]
        use std::os::unix::net::UnixStream;
        #[cfg(not(feature = "async-io"))]
        use tokio::net::UnixStream;
        #[cfg(all(windows, feature = "async-io"))]
        use uds_windows::UnixStream;

        let guid = Guid::generate();

        let (p0, p1) = UnixStream::pair().unwrap();

        let server = ConnectionBuilder::unix_stream(p0)
            .server(&guid)
            .p2p()
            .build();
        let client = ConnectionBuilder::unix_stream(p1).p2p().build();
        let (client, server) = futures_util::try_join!(client, server)?;

        test_p2p(server, client).await
    }

    #[test]
    #[timeout(15000)]
    fn serial_monotonically_increases() {
        crate::utils::block_on(test_serial_monotonically_increases());
    }

    async fn test_serial_monotonically_increases() {
        let c = Connection::session().await.unwrap();
        let serial = c.next_serial() + 1;

        for next in serial..serial + 10 {
            assert_eq!(next, c.next_serial());
        }
    }
}
