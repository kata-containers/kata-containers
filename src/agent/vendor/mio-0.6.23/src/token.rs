/// Associates readiness notifications with [`Evented`] handles.
///
/// `Token` is a wrapper around `usize` and is used as an argument to
/// [`Poll::register`] and [`Poll::reregister`].
///
/// See [`Poll`] for more documentation on polling.
///
/// # Example
///
/// Using `Token` to track which socket generated the notification. In this
/// example, `HashMap` is used, but usually something like [`slab`] is better.
///
/// ```
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// use mio::{Events, Ready, Poll, PollOpt, Token};
/// use mio::net::TcpListener;
///
/// use std::thread;
/// use std::io::{self, Read};
/// use std::collections::HashMap;
///
/// // After this number of sockets is accepted, the server will shutdown.
/// const MAX_SOCKETS: usize = 32;
///
/// // Pick a token that will not be used by any other socket and use that one
/// // for the listener.
/// const LISTENER: Token = Token(1024);
///
/// // Used to store the sockets.
/// let mut sockets = HashMap::new();
///
/// // This is used to generate a unique token for a socket
/// let mut next_socket_index = 0;
///
/// // The `Poll` instance
/// let poll = Poll::new()?;
///
/// // Tcp listener
/// let listener = TcpListener::bind(&"127.0.0.1:0".parse()?)?;
///
/// // Register the listener
/// poll.register(&listener,
///               LISTENER,
///               Ready::readable(),
///               PollOpt::edge())?;
///
/// // Spawn a thread that will connect a bunch of sockets then close them
/// let addr = listener.local_addr()?;
/// thread::spawn(move || {
///     use std::net::TcpStream;
///
///     // +1 here is to connect an extra socket to signal the socket to close
///     for _ in 0..(MAX_SOCKETS+1) {
///         // Connect then drop the socket
///         let _ = TcpStream::connect(&addr).unwrap();
///     }
/// });
///
/// // Event storage
/// let mut events = Events::with_capacity(1024);
///
/// // Read buffer, this will never actually get filled
/// let mut buf = [0; 256];
///
/// // The main event loop
/// loop {
///     // Wait for events
///     poll.poll(&mut events, None)?;
///
///     for event in &events {
///         match event.token() {
///             LISTENER => {
///                 // Perform operations in a loop until `WouldBlock` is
///                 // encountered.
///                 loop {
///                     match listener.accept() {
///                         Ok((socket, _)) => {
///                             // Shutdown the server
///                             if next_socket_index == MAX_SOCKETS {
///                                 return Ok(());
///                             }
///
///                             // Get the token for the socket
///                             let token = Token(next_socket_index);
///                             next_socket_index += 1;
///
///                             // Register the new socket w/ poll
///                             poll.register(&socket,
///                                          token,
///                                          Ready::readable(),
///                                          PollOpt::edge())?;
///
///                             // Store the socket
///                             sockets.insert(token, socket);
///                         }
///                         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
///                             // Socket is not ready anymore, stop accepting
///                             break;
///                         }
///                         e => panic!("err={:?}", e), // Unexpected error
///                     }
///                 }
///             }
///             token => {
///                 // Always operate in a loop
///                 loop {
///                     match sockets.get_mut(&token).unwrap().read(&mut buf) {
///                         Ok(0) => {
///                             // Socket is closed, remove it from the map
///                             sockets.remove(&token);
///                             break;
///                         }
///                         // Data is not actually sent in this example
///                         Ok(_) => unreachable!(),
///                         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
///                             // Socket is not ready anymore, stop reading
///                             break;
///                         }
///                         e => panic!("err={:?}", e), // Unexpected error
///                     }
///                 }
///             }
///         }
///     }
/// }
/// #     Ok(())
/// # }
/// #
/// # fn main() {
/// #     try_main().unwrap();
/// # }
/// ```
///
/// [`Evented`]: event/trait.Evented.html
/// [`Poll`]: struct.Poll.html
/// [`Poll::register`]: struct.Poll.html#method.register
/// [`Poll::reregister`]: struct.Poll.html#method.reregister
/// [`slab`]: https://crates.io/crates/slab
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(pub usize);

impl From<usize> for Token {
    fn from(val: usize) -> Token {
        Token(val)
    }
}

impl From<Token> for usize {
    fn from(val: Token) -> usize {
        val.0
    }
}
