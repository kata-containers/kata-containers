macro_rules! encoder {
    ($(#[$attr:meta])* $name:ident<$inner:ident> $({ $($constructor:tt)* })*) => {
        pin_project_lite::pin_project! {
            $(#[$attr])*
            #[derive(Debug)]
            ///
            /// This structure implements a [`Stream`](futures_core::stream::Stream) interface and will read
            /// uncompressed data from an underlying stream and emit a stream of compressed data.
            pub struct $name<$inner> {
                #[pin]
                inner: crate::stream::Encoder<$inner, crate::codec::$name>,
            }
        }

        impl<$inner: futures_core::stream::Stream<Item = std::io::Result<bytes_05::Bytes>>> $name<$inner> {
            $(
                /// Creates a new encoder which will read uncompressed data from the given stream
                /// and emit a compressed stream.
                ///
                $($constructor)*
            )*

            /// Acquires a reference to the underlying stream that this encoder is wrapping.
            pub fn get_ref(&self) -> &$inner {
                self.inner.get_ref()
            }

            /// Acquires a mutable reference to the underlying stream that this encoder is
            /// wrapping.
            ///
            /// Note that care must be taken to avoid tampering with the state of the stream which
            /// may otherwise confuse this encoder.
            pub fn get_mut(&mut self) -> &mut $inner {
                self.inner.get_mut()
            }

            /// Acquires a pinned mutable reference to the underlying stream that this encoder is
            /// wrapping.
            ///
            /// Note that care must be taken to avoid tampering with the state of the stream which
            /// may otherwise confuse this encoder.
            pub fn get_pin_mut(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut $inner> {
                self.project().inner.get_pin_mut()
            }

            /// Consumes this encoder returning the underlying stream.
            ///
            /// Note that this may discard internal state of this encoder, so care should be taken
            /// to avoid losing resources when this is called.
            pub fn into_inner(self) -> $inner {
                self.inner.into_inner()
            }
        }

        impl<$inner: futures_core::stream::Stream<Item = std::io::Result<bytes_05::Bytes>>>
            futures_core::stream::Stream for $name<$inner>
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
