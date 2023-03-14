macro_rules! decoder {
    ($(#[$attr:meta])* $name:ident) => {
        pin_project_lite::pin_project! {
            $(#[$attr])*
            #[derive(Debug)]
            ///
            /// This structure implements a [`Stream`](futures_core::stream::Stream) interface and will read
            /// compressed data from an underlying stream and emit a stream of uncompressed data.
            pub struct $name<S> {
                #[pin]
                inner: crate::stream::generic::Decoder<S, crate::codec::$name>,
            }
        }

        impl<S: futures_core::stream::Stream<Item = std::io::Result<bytes_05::Bytes>>> $name<S> {
            /// Creates a new decoder which will read compressed data from the given stream and
            /// emit an uncompressed stream.
            pub fn new(stream: S) -> Self {
                Self {
                    inner: crate::stream::Decoder::new(
                        stream,
                        crate::codec::$name::new(),
                    ),
                }
            }

            /// Configure multi-member/frame decoding, if enabled this will reset the decoder state
            /// when reaching the end of a compressed member/frame and expect either the end of the
            /// wrapped stream or another compressed member/frame to follow.
            pub fn multiple_members(&mut self, enabled: bool) {
                self.inner.multiple_members(enabled);
            }

            /// Acquires a reference to the underlying stream that this decoder is wrapping.
            pub fn get_ref(&self) -> &S {
                self.inner.get_ref()
            }

            /// Acquires a mutable reference to the underlying stream that this decoder is
            /// wrapping.
            ///
            /// Note that care must be taken to avoid tampering with the state of the stream which
            /// may otherwise confuse this decoder.
            pub fn get_mut(&mut self) -> &mut S {
                self.inner.get_mut()
            }

            /// Acquires a pinned mutable reference to the underlying stream that this decoder is
            /// wrapping.
            ///
            /// Note that care must be taken to avoid tampering with the state of the stream which
            /// may otherwise confuse this decoder.
            pub fn get_pin_mut(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut S> {
                self.project().inner.get_pin_mut()
            }

            /// Consumes this decoder returning the underlying stream.
            ///
            /// Note that this may discard internal state of this decoder, so care should be taken
            /// to avoid losing resources when this is called.
            pub fn into_inner(self) -> S {
                self.inner.into_inner()
            }
        }

        impl<S: futures_core::stream::Stream<Item = std::io::Result<bytes_05::Bytes>>>
            futures_core::stream::Stream for $name<S>
        {
            type Item = std::io::Result<bytes_05::Bytes>;

            fn poll_next(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<std::io::Result<bytes_05::Bytes>>> {
                self.project().inner.poll_next(cx)
            }
        }

        const _: () = {
            fn _assert() {
                use std::{pin::Pin, io::Result};
                use bytes_05::Bytes;
                use futures_core::stream::Stream;
                use crate::util::{_assert_send, _assert_sync};

                _assert_send::<$name<Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>>>();
                _assert_sync::<$name<Pin<Box<dyn Stream<Item = Result<Bytes>> + Sync>>>>();
            }
        };
    }
}
