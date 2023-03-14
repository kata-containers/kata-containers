//! Tokio wrappers which apply timeouts to IO operations.
//!
//! These timeouts are analogous to the read and write timeouts on traditional blocking sockets. A timeout countdown is
//! initiated when a read/write operation returns [`Poll::Pending`]. If a read/write does not return successfully before
//! the countdown expires, an [`io::Error`] with a kind of [`TimedOut`](io::ErrorKind::TimedOut) is returned.
#![doc(html_root_url = "https://docs.rs/tokio-io-timeout/1")]
#![warn(missing_docs)]

use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::io::SeekFrom;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use tokio::time::{sleep_until, Instant, Sleep};

pin_project! {
    #[derive(Debug)]
    struct TimeoutState {
        timeout: Option<Duration>,
        #[pin]
        cur: Sleep,
        active: bool,
    }
}

impl TimeoutState {
    #[inline]
    fn new() -> TimeoutState {
        TimeoutState {
            timeout: None,
            cur: sleep_until(Instant::now()),
            active: false,
        }
    }

    #[inline]
    fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    #[inline]
    fn set_timeout(&mut self, timeout: Option<Duration>) {
        // since this takes &mut self, we can't yet be active
        self.timeout = timeout;
    }

    #[inline]
    fn set_timeout_pinned(mut self: Pin<&mut Self>, timeout: Option<Duration>) {
        *self.as_mut().project().timeout = timeout;
        self.reset();
    }

    #[inline]
    fn reset(self: Pin<&mut Self>) {
        let this = self.project();

        if *this.active {
            *this.active = false;
            this.cur.reset(Instant::now());
        }
    }

    #[inline]
    fn poll_check(self: Pin<&mut Self>, cx: &mut Context<'_>) -> io::Result<()> {
        let mut this = self.project();

        let timeout = match this.timeout {
            Some(timeout) => *timeout,
            None => return Ok(()),
        };

        if !*this.active {
            this.cur.as_mut().reset(Instant::now() + timeout);
            *this.active = true;
        }

        match this.cur.poll(cx) {
            Poll::Ready(()) => Err(io::Error::from(io::ErrorKind::TimedOut)),
            Poll::Pending => Ok(()),
        }
    }
}

pin_project! {
    /// An `AsyncRead`er which applies a timeout to read operations.
    #[derive(Debug)]
    pub struct TimeoutReader<R> {
        #[pin]
        reader: R,
        #[pin]
        state: TimeoutState,
    }
}

impl<R> TimeoutReader<R>
where
    R: AsyncRead,
{
    /// Returns a new `TimeoutReader` wrapping the specified reader.
    ///
    /// There is initially no timeout.
    pub fn new(reader: R) -> TimeoutReader<R> {
        TimeoutReader {
            reader,
            state: TimeoutState::new(),
        }
    }

    /// Returns the current read timeout.
    pub fn timeout(&self) -> Option<Duration> {
        self.state.timeout()
    }

    /// Sets the read timeout.
    ///
    /// This can only be used before the reader is pinned; use [`set_timeout_pinned`](Self::set_timeout_pinned)
    /// otherwise.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.state.set_timeout(timeout);
    }

    /// Sets the read timeout.
    ///
    /// This will reset any pending timeout. Use [`set_timeout`](Self::set_timeout) instead if the reader is not yet
    /// pinned.
    pub fn set_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project().state.set_timeout_pinned(timeout);
    }

    /// Returns a shared reference to the inner reader.
    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    /// Returns a mutable reference to the inner reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Returns a pinned mutable reference to the inner reader.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut R> {
        self.project().reader
    }

    /// Consumes the `TimeoutReader`, returning the inner reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R> AsyncRead for TimeoutReader<R>
where
    R: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.reader.poll_read(cx, buf);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }
}

impl<R> AsyncWrite for TimeoutReader<R>
where
    R: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().reader.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        self.project().reader.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        self.project().reader.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.project().reader.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.reader.is_write_vectored()
    }
}

