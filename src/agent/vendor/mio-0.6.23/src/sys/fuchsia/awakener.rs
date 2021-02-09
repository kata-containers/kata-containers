use {io, poll, Evented, Ready, Poll, PollOpt, Token};
use zircon;
use std::sync::{Arc, Mutex, Weak};

pub struct Awakener {
    /// Token and weak reference to the port on which Awakener was registered.
    ///
    /// When `Awakener::wakeup` is called, these are used to send a wakeup message to the port.
    inner: Mutex<Option<(Token, Weak<zircon::Port>)>>,
}

impl Awakener {
    /// Create a new `Awakener`.
    pub fn new() -> io::Result<Awakener> {
        Ok(Awakener {
            inner: Mutex::new(None)
        })
    }

    /// Send a wakeup signal to the `Selector` on which the `Awakener` was registered.
    pub fn wakeup(&self) -> io::Result<()> {
        let inner_locked = self.inner.lock().unwrap();
        let &(token, ref weak_port) =
            inner_locked.as_ref().expect("Called wakeup on unregistered awakener.");

        let port = weak_port.upgrade().expect("Tried to wakeup a closed port.");

        let status = 0; // arbitrary
        let packet = zircon::Packet::from_user_packet(
            token.0 as u64, status, zircon::UserPacket::from_u8_array([0; 32]));

        Ok(port.queue(&packet)?)
    }

    pub fn cleanup(&self) {}
}

impl Evented for Awakener {
    fn register(&self,
                poll: &Poll,
                token: Token,
                _events: Ready,
                _opts: PollOpt) -> io::Result<()>
    {
        let mut inner_locked = self.inner.lock().unwrap();
        if inner_locked.is_some() {
            panic!("Called register on already-registered Awakener.");
        }
        *inner_locked = Some((token, Arc::downgrade(poll::selector(poll).port())));

        Ok(())
    }

    fn reregister(&self,
                  poll: &Poll,
                  token: Token,
                  _events: Ready,
                  _opts: PollOpt) -> io::Result<()>
    {
        let mut inner_locked = self.inner.lock().unwrap();
        *inner_locked = Some((token, Arc::downgrade(poll::selector(poll).port())));

        Ok(())
    }

    fn deregister(&self, _poll: &Poll) -> io::Result<()>
    {
        let mut inner_locked = self.inner.lock().unwrap();
        *inner_locked = None;

        Ok(())
    }
}