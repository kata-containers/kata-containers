macro_rules! encoder {
    ($(#[$attr:meta])* $name:ident<$inner:ident> $({ $($constructor:tt)* })*) => {
        pin_project_lite::pin_project! {
            $(#[$attr])*
            #[derive(Debug)]
            ///
            /// This structure implements an [`AsyncRead`](tokio_02::io::AsyncRead) interface and will
            /// read uncompressed data from an underlying stream and emit a stream of compressed data.
            pub struct $name<$inner> {
                #[pin]
                inner: crate::tokio_02::bufread::Encoder<$inner, crate::codec::$name>,
            }
        }

        impl<$inner: tokio_02::io::AsyncBufRead> $name<$inner> {
            $(
                /// Creates a new encoder which will read uncompressed data from the given stream
                /// and emit a compressed stream.
                ///
                $($constructor)*
            )*

            /// Acquires a reference to the underlying reader that this encoder is wrapping.
            pub fn get_ref(&self) -> &$inner {
                self.inner.get_ref()
            }

            /// Acquires a mutable reference to the underlying reader that this encoder is
            /// wrapping.
            ///
            /// Note that care must be taken to avoid tampering with the state of the reader which
            /// may otherwise confuse this encoder.
            pub fn get_mut(&mut self) -> &mut $inner {
                self.inner.get_mut()
            }

            /// Acquires a pinned mutable reference to the underlying reader that this encoder is
            /// wrapping.
            ///
            /// Note that care must be taken to avoid tampering with the state of the reader which
            /// may otherwise confuse this encoder.
            pub fn get_pin_mut(self: std::pin::Pin<&mut Self>) -> std::pin::Pin<&mut $inner> {
                self.project().inner.get_pin_mut()
            }

            /// Consumes this encoder returning the underlying reader.
            ///
            /// Note that this may discard internal state of this encoder, so care should be taken
            /// to avoid losing resources when this is called.
            pub fn into_inner(self) -> $inner {
                self.inner.into_inner()
            }
        }

        impl<$inner: tokio_02::io::AsyncBufRead> tokio_02::io::AsyncRead for $name<$inner> {
            fn poll_read(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut [u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                self.project().inner.poll_read(cx, buf)
            }
        }

        const _: () = {
            fn _assert() {
                use crate::util::{_assert_send, _assert_sync};
                use core::pin::Pin;
                use tokio_02::io::AsyncBufRead;

                _assert_send::<$name<Pin<Box<dyn AsyncBufRead + Send>>>>();
                _assert_sync::<$name<Pin<Box<dyn AsyncBufRead + Sync>>>>();
            }
        };
    }
}