impl<R> AsyncSeek for TimeoutReader<R>
where
    R: AsyncSeek,
{
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        self.project().reader.start_seek(position)
    }
    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        self.project().reader.poll_complete(cx)
    }
}

pin_project! {
    /// An `AsyncWrite`er which applies a timeout to write operations.
    #[derive(Debug)]
    pub struct TimeoutWriter<W> {
        #[pin]
        writer: W,
        #[pin]
        state: TimeoutState,
    }
}

impl<W> TimeoutWriter<W>
where
    W: AsyncWrite,
{
    /// Returns a new `TimeoutReader` wrapping the specified reader.
    ///
    /// There is initially no timeout.
    pub fn new(writer: W) -> TimeoutWriter<W> {
        TimeoutWriter {
            writer,
            state: TimeoutState::new(),
        }
    }

    /// Returns the current write timeout.
    pub fn timeout(&self) -> Option<Duration> {
        self.state.timeout()
    }

    /// Sets the write timeout.
    ///
    /// This can only be used before the writer is pinned; use [`set_timeout_pinned`](Self::set_timeout_pinned)
    /// otherwise.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.state.set_timeout(timeout);
    }

    /// Sets the write timeout.
    ///
    /// This will reset any pending timeout. Use [`set_timeout`](Self::set_timeout) instead if the reader is not yet
    /// pinned.
    pub fn set_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project().state.set_timeout_pinned(timeout);
    }

    /// Returns a shared reference to the inner writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Returns a mutable reference to the inner writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Returns a pinned mutable reference to the inner writer.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer
    }

    /// Consumes the `TimeoutWriter`, returning the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W> AsyncWrite for TimeoutWriter<W>
where
    W: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        let r = this.writer.poll_write(cx, buf);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.writer.poll_flush(cx);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        let r = this.writer.poll_shutdown(cx);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        let r = this.writer.poll_write_vectored(cx, bufs);
        match r {
            Poll::Pending => this.state.poll_check(cx)?,
            _ => this.state.reset(),
        }
        r
    }

    fn is_write_vectored(&self) -> bool {
        self.writer.is_write_vectored()
    }
}

impl<W> AsyncRead for TimeoutWriter<W>
where
    W: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), io::Error>> {
        self.project().writer.poll_read(cx, buf)
    }
}

impl<W> AsyncSeek for TimeoutWriter<W>
where
    W: AsyncSeek,
{
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        self.project().writer.start_seek(position)
    }
    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        self.project().writer.poll_complete(cx)
    }
}

pin_project! {
    /// A stream which applies read and write timeouts to an inner stream.
    #[derive(Debug)]
    pub struct TimeoutStream<S> {
        #[pin]
        stream: TimeoutReader<TimeoutWriter<S>>
    }
}

impl<S> TimeoutStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    /// Returns a new `TimeoutStream` wrapping the specified stream.
    ///
    /// There is initially no read or write timeout.
    pub fn new(stream: S) -> TimeoutStream<S> {
        let writer = TimeoutWriter::new(stream);
        let stream = TimeoutReader::new(writer);
        TimeoutStream { stream }
    }

    /// Returns the current read timeout.
    pub fn read_timeout(&self) -> Option<Duration> {
        self.stream.timeout()
    }

    /// Sets the read timeout.
    ///
    /// This can only be used before the stream is pinned; use
    /// [`set_read_timeout_pinned`](Self::set_read_timeout_pinned) otherwise.
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) {
        self.stream.set_timeout(timeout)
    }

    /// Sets the read timeout.
    ///
    /// This will reset any pending read timeout. Use [`set_read_timeout`](Self::set_read_timeout) instead if the stream
    /// has not yet been pinned.
    pub fn set_read_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project().stream.set_timeout_pinned(timeout)
    }

    /// Returns the current write timeout.
    pub fn write_timeout(&self) -> Option<Duration> {
        self.stream.get_ref().timeout()
    }

    /// Sets the write timeout.
    ///
    /// This can only be used before the stream is pinned; use
    /// [`set_write_timeout_pinned`](Self::set_write_timeout_pinned) otherwise.
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) {
        self.stream.get_mut().set_timeout(timeout)
    }

    /// Sets the write timeout.
    ///
    /// This will reset any pending write timeout. Use [`set_write_timeout`](Self::set_write_timeout) instead if the
    /// stream has not yet been pinned.
    pub fn set_write_timeout_pinned(self: Pin<&mut Self>, timeout: Option<Duration>) {
        self.project()
            .stream
            .get_pin_mut()
            .set_timeout_pinned(timeout)
    }

    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref().get_ref()
    }

    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut().get_mut()
    }

    /// Returns a pinned mutable reference to the inner stream.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        self.project().stream.get_pin_mut().get_pin_mut()
    }

    /// Consumes the stream, returning the inner stream.
    pub fn into_inner(self) -> S {
        self.stream.into_inner().into_inner()
    }
}

