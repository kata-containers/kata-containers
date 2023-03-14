use bytes::{BufMut, BytesMut};
use std::{
    io,
    pin::Pin,
    slice,
    task::{Context, Poll},
};

use futures::{Sink, Stream};
use log::error;
use tokio_util::codec::{Decoder, Encoder};

use crate::sys::{Socket, SocketAddr};

pub struct NetlinkFramed<C> {
    socket: Socket,
    codec: C,
    reader: BytesMut,
    writer: BytesMut,
    in_addr: SocketAddr,
    out_addr: SocketAddr,
    flushed: bool,
}

impl<C> Stream for NetlinkFramed<C>
where
    C: Decoder + Unpin,
    C::Error: std::error::Error,
{
    type Item = (C::Item, SocketAddr);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Self {
            ref mut codec,
            ref mut socket,
            ref mut in_addr,
            ref mut reader,
            ..
        } = Pin::get_mut(self);

        loop {
            match codec.decode(reader) {
                Ok(Some(item)) => return Poll::Ready(Some((item, *in_addr))),
                Ok(None) => {}
                Err(e) => {
                    error!("unrecoverable error in decoder: {:?}", e);
                    return Poll::Ready(None);
                }
            }

            reader.clear();
            reader.reserve(INITIAL_READER_CAPACITY);

            *in_addr = unsafe {
                // Read into the buffer without having to initialize the memory.
                //
                // safety: we know poll_recv_from never reads from the
                // memory during a recv so it's fine to turn &mut
                // [<MaybeUninitialized<u8>>] into &mut[u8]
                let bytes = reader.chunk_mut();
                let bytes = slice::from_raw_parts_mut(bytes.as_mut_ptr(), bytes.len());
                match ready!(socket.poll_recv_from(cx, bytes)) {
                    Ok((n, addr)) => {
                        reader.advance_mut(n);
                        addr
                    }
                    Err(e) => {
                        error!("failed to read from netlink socket: {:?}", e);
                        return Poll::Ready(None);
                    }
                }
            };
        }
    }
}

impl<C: Encoder<Item> + Unpin, Item> Sink<(Item, SocketAddr)> for NetlinkFramed<C> {
    type Error = C::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.flushed {
            match self.poll_flush(cx)? {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }

        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: (Item, SocketAddr)) -> Result<(), Self::Error> {
        trace!("sending frame");
        let (frame, out_addr) = item;
        let pin = self.get_mut();
        pin.codec.encode(frame, &mut pin.writer)?;
        pin.out_addr = out_addr;
        pin.flushed = false;
        trace!("frame encoded; length={}", pin.writer.len());
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.flushed {
            return Poll::Ready(Ok(()));
        }

        trace!("flushing frame; length={}", self.writer.len());
        let Self {
            ref mut socket,
            ref mut out_addr,
            ref mut writer,
            ..
        } = *self;

        let n = ready!(socket.poll_send_to(cx, &writer, &out_addr))?;
        trace!("written {}", n);

        let wrote_all = n == self.writer.len();
        self.writer.clear();
        self.flushed = true;

        let res = if wrote_all {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write entire datagram to socket",
            )
            .into())
        };

        Poll::Ready(res)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }
}

// The theoritical max netlink packet size is 32KB for a netlink
// message since Linux 4.9 (16KB before). See:
// https://git.kernel.org/pub/scm/linux/kernel/git/davem/net-next.git/commit/?id=d35c99ff77ecb2eb239731b799386f3b3637a31e
const INITIAL_READER_CAPACITY: usize = 64 * 1024;
const INITIAL_WRITER_CAPACITY: usize = 8 * 1024;

impl<C> NetlinkFramed<C> {
    /// Create a new `NetlinkFramed` backed by the given socket and codec.
    ///
    /// See struct level documentation for more details.
    pub fn new(socket: Socket, codec: C) -> NetlinkFramed<C> {
        NetlinkFramed {
            socket,
            codec,
            out_addr: SocketAddr::new(0, 0),
            in_addr: SocketAddr::new(0, 0),
            reader: BytesMut::with_capacity(INITIAL_READER_CAPACITY),
            writer: BytesMut::with_capacity(INITIAL_WRITER_CAPACITY),
            flushed: true,
        }
    }

    /// Returns a reference to the underlying I/O stream wrapped by `Framed`.
    ///
    /// # Note
    ///
    /// Care should be taken to not tamper with the underlying stream of data
    /// coming in as it may corrupt the stream of frames otherwise being worked
    /// with.
    pub fn get_ref(&self) -> &Socket {
        &self.socket
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `Framed`.
    ///
    /// # Note
    ///
    /// Care should be taken to not tamper with the underlying stream of data
    /// coming in as it may corrupt the stream of frames otherwise being worked
    /// with.
    pub fn get_mut(&mut self) -> &mut Socket {
        &mut self.socket
    }

    /// Consumes the `Framed`, returning its underlying I/O stream.
    pub fn into_inner(self) -> Socket {
        self.socket
    }
}
