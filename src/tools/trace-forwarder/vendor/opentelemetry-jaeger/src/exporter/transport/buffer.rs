use std::io;
use std::sync::{Arc, Mutex};
use thrift::transport::{ReadHalf, TIoChannel, WriteHalf};

/// Custom TBufferChannel that can be dynamically grown and split off of.
#[derive(Debug, Clone)]
pub(crate) struct TBufferChannel {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl TBufferChannel {
    /// Create a new buffer channel with the given initial capacity
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        TBufferChannel {
            inner: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
        }
    }

    /// Take the accumulated bytes from the buffer, leaving capacity unchanged.
    pub(crate) fn take_bytes(&mut self) -> Vec<u8> {
        self.inner
            .lock()
            .map(|mut write| write.split_off(0))
            .unwrap_or_default()
    }
}

impl io::Read for TBufferChannel {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        unreachable!("jaeger protocol never reads")
    }
}

impl io::Write for TBufferChannel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut inner) = self.inner.lock() {
            inner.extend_from_slice(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TIoChannel for TBufferChannel {
    fn split(self) -> thrift::Result<(ReadHalf<Self>, WriteHalf<Self>)>
    where
        Self: Sized,
    {
        Ok((ReadHalf::new(self.clone()), WriteHalf::new(self)))
    }
}
