use super::UnixStream;
use crate::driver::Socket;
use std::{io, path::Path};

/// A Unix socket server, listening for connections.
///
/// You can accept a new connection by using the [`accept`](`UnixListener::accept`)
/// method.
///
/// # Examples
///
/// ```
/// use tokio_uring::net::UnixListener;
/// use tokio_uring::net::UnixStream;
///
/// fn main() {
///     let sock_file = "/tmp/tokio-uring-unix-test.sock";
///     let listener = UnixListener::bind(&sock_file).unwrap();
///
///     tokio_uring::start(async move {
///         let tx_fut = UnixStream::connect(&sock_file);
///
///         let rx_fut = listener.accept();
///
///         let (tx, rx) = tokio::try_join!(tx_fut, rx_fut).unwrap();
///
///         tx.write(b"test" as &'static [u8]).await.0.unwrap();
///
///         let (_, buf) = rx.read(vec![0; 4]).await;
///
///         assert_eq!(buf, b"test");
///     });
///     std::fs::remove_file(&sock_file);
/// }
/// ```
pub struct UnixListener {
    inner: Socket,
}

impl UnixListener {
    /// Creates a new UnixListener, which will be bound to the specified file path.
    /// The file path cannnot yet exist, and will be cleaned up upon dropping `UnixListener`
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        let socket = Socket::bind_unix(path, libc::SOCK_STREAM)?;
        socket.listen(1024)?;
        Ok(UnixListener { inner: socket })
    }

    /// Accepts a new incoming connection from this listener.
    ///
    /// This function will yield once a new Unix domain socket connection
    /// is established. When established, the corresponding [`UnixStream`] and
    /// will be returned.
    ///
    /// [`UnixStream`]: struct@crate::net::UnixStream
    pub async fn accept(&self) -> io::Result<UnixStream> {
        let (socket, _) = self.inner.accept().await?;
        let stream = UnixStream { inner: socket };
        Ok(stream)
    }
}
