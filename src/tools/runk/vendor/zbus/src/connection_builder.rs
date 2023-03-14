#[cfg(feature = "async-io")]
use async_io::Async;
use async_lock::RwLock;
use static_assertions::assert_impl_all;
#[cfg(feature = "async-io")]
use std::net::TcpStream;
#[cfg(all(unix, feature = "async-io"))]
use std::os::unix::net::UnixStream;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::TryInto,
    sync::Arc,
};
#[cfg(not(feature = "async-io"))]
use tokio::net::TcpStream;
#[cfg(all(unix, not(feature = "async-io")))]
use tokio::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;

use zvariant::ObjectPath;

use crate::{
    address::{self, Address},
    names::{InterfaceName, WellKnownName},
    raw::Socket,
    AuthMechanism, Authenticated, Connection, Error, Guid, Interface, Result,
};

const DEFAULT_MAX_QUEUED: usize = 64;

#[derive(Debug)]
enum Target {
    UnixStream(UnixStream),
    TcpStream(TcpStream),
    Address(Address),
    Socket(Box<dyn Socket>),
}

type Interfaces<'a> =
    HashMap<ObjectPath<'a>, HashMap<InterfaceName<'static>, Arc<RwLock<dyn Interface>>>>;

/// A builder for [`zbus::Connection`].
#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct ConnectionBuilder<'a> {
    target: Target,
    max_queued: Option<usize>,
    guid: Option<&'a Guid>,
    p2p: bool,
    internal_executor: bool,
    #[derivative(Debug = "ignore")]
    interfaces: Interfaces<'a>,
    names: HashSet<WellKnownName<'a>>,
    auth_mechanisms: Option<VecDeque<AuthMechanism>>,
}

