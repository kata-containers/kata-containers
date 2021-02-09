use {io, poll, Ready, Poll, PollOpt, Token};
use event::Evented;
use std::os::unix::io::RawFd;

/*
 *
 * ===== EventedFd =====
 *
 */

#[derive(Debug)]

/// Adapter for [`RawFd`] providing an [`Evented`] implementation.
///
/// `EventedFd` enables registering any type with an FD with [`Poll`].
///
/// While only implementations for TCP and UDP are provided, Mio supports
/// registering any FD that can be registered with the underlying OS selector.
/// `EventedFd` provides the necessary bridge.
///
/// Note that `EventedFd` takes a `&RawFd`. This is because `EventedFd` **does
/// not** take ownership of the FD. Specifically, it will not manage any
/// lifecycle related operations, such as closing the FD on drop. It is expected
/// that the `EventedFd` is constructed right before a call to
/// [`Poll::register`]. See the examples for more detail.
///
/// # Examples
///
/// Basic usage
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Ready, Poll, PollOpt, Token};
/// use mio::unix::EventedFd;
///
/// use std::os::unix::io::AsRawFd;
/// use std::net::TcpListener;
///
/// // Bind a std listener
/// let listener = TcpListener::bind("127.0.0.1:0")?;
///
/// let poll = Poll::new()?;
///
/// // Register the listener
/// poll.register(&EventedFd(&listener.as_raw_fd()),
///              Token(0), Ready::readable(), PollOpt::edge())?;
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// Implementing [`Evented`] for a custom type backed by a [`RawFd`].
///
/// ```
/// use mio::{Ready, Poll, PollOpt, Token};
/// use mio::event::Evented;
/// use mio::unix::EventedFd;
///
/// use std::os::unix::io::RawFd;
/// use std::io;
///
/// pub struct MyIo {
///     fd: RawFd,
/// }
///
/// impl Evented for MyIo {
///     fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
///         -> io::Result<()>
///     {
///         EventedFd(&self.fd).register(poll, token, interest, opts)
///     }
///
///     fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt)
///         -> io::Result<()>
///     {
///         EventedFd(&self.fd).reregister(poll, token, interest, opts)
///     }
///
///     fn deregister(&self, poll: &Poll) -> io::Result<()> {
///         EventedFd(&self.fd).deregister(poll)
///     }
/// }
/// ```
///
/// [`RawFd`]: https://doc.rust-lang.org/std/os/unix/io/type.RawFd.html
/// [`Evented`]: ../event/trait.Evented.html
/// [`Poll`]: ../struct.Poll.html
/// [`Poll::register`]: ../struct.Poll.html#method.register
pub struct EventedFd<'a>(pub &'a RawFd);

impl<'a> Evented for EventedFd<'a> {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        poll::selector(poll).register(*self.0, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        poll::selector(poll).reregister(*self.0, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        poll::selector(poll).deregister(*self.0)
    }
}
