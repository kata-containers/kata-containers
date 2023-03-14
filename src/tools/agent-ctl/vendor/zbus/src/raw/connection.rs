use std::{
    collections::VecDeque,
    io,
    task::{Context, Poll},
};

use event_listener::{Event, EventListener};

#[cfg(unix)]
use crate::OwnedFd;
use crate::{
    message_header::MIN_MESSAGE_SIZE, raw::Socket, utils::padding_for_8_bytes, Message,
    MessagePrimaryHeader,
};

use futures_core::ready;

/// A low-level representation of a D-Bus connection
///
/// This wrapper is agnostic on the actual transport, using the `Socket` trait
/// to abstract it. It is compatible with sockets both in blocking or non-blocking
/// mode.
///
/// This wrapper abstracts away the serialization & buffering considerations of the
/// protocol, and allows interaction based on messages, rather than bytes.
#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Connection<S> {
    #[derivative(Debug = "ignore")]
    socket: S,
    event: Event,
    raw_in_buffer: Vec<u8>,
    #[cfg(unix)]
    raw_in_fds: Vec<OwnedFd>,
    raw_in_pos: usize,
    out_pos: usize,
    out_msgs: VecDeque<Message>,
    prev_seq: u64,
}

impl<S: Socket> Connection<S> {
    pub(crate) fn wrap(socket: S) -> Connection<S> {
        Connection {
            socket,
            event: Event::new(),
            raw_in_buffer: vec![],
            #[cfg(unix)]
            raw_in_fds: vec![],
            raw_in_pos: 0,
            out_pos: 0,
            out_msgs: VecDeque::new(),
            prev_seq: 0,
        }
    }

    /// Attempt to flush the outgoing buffer
    ///
    /// This will try to write as many messages as possible from the
    /// outgoing buffer into the socket, until an error is encountered.
    ///
    /// This method will thus only block if the socket is in blocking mode.
    pub fn try_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.event.notify(usize::MAX);
        while let Some(msg) = self.out_msgs.front() {
            loop {
                let data = &msg.as_bytes()[self.out_pos..];
                if data.is_empty() {
                    self.out_pos = 0;
                    self.out_msgs.pop_front();
                    break;
                }
                #[cfg(unix)]
                let fds = if self.out_pos == 0 { msg.fds() } else { vec![] };
                self.out_pos += ready!(self.socket.poll_sendmsg(
                    cx,
                    data,
                    #[cfg(unix)]
                    &fds,
                ))?;
            }
        }
        Poll::Ready(Ok(()))
    }

    /// Enqueue a message to be sent out to the socket
    ///
    /// This method will *not* write anything to the socket, you need to call
    /// `try_flush()` afterwards so that your message is actually sent out.
    pub fn enqueue_message(&mut self, msg: Message) {
        self.out_msgs.push_back(msg);
    }

    /// Attempt to read a message from the socket
    ///
    /// This methods will read from the socket until either a full D-Bus message is
    /// read or an error is encountered.
    ///
    /// If the socket is in non-blocking mode, it may read a partial message. In such case it
    /// will buffer it internally and try to complete it the next time you call `try_receive_message`.
    pub fn try_receive_message(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<Message>> {
        self.event.notify(usize::MAX);
        if self.raw_in_pos < MIN_MESSAGE_SIZE {
            self.raw_in_buffer.resize(MIN_MESSAGE_SIZE, 0);
            // We don't have enough data to make a proper message header yet.
            // Some partial read may be in raw_in_buffer, so we try to complete it
            // until we have MIN_MESSAGE_SIZE bytes
            //
            // Given that MIN_MESSAGE_SIZE is 16, this codepath is actually extremely unlikely
            // to be taken more than once
            while self.raw_in_pos < MIN_MESSAGE_SIZE {
                let res = ready!(self
                    .socket
                    .poll_recvmsg(cx, &mut self.raw_in_buffer[self.raw_in_pos..]))?;
                let len = {
                    #[cfg(unix)]
                    {
                        let (len, fds) = res;
                        self.raw_in_fds.extend(fds);
                        len
                    }
                    #[cfg(not(unix))]
                    {
                        res
                    }
                };
                self.raw_in_pos += len;
                if len == 0 {
                    return Poll::Ready(Err(crate::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "failed to receive message",
                    ))));
                }
            }

            let (primary_header, fields_len) = MessagePrimaryHeader::read(&self.raw_in_buffer)?;
            let header_len = MIN_MESSAGE_SIZE + fields_len as usize;
            let body_padding = padding_for_8_bytes(header_len);
            let body_len = primary_header.body_len() as usize;

            // We now have a full message header, so we know the exact length of the complete message
            self.raw_in_buffer
                .resize(header_len + body_padding + body_len, 0);
        }

        // Now we have an incomplete message; read the rest
        while self.raw_in_buffer.len() > self.raw_in_pos {
            let res = ready!(self
                .socket
                .poll_recvmsg(cx, &mut self.raw_in_buffer[self.raw_in_pos..]))?;
            let read = {
                #[cfg(unix)]
                {
                    let (read, fds) = res;
                    self.raw_in_fds.extend(fds);
                    read
                }
                #[cfg(not(unix))]
                {
                    res
                }
            };
            self.raw_in_pos += read;
        }

        // If we reach here, the message is complete; return it
        self.raw_in_pos = 0;
        let bytes = std::mem::take(&mut self.raw_in_buffer);
        #[cfg(unix)]
        let fds = std::mem::take(&mut self.raw_in_fds);
        let seq = self.prev_seq + 1;
        self.prev_seq = seq;
        Poll::Ready(Message::from_raw_parts(
            bytes,
            #[cfg(unix)]
            fds,
            seq,
        ))
    }

    /// Close the connection.
    ///
    /// After this call, all reading and writing operations will fail.
    pub fn close(&self) -> crate::Result<()> {
        self.event.notify(usize::MAX);
        self.socket().close().map_err(|e| e.into())
    }

    /// Access the underlying socket
    ///
    /// This method is intended to provide access to the socket in order to register it
    /// to you event loop, for async integration.
    ///
    /// You should not try to read or write from it directly, as it may
    /// corrupt the internal state of this wrapper.
    pub fn socket(&self) -> &S {
        &self.socket
    }

    pub(crate) fn monitor_activity(&self) -> EventListener {
        self.event.listen()
    }
}

impl Connection<Box<dyn Socket>> {
    /// Same as `try_flush` above, except it wraps the method for use in [`std::future::Future`] impls.
    pub(crate) fn flush(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        self.try_flush(cx).map_err(Into::into)
    }
}

#[cfg(all(unix, feature = "async-io"))]
#[cfg(test)]
mod tests {
    use super::Connection;
    use crate::message::Message;
    use async_io::Async;
    use futures_util::future::poll_fn;
    use std::os::unix::net::UnixStream;
    use test_log::test;

    #[test]
    fn raw_send_receive() {
        let (p0, p1) = UnixStream::pair().unwrap();

        let mut conn0 = Connection::wrap(Async::new(p0).unwrap());
        let mut conn1 = Connection::wrap(Async::new(p1).unwrap());

        let msg = Message::method(
            None::<()>,
            None::<()>,
            "/",
            Some("org.zbus.p2p"),
            "Test",
            &(),
        )
        .unwrap();

        async_io::block_on(async {
            conn0.enqueue_message(msg);
            poll_fn(|cx| conn0.try_flush(cx)).await.unwrap();

            let ret = poll_fn(|cx| conn1.try_receive_message(cx)).await.unwrap();
            assert_eq!(ret.to_string(), "Method call Test");
        });
    }
}
