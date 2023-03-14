use std::{
    error::Error as StdError,
    fmt::{self, Debug},
    io,
};

use netlink_packet_core::NetlinkMessage;

#[derive(Debug)]
pub struct Error<T>
where
    T: Debug + Eq + PartialEq + Clone,
{
    kind: ErrorKind<T>,
}

impl<T> Error<T>
where
    T: Debug + Eq + PartialEq + Clone,
{
    pub fn kind(&self) -> &ErrorKind<T> {
        &self.kind
    }

    pub fn into_inner(self) -> ErrorKind<T> {
        self.kind
    }
}

#[derive(Debug)]
pub enum ErrorKind<T>
where
    T: Debug + Eq + PartialEq + Clone,
{
    /// The netlink connection is closed
    ConnectionClosed,

    /// Received an error message as a response
    NetlinkError(NetlinkMessage<T>),

    /// Error while reading from or writing to the netlink socket
    SocketIo(io::Error),
}

impl<T> From<ErrorKind<T>> for Error<T>
where
    T: Debug + Eq + PartialEq + Clone,
{
    fn from(kind: ErrorKind<T>) -> Error<T> {
        Error { kind }
    }
}

impl<T> fmt::Display for Error<T>
where
    T: Debug + Eq + PartialEq + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::ErrorKind::*;
        match self.kind() {
            SocketIo(ref e) => write!(f, "{}: {}", self, e),
            ConnectionClosed => write!(f, "{}", self),
            NetlinkError(ref message) => write!(f, "{}: {:?}", self, message),
        }
    }
}

impl<T> StdError for Error<T>
where
    T: Debug + Eq + PartialEq + Clone,
{
    fn description(&self) -> &str {
        use crate::ErrorKind::*;
        match self.kind() {
            SocketIo(_) => "Error while reading from or writing to the netlink socket",
            ConnectionClosed => "The netlink connection is closed",
            NetlinkError(_) => "Received an error message as a response",
        }
    }

    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        if let ErrorKind::SocketIo(ref e) = self.kind() {
            Some(e)
        } else {
            None
        }
    }
}
