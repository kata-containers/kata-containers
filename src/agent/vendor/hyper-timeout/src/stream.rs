use std::io;
use std::io::IoSlice;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use hyper::client::connect::{Connected, Connection};
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_io_timeout::TimeoutStream;

pin_project! {
    /// A timeout stream that implements required traits to be a Connector
    #[derive(Debug)]
    pub struct TimeoutConnectorStream<S> {
        #[pin]
        stream: TimeoutStream<S>
    }
}

impl<S> TimeoutConnectorStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    /// Returns a new `TimeoutConnectorStream` wrapping the specified stream.
    ///
    /// There is initially no read or write timeout.
    pub fn new(stream: TimeoutStream<S>) -> TimeoutConnectorStream<S> {
        TimeoutConnectorStream { stream }
    }

    /// Returns the current read timeout.
    pub fn read_timeout(&self) -> Option<Duration> {
        self.stream.read_timeout()
    }

    /// Sets the read timeout.
    ///
    /// This can only be used before the stream is pinned; use
    /// [`set_read_timeout_pinned`](Self::set_read_timeout_pinned) otherwise.
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) {
        self.stream.set_read_timeout(timeout)
    }

    /// Sets the read timeout.
    ///
    /// This will reset any pending read timeout. Use
    /// [`set_read_timeout`](Self::set_read_timeout) instead if the stream has not yet been pinned.
    pub fn set_read_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project()
            .stream
            .as_mut()
            .set_read_timeout_pinned(timeout)
    }

    /// Returns the current write timeout.
    pub fn write_timeout(&self) -> Option<Duration> {
        self.stream.write_timeout()
    }

    /// Sets the write timeout.
    ///
    /// This can only be used before the stream is pinned; use
    /// [`set_write_timeout_pinned`](Self::set_write_timeout_pinned) otherwise.
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) {
        self.stream.set_write_timeout(timeout)
    }

    /// Sets the write timeout.
    ///
    /// This will reset any pending write timeout. Use
    /// [`set_write_timeout`](Self::set_write_timeout) instead if the stream has not yet been
    /// pinned.
    pub fn set_write_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project()
            .stream
            .as_mut()
            .set_write_timeout_pinned(timeout)
    }

    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref()
    }

    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut()
    }

    /// Returns a pinned mutable reference to the inner stream.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        self.project().stream.get_pin_mut()
    }

    /// Consumes the stream, returning the inner stream.
    pub fn into_inner(self) -> S {
        self.stream.into_inner()
    }
}

impl<S> AsyncRead for TimeoutConnectorStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for TimeoutConnectorStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().stream.poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().stream.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_shutdown(cx)
    }
}

impl<S> Connection for TimeoutConnectorStream<S>
where
    S: AsyncRead + AsyncWrite + Connection + Unpin,
{
    fn connected(&self) -> Connected {
        self.stream.get_ref().connected()
    }
}

impl<S> Connection for Pin<Box<TimeoutConnectorStream<S>>>
where
    S: AsyncRead + AsyncWrite + Connection + Unpin,
{
    fn connected(&self) -> Connected {
        self.stream.get_ref().connected()
    }
}
