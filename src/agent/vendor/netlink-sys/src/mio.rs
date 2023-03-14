use crate::Socket;
use std::os::unix::io::AsRawFd;

use mio::{event::Source, unix::SourceFd};

impl Source for Socket {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        let raw_fd = self.as_raw_fd();

        SourceFd(&raw_fd).register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        let raw_fd = self.as_raw_fd();

        SourceFd(&raw_fd).reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        let raw_fd = self.as_raw_fd();

        SourceFd(&raw_fd).deregister(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_neighbour_dump(socket: &mut Socket) -> std::io::Result<()> {
        // Buffer generated from:
        // ```
        // let mut neighbour_dump_request = NetlinkMessage {
        //     header: NetlinkHeader {
        //         flags: NLM_F_DUMP | NLM_F_REQUEST,
        //         ..Default::default()
        //     },
        //     payload: NetlinkPayload::from(RtnlMessage::GetNeighbour(NeighbourMessage::default())),
        // };
        // ```
        let buf = [
            28, 0, 0, 0, 30, 0, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        socket.send(&buf[..], 0)?;

        Ok(())
    }

    #[test]
    fn test_event_loop() -> Result<(), Box<dyn std::error::Error>> {
        use crate::{protocols::NETLINK_ROUTE, Socket, SocketAddr};
        use mio::{Events, Interest, Poll, Token};
        use std::time::Duration;

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(128);

        let mut socket = Socket::new(NETLINK_ROUTE)?;
        socket.bind_auto()?;
        socket.connect(&SocketAddr::new(0, 0))?;
        poll.registry()
            .register(&mut socket, Token(1), Interest::READABLE)?;

        // Send neighbour query
        request_neighbour_dump(&mut socket)?;

        // Make sure that we got anything
        poll.poll(&mut events, Some(Duration::from_secs(1)))?;
        assert!(!events.is_empty());

        // Make sure the we didn't get a thing after removing socket from loop
        poll.registry().deregister(&mut socket)?;
        poll.poll(&mut events, Some(Duration::from_secs(1)))?;
        assert!(events.is_empty());

        Ok(())
    }
}
