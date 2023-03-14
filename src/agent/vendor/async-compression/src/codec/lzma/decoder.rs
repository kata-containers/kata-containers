use crate::{codec::Decode, util::PartialBuffer};

use std::io::Result;

#[derive(Debug)]
pub struct LzmaDecoder {
    inner: crate::codec::Xz2Decoder,
}

impl LzmaDecoder {
    pub fn new() -> Self {
        Self {
            inner: crate::codec::Xz2Decoder::new(),
        }
    }
}

impl Decode for LzmaDecoder {
    fn reinit(&mut self) -> Result<()> {
        self.inner.reinit()
    }

    fn decode(
        &mut self,
        input: &mut PartialBuffer<impl AsRef<[u8]>>,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        self.inner.decode(input, output)
    }

    fn flush(
        &mut self,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        self.inner.flush(output)
    }

    fn finish(
        &mut self,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        self.inner.finish(output)
    }
}
