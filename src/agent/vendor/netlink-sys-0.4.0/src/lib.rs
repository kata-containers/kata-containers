mod protocols;
pub use self::protocols::*;

mod sys;
pub use self::sys::SocketAddr;

#[cfg(feature = "mio_socket")]
mod mio;

#[cfg(feature = "tokio_socket")]
mod tokio;

#[cfg(not(feature = "tokio_socket"))]
pub use self::sys::Socket;
#[cfg(feature = "tokio_socket")]
pub use self::tokio::Socket;

pub mod constants;