impl<S> AsyncRead for TimeoutStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for TimeoutStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.project().stream.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        self.project().stream.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.project().stream.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::pin;

    pin_project! {
        struct DelayStream {
            #[pin]
            sleep: Sleep,
        }
    }

    impl DelayStream {
        fn new(until: Instant) -> Self {
            DelayStream {
                sleep: sleep_until(until),
            }
        }
    }

    impl AsyncRead for DelayStream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context,
            _buf: &mut ReadBuf,
        ) -> Poll<Result<(), io::Error>> {
            match self.project().sleep.poll(cx) {
                Poll::Ready(()) => Poll::Ready(Ok(())),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    impl AsyncWrite for DelayStream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            match self.project().sleep.poll(cx) {
                Poll::Ready(()) => Poll::Ready(Ok(buf.len())),
                Poll::Pending => Poll::Pending,
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn read_timeout() {
        let reader = DelayStream::new(Instant::now() + Duration::from_millis(500));
        let mut reader = TimeoutReader::new(reader);
        reader.set_timeout(Some(Duration::from_millis(100)));
        pin!(reader);

        let r = reader.read(&mut [0]).await;
        assert_eq!(r.err().unwrap().kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn read_ok() {
        let reader = DelayStream::new(Instant::now() + Duration::from_millis(100));
        let mut reader = TimeoutReader::new(reader);
        reader.set_timeout(Some(Duration::from_millis(500)));
        pin!(reader);

        reader.read(&mut [0]).await.unwrap();
    }

    #[tokio::test]
    async fn write_timeout() {
        let writer = DelayStream::new(Instant::now() + Duration::from_millis(500));
        let mut writer = TimeoutWriter::new(writer);
        writer.set_timeout(Some(Duration::from_millis(100)));
        pin!(writer);

        let r = writer.write(&[0]).await;
        assert_eq!(r.err().unwrap().kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn write_ok() {
        let writer = DelayStream::new(Instant::now() + Duration::from_millis(100));
        let mut writer = TimeoutWriter::new(writer);
        writer.set_timeout(Some(Duration::from_millis(500)));
        pin!(writer);

        writer.write(&[0]).await.unwrap();
    }

    #[tokio::test]
    async fn tcp_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let mut socket = listener.accept().unwrap().0;
            thread::sleep(Duration::from_millis(10));
            socket.write_all(b"f").unwrap();
            thread::sleep(Duration::from_millis(500));
            let _ = socket.write_all(b"f"); // this may hit an eof
        });

        let s = TcpStream::connect(&addr).await.unwrap();
        let mut s = TimeoutStream::new(s);
        s.set_read_timeout(Some(Duration::from_millis(100)));
        pin!(s);
        s.read(&mut [0]).await.unwrap();
        let r = s.read(&mut [0]).await;

        match r {
            Ok(_) => panic!("unexpected success"),
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => panic!("{:?}", e),
        }
    }
}
