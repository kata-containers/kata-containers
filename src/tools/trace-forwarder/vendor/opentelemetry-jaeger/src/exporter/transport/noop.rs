use std::io;

#[derive(Debug)]
pub(crate) struct TNoopChannel;

impl io::Read for TNoopChannel {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}