assert_impl_all!(ConnectionBuilder<'_>: Send, Sync, Unpin);

impl<'a> ConnectionBuilder<'a> {
    /// Create a builder for the session/user message bus connection.
    pub fn session() -> Result<Self> {
        Ok(Self::new(Target::Address(Address::session()?)))
    }

    /// Create a builder for the system-wide message bus connection.
    pub fn system() -> Result<Self> {
        Ok(Self::new(Target::Address(Address::system()?)))
    }

    /// Create a builder for connection that will use the given [D-Bus bus address].
    ///
    /// [D-Bus bus address]: https://dbus.freedesktop.org/doc/dbus-specification.html#addresses
    pub fn address<A>(address: A) -> Result<Self>
    where
        A: TryInto<Address>,
        A::Error: Into<Error>,
    {
        Ok(Self::new(Target::Address(
            address.try_into().map_err(Into::into)?,
        )))
    }

    /// Create a builder for connection that will use the given unix stream.
    ///
    /// If the default `async-io` feature is disabled, this method will expect
    /// [`tokio::net::UnixStream`](https://docs.rs/tokio/latest/tokio/net/struct.UnixStream.html)
    /// argument.
    #[must_use]
    pub fn unix_stream(stream: UnixStream) -> Self {
        Self::new(Target::UnixStream(stream))
    }

    /// Create a builder for connection that will use the given TCP stream.
    ///
    /// If the default `async-io` feature is disabled, this method will expect
    /// [`tokio::net::TcpStream`](https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html)
    /// argument.
    #[must_use]
    pub fn tcp_stream(stream: TcpStream) -> Self {
        Self::new(Target::TcpStream(stream))
    }

    /// Create a builder for connection that will use the given socket.
    #[must_use]
    pub fn socket<S: Socket + 'static>(socket: S) -> Self {
        Self::new(Target::Socket(Box::new(socket)))
    }

    /// Specify the mechanisms to use during authentication.
    #[must_use]
    pub fn auth_mechanisms(mut self, auth_mechanisms: &[AuthMechanism]) -> Self {
        self.auth_mechanisms = Some(VecDeque::from(auth_mechanisms.to_vec()));

        self
    }

    /// The to-be-created connection will be a peer-to-peer connection.
    #[must_use]
    pub fn p2p(mut self) -> Self {
        self.p2p = true;

        self
    }

    /// The to-be-created connection will be a server using the given GUID.
    ///
    /// The to-be-created connection will wait for incoming client authentication handshake and
    /// negotiation messages, for peer-to-peer communications after successful creation.
    #[must_use]
    pub fn server(mut self, guid: &'a Guid) -> Self {
        self.guid = Some(guid);

        self
    }

    /// Set the max number of messages to queue.
    ///
    /// Since typically you'd want to set this at instantiation time, you can set it through the builder.
    ///
    /// # Example
    ///
    /// ```
    ///# use std::error::Error;
    ///# use zbus::ConnectionBuilder;
    ///# use async_io::block_on;
    ///#
    ///# block_on(async {
    /// let conn = ConnectionBuilder::session()?
    ///     .max_queued(30)
    ///     .build()
    ///     .await?;
    /// assert_eq!(conn.max_queued(), 30);
    ///
    ///#     Ok::<(), zbus::Error>(())
    ///# });
    ///#
    /// // Do something useful with `conn`..
    ///# Ok::<_, Box<dyn Error + Send + Sync>>(())
    /// ```
    #[must_use]
    pub fn max_queued(mut self, max: usize) -> Self {
        self.max_queued = Some(max);

        self
    }

    /// Enable or disable the internal executor thread.
    ///
    /// The thread is enabled by default.
    ///
    /// See [Connection::executor] for more details.
    #[must_use]
    pub fn internal_executor(mut self, enabled: bool) -> Self {
        self.internal_executor = enabled;

        self
    }

    /// Register a D-Bus [`Interface`] to be served at a given path.
    ///
    /// This is similar to [`zbus::ObjectServer::at`], except that it allows you to have your
    /// interfaces available immediately after the connection is established. Typically, this is
    /// exactly what you'd want. Also in contrast to [`zbus::ObjectServer::at`], this method will
    /// replace any previously added interface with the same name at the same path.
    pub fn serve_at<P, I>(mut self, path: P, iface: I) -> Result<Self>
    where
        I: Interface,
        P: TryInto<ObjectPath<'a>>,
        P::Error: Into<Error>,
    {
        let path = path.try_into().map_err(Into::into)?;
        let entry = self.interfaces.entry(path).or_default();
        entry.insert(I::name(), Arc::new(RwLock::new(iface)));

        Ok(self)
    }

    /// Register a well-known name for this connection on the bus.
    ///
    /// This is similar to [`zbus::Connection::request_name`], except the name is requested as part
    /// of the connection setup ([`ConnectionBuilder::build`]), immediately after interfaces
    /// registered (through [`ConnectionBuilder::serve_at`]) are advertised. Typically this is
    /// exactly what you want.
    pub fn name<W>(mut self, well_known_name: W) -> Result<Self>
    where
        W: TryInto<WellKnownName<'a>>,
        W::Error: Into<Error>,
    {
        let well_known_name = well_known_name.try_into().map_err(Into::into)?;
        self.names.insert(well_known_name);

        Ok(self)
    }

    /// Build the connection, consuming the builder.
    ///
    /// # Errors
    ///
    /// Until server-side bus connection is supported, attempting to build such a connection will
    /// result in [`Error::Unsupported`] error.
    pub async fn build(self) -> Result<Connection> {
        let stream = match self.target {
            #[cfg(all(feature = "async-io"))]
            Target::UnixStream(stream) => Box::new(Async::new(stream)?) as Box<dyn Socket>,
            #[cfg(all(unix, not(feature = "async-io")))]
            Target::UnixStream(stream) => Box::new(stream) as Box<dyn Socket>,
            #[cfg(all(not(unix), not(feature = "async-io")))]
            Target::UnixStream(_) => return Err(Error::Unsupported),
            #[cfg(feature = "async-io")]
            Target::TcpStream(stream) => Box::new(Async::new(stream)?) as Box<dyn Socket>,
            #[cfg(not(feature = "async-io"))]
            Target::TcpStream(stream) => Box::new(stream) as Box<dyn Socket>,
            Target::Address(address) => match address.connect().await? {
                #[cfg(any(unix, feature = "async-io"))]
                address::Stream::Unix(stream) => Box::new(stream) as Box<dyn Socket>,
                address::Stream::Tcp(stream) => Box::new(stream) as Box<dyn Socket>,
            },
            Target::Socket(stream) => stream,
        };
        let auth = match self.guid {
            None => {
                // SASL Handshake
                Authenticated::client(stream, self.auth_mechanisms).await?
            }
            Some(guid) => {
                if !self.p2p {
                    return Err(Error::Unsupported);
                }

                #[cfg(unix)]
                let client_uid = {
                    #[cfg(any(target_os = "android", target_os = "linux"))]
                    {
                        use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};

                        let creds =
                            getsockopt(stream.as_raw_fd(), PeerCredentials).map_err(|e| {
                                Error::Handshake(format!("Failed to get peer credentials: {}", e))
                            })?;

                        creds.uid()
                    }

                    #[cfg(any(
                        target_os = "macos",
                        target_os = "ios",
                        target_os = "freebsd",
                        target_os = "dragonfly",
                        target_os = "openbsd",
                        target_os = "netbsd"
                    ))]
                    {
                        let uid = nix::unistd::getpeereid(stream.as_raw_fd())
                            .map_err(|e| {
                                Error::Handshake(format!("Failed to get peer credentials: {}", e))
                            })?
                            .0;

                        uid.into()
                    }
                };

                #[cfg(windows)]
                let client_sid = stream.peer_sid();

                Authenticated::server(
                    stream,
                    guid.clone(),
                    #[cfg(unix)]
                    client_uid,
                    #[cfg(windows)]
                    client_sid,
                    self.auth_mechanisms,
                )
                .await?
            }
        };

        let mut conn = Connection::new(auth, !self.p2p, self.internal_executor).await?;
        conn.set_max_queued(self.max_queued.unwrap_or(DEFAULT_MAX_QUEUED));

        if !self.interfaces.is_empty() {
            let object_server = conn.sync_object_server(false);
            for (path, interfaces) in self.interfaces {
                for (name, iface) in interfaces {
                    let future = object_server.at_ready(path.to_owned(), name, || iface);
                    let added = conn.run_future_at_init(future).await?;
                    // Duplicates shouldn't happen.
                    assert!(added);
                }
            }

            conn.start_object_server();
        }

        for name in self.names {
            let future = conn.request_name(name);
            conn.run_future_at_init(future).await?;
        }

        Ok(conn)
    }

    fn new(target: Target) -> Self {
        Self {
            target,
            p2p: false,
            max_queued: None,
            guid: None,
            internal_executor: true,
            interfaces: HashMap::new(),
            names: HashSet::new(),
            auth_mechanisms: None,
        }
    }
}
