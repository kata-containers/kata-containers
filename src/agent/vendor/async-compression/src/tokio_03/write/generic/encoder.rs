use core::{
    pin::Pin,
    task::{Context, Poll},
};
use std::io::Result;

use crate::{
    codec::Encode,
    tokio_03::write::{AsyncBufWrite, BufWriter},
    util::PartialBuffer,
};
use futures_core::ready;
use pin_project_lite::pin_project;
use tokio_03::io::AsyncWrite;

#[derive(Debug)]
enum State {
    Encoding,
    Finishing,
    Done,
}

pin_project! {
    #[derive(Debug)]
    pub struct Encoder<W, E: Encode> {
        #[pin]
        writer: BufWriter<W>,
        encoder: E,
        state: State,
    }
}

impl<W: AsyncWrite, E: Encode> Encoder<W, E> {
    pub fn new(writer: W, encoder: E) -> Self {
        Self {
            writer: BufWriter::new(writer),
            encoder,
            state: State::Encoding,
        }
    }

    pub fn get_ref(&self) -> &W {
        self.writer.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut W {
        self.writer.get_mut()
    }

    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut W> {
        self.project().writer.get_pin_mut()
    }

    pub fn into_inner(self) -> W {
        self.writer.into_inner()
    }

    fn do_poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        input: &mut PartialBuffer<&[u8]>,
    ) -> Poll<Result<()>> {
        let mut this = self.project();

        loop {
            let output = ready!(this.writer.as_mut().poll_partial_flush_buf(cx))?;
            let mut output = PartialBuffer::new(output);

            *this.state = match this.state {
                State::Encoding => {
                    this.encoder.encode(input, &mut output)?;
                    State::Encoding
                }

                State::Finishing | State::Done => panic!("Write after shutdown"),
            };

            let produced = output.written().len();
            this.writer.as_mut().produce(produced);

            if input.unwritten().is_empty() {
                return Poll::Ready(Ok(()));
            }
        }
    }

    fn do_poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut this = self.project();

        loop {
            let output = ready!(this.writer.as_mut().poll_partial_flush_buf(cx))?;
            let mut output = PartialBuffer::new(output);

            let done = match this.state {
                State::Encoding => this.encoder.flush(&mut output)?,

                State::Finishing | State::Done => panic!("Flush after shutdown"),
            };

            let produced = output.written().len();
            this.writer.as_mut().produce(produced);

            if done {
                return Poll::Ready(Ok(()));
            }
        }
    }

    fn do_poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let mut this = self.project();

        loop {
            let output = ready!(this.writer.as_mut().poll_partial_flush_buf(cx))?;
            let mut output = PartialBuffer::new(output);

            *this.state = match this.state {
                State::Encoding | State::Finishing => {
                    if this.encoder.finish(&mut output)? {
                        State::Done
                    } else {
                        State::Finishing
                    }
                }

                State::Done => State::Done,
            };

            let produced = output.written().len();
            this.writer.as_mut().produce(produced);

            if let State::Done = this.state {
                return Poll::Ready(Ok(()));
            }
        }
    }
}

impl<W: AsyncWrite, E: Encode> AsyncWrite for Encoder<W, E> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        let mut input = PartialBuffer::new(buf);

        match self.do_poll_write(cx, &mut input)? {
            Poll::Pending if input.written().is_empty() => Poll::Pending,
            _ => Poll::Ready(Ok(input.written().len())),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        ready!(self.as_mut().do_poll_flush(cx))?;
        ready!(self.project().writer.as_mut().poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        ready!(self.as_mut().do_poll_shutdown(cx))?;
        ready!(self.project().writer.as_mut().poll_shutdown(cx))?;
        Poll::Ready(Ok(()))
    }
}
