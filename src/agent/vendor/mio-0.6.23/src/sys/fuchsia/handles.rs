use {io, poll, Evented, Ready, Poll, PollOpt, Token};
use zircon_sys::zx_handle_t;
use std::sync::Mutex;

/// Wrapper for registering a `HandleBase` type with mio.
#[derive(Debug)]
pub struct EventedHandle {
    /// The handle to be registered.
    handle: zx_handle_t,

    /// The current `Token` with which the handle is registered with mio.
    token: Mutex<Option<Token>>,
}

impl EventedHandle {
    /// Create a new `EventedHandle` which can be registered with mio
    /// in order to receive event notifications.
    ///
    /// The underlying handle must not be dropped while the
    /// `EventedHandle` still exists.
    pub unsafe fn new(handle: zx_handle_t) -> Self {
        EventedHandle {
            handle: handle,
            token: Mutex::new(None),
        }
    }

    /// Get the underlying handle being registered.
    pub fn get_handle(&self) -> zx_handle_t {
        self.handle
    }
}

impl Evented for EventedHandle {
    fn register(&self,
                poll: &Poll,
                token: Token,
                interest: Ready,
                opts: PollOpt) -> io::Result<()>
    {
        let mut this_token = self.token.lock().unwrap();
        {
            poll::selector(poll).register_handle(self.handle, token, interest, opts)?;
            *this_token = Some(token);
        }
        Ok(())
    }

    fn reregister(&self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt) -> io::Result<()>
    {
        let mut this_token = self.token.lock().unwrap();
        {
            poll::selector(poll).deregister_handle(self.handle, token)?;
            *this_token = None;
            poll::selector(poll).register_handle(self.handle, token, interest, opts)?;
            *this_token = Some(token);
        }
        Ok(())
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        let mut this_token = self.token.lock().unwrap();
        let token = if let Some(token) = *this_token { token } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Attempted to deregister an unregistered handle."))
        };
        {
            poll::selector(poll).deregister_handle(self.handle, token)?;
            *this_token = None;
        }
        Ok(())
    }
}
