use crate::{codec::Decode, util::PartialBuffer};

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::Result;
use xz2::stream::{Action, Status, Stream};

pub struct Xz2Decoder {
    stream: Stream,
}

impl Debug for Xz2Decoder {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "LzmaDecoder")
    }
}

impl Xz2Decoder {
    pub fn new() -> Self {
        Self {
            stream: Stream::new_auto_decoder(u64::max_value(), 0).unwrap(),
        }
    }
}

impl Decode for Xz2Decoder {
    fn reinit(&mut self) -> Result<()> {
        *self = Self::new();
        Ok(())
    }

    fn decode(
        &mut self,
        input: &mut PartialBuffer<impl AsRef<[u8]>>,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        let previous_in = self.stream.total_in() as usize;
        let previous_out = self.stream.total_out() as usize;

        let status = self
            .stream
            .process(input.unwritten(), output.unwritten_mut(), Action::Run)?;

        input.advance(self.stream.total_in() as usize - previous_in);
        output.advance(self.stream.total_out() as usize - previous_out);

        match status {
            Status::Ok => Ok(false),
            Status::StreamEnd => Ok(true),
            Status::GetCheck => panic!("Unexpected lzma integrity check"),
            Status::MemNeeded => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "More memory needed",
            )),
        }
    }

    fn flush(
        &mut self,
        _output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        // While decoding flush is a noop
        Ok(true)
    }

    fn finish(
        &mut self,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        let previous_out = self.stream.total_out() as usize;

        let status = self
            .stream
            .process(&[], output.unwritten_mut(), Action::Finish)?;

        output.advance(self.stream.total_out() as usize - previous_out);

        match status {
            Status::Ok => Ok(false),
            Status::StreamEnd => Ok(true),
            Status::GetCheck => panic!("Unexpected lzma integrity check"),
            Status::MemNeeded => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "More memory needed",
            )),
        }
    }
}
