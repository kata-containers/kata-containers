use std::{io, path::Path};

use hyper::server::{Builder, Server};

use conn::SocketIncoming;

pub(crate) mod conn {
    use futures_util::ready;
    use hyper::server::accept::Accept;
    use pin_project::pin_project;
    use std::{
        io,
        path::Path,
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::net::{UnixListener, UnixStream};

    /// A stream of connections from binding to a socket.
    #[pin_project]
    #[derive(Debug)]
    pub struct SocketIncoming {
        listener: UnixListener,
    }

    impl SocketIncoming {
        /// Creates a new `SocketIncoming` binding to provided socket path.
        pub fn bind(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
            let listener = UnixListener::bind(path)?;

            Ok(Self { listener })
        }

        /// Creates a new `SocketIncoming` from Tokio's `UnixListener`
        ///
        /// ```rust,ignore
        /// let socket = SocketIncoming::from_listener(unix_listener);
        /// let server = Server::builder(socket).serve(service);
        /// ```
        pub fn from_listener(listener: UnixListener) -> Self {
            Self { listener }
        }
    }

    impl Accept for SocketIncoming {
        type Conn = UnixStream;
        type Error = io::Error;

        fn poll_accept(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
            let conn = ready!(self.listener.poll_accept(cx))?.0;
            Poll::Ready(Some(Ok(conn)))
        }
    }

    impl From<UnixListener> for SocketIncoming {
        fn from(listener: UnixListener) -> Self {
            Self::from_listener(listener)
        }
    }
}

/// Extension trait for provisioning a hyper HTTP server over a Unix domain
/// socket.
///
/// # Example
///
/// ```rust
/// use hyper::{Server, Body, Response, service::{make_service_fn, service_fn}};
/// use hyperlocal::UnixServerExt;
///
/// # async {
/// let make_service = make_service_fn(|_| async {
///     Ok::<_, hyper::Error>(service_fn(|_req| async {
///         Ok::<_, hyper::Error>(Response::new(Body::from("It works!")))
///     }))
/// });
///
/// Server::bind_unix("/tmp/hyperlocal.sock")?.serve(make_service).await?;
/// # Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
/// # };
/// ```
pub trait UnixServerExt {
    /// Convenience method for constructing a Server listening on a Unix socket.
    fn bind_unix(path: impl AsRef<Path>) -> Result<Builder<SocketIncoming>, io::Error>;
}

impl UnixServerExt for Server<SocketIncoming, ()> {
    fn bind_unix(path: impl AsRef<Path>) -> Result<Builder<SocketIncoming>, io::Error> {
        let incoming = SocketIncoming::bind(path)?;
        Ok(Server::builder(incoming))
    }
}
