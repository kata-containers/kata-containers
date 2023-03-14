use crate::{codec::Encode, util::PartialBuffer};
use std::{
    fmt,
    io::{Error, ErrorKind, Result},
};

use brotli::enc::{
    backward_references::BrotliEncoderParams,
    encode::{
        BrotliEncoderCompressStream, BrotliEncoderCreateInstance, BrotliEncoderHasMoreOutput,
        BrotliEncoderIsFinished, BrotliEncoderOperation, BrotliEncoderStateStruct,
    },
    StandardAlloc,
};

pub struct BrotliEncoder {
    state: BrotliEncoderStateStruct<StandardAlloc>,
}

impl BrotliEncoder {
    pub(crate) fn new(params: BrotliEncoderParams) -> Self {
        let mut state = BrotliEncoderCreateInstance(StandardAlloc::default());
        state.params = params;
        Self { state }
    }

    fn encode(
        &mut self,
        input: &mut PartialBuffer<impl AsRef<[u8]>>,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
        op: BrotliEncoderOperation,
    ) -> Result<()> {
        let in_buf = input.unwritten();
        let mut out_buf = output.unwritten_mut();

        let mut input_len = 0;
        let mut output_len = 0;

        if BrotliEncoderCompressStream(
            &mut self.state,
            op,
            &mut in_buf.len(),
            in_buf,
            &mut input_len,
            &mut out_buf.len(),
            out_buf,
            &mut output_len,
            &mut None,
            &mut |_, _, _, _| (),
        ) <= 0
        {
            return Err(Error::new(ErrorKind::Other, "brotli error"));
        }

        input.advance(input_len);
        output.advance(output_len);

        Ok(())
    }
}

impl Encode for BrotliEncoder {
    fn encode(
        &mut self,
        input: &mut PartialBuffer<impl AsRef<[u8]>>,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<()> {
        self.encode(
            input,
            output,
            BrotliEncoderOperation::BROTLI_OPERATION_PROCESS,
        )
    }

    fn flush(
        &mut self,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        self.encode(
            &mut PartialBuffer::new(&[][..]),
            output,
            BrotliEncoderOperation::BROTLI_OPERATION_FLUSH,
        )?;

        Ok(BrotliEncoderHasMoreOutput(&self.state) == 0)
    }

    fn finish(
        &mut self,
        output: &mut PartialBuffer<impl AsRef<[u8]> + AsMut<[u8]>>,
    ) -> Result<bool> {
        self.encode(
            &mut PartialBuffer::new(&[][..]),
            output,
            BrotliEncoderOperation::BROTLI_OPERATION_FINISH,
        )?;

        Ok(BrotliEncoderIsFinished(&self.state) == 1)
    }
}

impl fmt::Debug for BrotliEncoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrotliEncoder")
            .field("compress", &"<no debug>")
            .finish()
    }
}
