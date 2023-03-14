use futures_core::ready;

use std::{
    collections::VecDeque,
    fmt::Debug,
    future::Future,
    marker::PhantomData,
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{
    guid::Guid,
    raw::{self, Handshake as SyncHandshake, Socket},
    Result,
};

/// Authentication mechanisms
///
/// See <https://dbus.freedesktop.org/doc/dbus-specification.html#auth-mechanisms>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMechanism {
    /// This is the recommended authentication mechanism on platforms where credentials can be
    /// transferred out-of-band, in particular Unix platforms that can perform credentials-passing
    /// over the `unix:` transport.
    External,

    /// This mechanism is designed to establish that a client has the ability to read a private file
    /// owned by the user being authenticated.
    Cookie,

    /// Does not perform any authentication at all, and should not be accepted by message buses.
    /// However, it might sometimes be useful for non-message-bus uses of D-Bus.
    Anonymous,
}

/// The asynchronous authentication implementation based on non-blocking [`raw::Handshake`].
///
/// The underlying socket is in nonblocking mode. Enabling blocking mode on it, will lead to
/// undefined behaviour.
pub(crate) struct Authenticated<S>(raw::Authenticated<S>);

impl<S> Authenticated<S> {
    /// Unwraps the inner [`raw::Authenticated`].
    pub fn into_inner(self) -> raw::Authenticated<S> {
        self.0
    }
}

impl<S> Deref for Authenticated<S> {
    type Target = raw::Authenticated<S>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> Authenticated<S>
where
    S: Socket + Unpin,
{
    /// Create a client-side `Authenticated` for the given `socket`.
    pub async fn client(socket: S, mechanisms: Option<VecDeque<AuthMechanism>>) -> Result<Self> {
        Handshake {
            handshake: Some(raw::ClientHandshake::new(socket, mechanisms)),
            phantom: PhantomData,
        }
        .await
    }

    /// Create a server-side `Authenticated` for the given `socket`.
    ///
    /// The function takes `client_uid` on Unix only.
    pub async fn server(
        socket: S,
        guid: Guid,
        #[cfg(unix)] client_uid: u32,
        #[cfg(windows)] client_sid: Option<String>,
        auth_mechanisms: Option<VecDeque<AuthMechanism>>,
    ) -> Result<Self> {
        Handshake {
            handshake: Some(raw::ServerHandshake::new(
                socket,
                guid,
                #[cfg(unix)]
                client_uid,
                #[cfg(windows)]
                client_sid,
                auth_mechanisms,
            )?),
            phantom: PhantomData,
        }
        .await
    }
}

struct Handshake<H, S> {
    handshake: Option<H>,
    phantom: PhantomData<S>,
}

impl<H, S> Future for Handshake<H, S>
where
    H: SyncHandshake<S> + Unpin + Debug,
    S: Unpin,
{
    type Output = Result<Authenticated<S>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let self_mut = &mut self.get_mut();
        let handshake = self_mut
            .handshake
            .as_mut()
            .expect("ClientHandshake::poll() called unexpectedly");

        ready!(handshake.advance_handshake(cx))?;

        let handshake = self_mut
            .handshake
            .take()
            .expect("<Handshake as Future>::poll() called unexpectedly");
        let authenticated = handshake
            .try_finish()
            .expect("Failed to finish a successful handshake");

        Poll::Ready(Ok(Authenticated(authenticated)))
    }
}

#[cfg(all(unix, feature = "async-io"))]
#[cfg(test)]
mod tests {
    use async_io::Async;
    use nix::unistd::Uid;
    use std::os::unix::net::UnixStream;
    use test_log::test;

    use super::*;

    use crate::{Guid, Result};

    #[test]
    fn async_handshake() {
        crate::utils::block_on(handshake()).unwrap();
    }

    async fn handshake() -> Result<()> {
        // a pair of non-blocking connection UnixStream
        let (p0, p1) = UnixStream::pair()?;

        // initialize both handshakes
        let client = Authenticated::client(Async::new(p0)?, None);
        let server = Authenticated::server(
            Async::new(p1)?,
            Guid::generate(),
            Uid::current().into(),
            None,
        );

        // proceed to the handshakes
        let (client_auth, server_auth) = futures_util::try_join!(client, server)?;

        assert_eq!(client_auth.server_guid, server_auth.server_guid);
        assert_eq!(client_auth.cap_unix_fd, server_auth.cap_unix_fd);

        Ok(())
    }
}
