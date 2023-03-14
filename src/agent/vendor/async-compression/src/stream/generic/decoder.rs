use std::{
    io::Result,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{codec::Decode, util::PartialBuffer};
use bytes_05::{Buf, Bytes, BytesMut};
use futures_core::{ready, stream::Stream};
use pin_project_lite::pin_project;

const OUTPUT_BUFFER_SIZE: usize = 8_000;

#[derive(Debug)]
enum State {
    Reading,
    Writing,
    Flushing,
    Next,
    Done,
}

pin_project! {
    #[derive(Debug)]
    pub struct Decoder<S, D: Decode> {
        #[pin]
        stream: S,
        decoder: D,
        state: State,
        input: Bytes,
        output: BytesMut,
        multiple_members: bool,
    }
}

impl<S: Stream<Item = Result<Bytes>>, D: Decode> Decoder<S, D> {
    pub fn new(stream: S, decoder: D) -> Self {
        Self {
            stream,
            decoder,
            state: State::Reading,
            input: Bytes::new(),
            output: BytesMut::new(),
            multiple_members: false,
        }
    }

    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        self.project().stream
    }

    pub fn into_inner(self) -> S {
        self.stream
    }

    pub fn multiple_members(&mut self, enabled: bool) {
        self.multiple_members = enabled;
    }
}

impl<S: Stream<Item = Result<Bytes>>, D: Decode> Stream for Decoder<S, D> {
    type Item = Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Bytes>>> {
        let this = self.project();

        let (mut stream, input, state, decoder, multiple_members) = (
            this.stream,
            this.input,
            this.state,
            this.decoder,
            *this.multiple_members,
        );

        let mut output = PartialBuffer::new(this.output);

        let result = (|| loop {
            let output_capacity = output.written().len() + OUTPUT_BUFFER_SIZE;
            output.get_mut().resize(output_capacity, 0);

            *state = match state {
                State::Reading => {
                    if let Some(chunk) = ready!(stream.as_mut().poll_next(cx)) {
                        *input = chunk?;
                        State::Writing
                    } else {
                        State::Flushing
                    }
                }

                State::Writing => {
                    if input.is_empty() {
                        State::Reading
                    } else {
                        let mut input = PartialBuffer::new(&mut *input);

                        let done = decoder.decode(&mut input, &mut output)?;

                        let input_len = input.written().len();
                        input.into_inner().advance(input_len);

                        if done {
                            State::Flushing
                        } else {
                            State::Writing
                        }
                    }
                }

                State::Flushing => {
                    if decoder.finish(&mut output)? {
                        if multiple_members {
                            State::Next
                        } else {
                            State::Done
                        }
                    } else {
                        State::Flushing
                    }
                }

                State::Next => {
                    if input.is_empty() {
                        if let Some(chunk) = ready!(stream.as_mut().poll_next(cx)) {
                            *input = chunk?;
                            State::Next
                        } else {
                            State::Done
                        }
                    } else {
                        decoder.reinit()?;
                        State::Writing
                    }
                }

                State::Done => {
                    return Poll::Ready(None);
                }
            };
        })();

        match result {
            Poll::Ready(Some(Ok(_))) => unreachable!(),
            Poll::Ready(Some(Err(_))) => {
                *state = State::Done;
                result
            }
            Poll::Ready(None) | Poll::Pending => {
                if output.written().is_empty() {
                    result
                } else {
                    let output_len = output.written().len();
                    Poll::Ready(Some(Ok(output.into_inner().split_to(output_len).freeze())))
                }
            }
        }
    }
}
