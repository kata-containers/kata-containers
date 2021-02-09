use {channel};
use std::{fmt, io, error, any};

pub enum NotifyError<T> {
    Io(io::Error),
    Full(T),
    Closed(Option<T>),
}

impl<M> fmt::Debug for NotifyError<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NotifyError::Io(ref e) => {
                write!(fmt, "NotifyError::Io({:?})", e)
            }
            NotifyError::Full(..) => {
                write!(fmt, "NotifyError::Full(..)")
            }
            NotifyError::Closed(..) => {
                write!(fmt, "NotifyError::Closed(..)")
            }
        }
    }
}

impl<M> fmt::Display for NotifyError<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NotifyError::Io(ref e) => {
                write!(fmt, "IO error: {}", e)
            }
            NotifyError::Full(..) => write!(fmt, "Full"),
            NotifyError::Closed(..) => write!(fmt, "Closed")
        }
    }
}

impl<M: any::Any> error::Error for NotifyError<M> {
    fn description(&self) -> &str {
        match *self {
            NotifyError::Io(ref err) => err.description(),
            NotifyError::Closed(..) => "The receiving end has hung up",
            NotifyError::Full(..) => "Queue is full"
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            NotifyError::Io(ref err) => Some(err),
            _ => None
        }
    }
}

impl<M> From<channel::TrySendError<M>> for NotifyError<M> {
    fn from(src: channel::TrySendError<M>) -> NotifyError<M> {
        match src {
            channel::TrySendError::Io(e) => NotifyError::Io(e),
            channel::TrySendError::Full(v) => NotifyError::Full(v),
            channel::TrySendError::Disconnected(v) => NotifyError::Closed(Some(v)),
        }
    }
}
