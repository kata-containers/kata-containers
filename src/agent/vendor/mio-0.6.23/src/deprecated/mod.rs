#![allow(deprecated)]

mod event_loop;
mod io;
mod handler;
mod notify;

#[cfg(all(unix, not(target_os = "fuchsia")))]
pub mod unix;

pub use self::event_loop::{
    EventLoop,
    EventLoopBuilder,
    Sender,
};
pub use self::io::{
    TryAccept,
    TryRead,
    TryWrite,
};
pub use self::handler::{
    Handler,
};
pub use self::notify::{
    NotifyError,
};
#[cfg(all(unix, not(target_os = "fuchsia")))]
pub use self::unix::{
    pipe,
    PipeReader,
    PipeWriter,
    UnixListener,
    UnixSocket,
    UnixStream,
    Shutdown,
};
