//! The interface and implementations of LZ77 compression algorithm.
//!
//! LZ77 is a compression algorithm used in [DEFLATE](https://tools.ietf.org/html/rfc1951).
pub use self::default::{DefaultLz77Encoder, DefaultLz77EncoderBuilder};

mod default;

/// Maximum length of sharable bytes in a pointer.
pub const MAX_LENGTH: u16 = 258;

/// Maximum backward distance of a pointer.
pub const MAX_DISTANCE: u16 = 32_768;

/// Maximum size of a sliding window.
pub const MAX_WINDOW_SIZE: u16 = MAX_DISTANCE;

/// A LZ77 encoded data.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Code {
    /// Literal byte.
    Literal(u8),

    /// Backward pointer to shared data.
    Pointer {
        /// Length of the shared data.
        /// The values must be limited to `MAX_LENGTH`.
        length: u16,

        /// Distance between current position and start position of the shared data.
        /// The values must be limited to `MAX_DISTANCE`.
        backward_distance: u16,
    },
}

/// Compression level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompressionLevel {
    /// No compression.
    None,

    /// Best speed.
    Fast,

    /// Balanced between speed and size.
    Balance,

    /// Best compression.
    Best,
}

/// The `Sink` trait represents a consumer of LZ77 encoded data.
pub trait Sink {
    /// Consumes a LZ77 encoded `Code`.
    fn consume(&mut self, code: Code);
}
impl<'a, T> Sink for &'a mut T
where
    T: Sink,
{
    fn consume(&mut self, code: Code) {
        (*self).consume(code);
    }
}
impl<T> Sink for Vec<T>
where
    T: From<Code>,
{
    fn consume(&mut self, code: Code) {
        self.push(T::from(code));
    }
}

/// The `LZ77Encode` trait defines the interface of LZ77 encoding algorithm.
pub trait Lz77Encode {
    /// Encodes a buffer and writes result LZ77 codes to `sink`.
    fn encode<S>(&mut self, buf: &[u8], sink: S)
    where
        S: Sink;

    /// Flushes the encoder, ensuring that all intermediately buffered codes are consumed by `sink`.
    fn flush<S>(&mut self, sink: S)
    where
        S: Sink;

    /// Returns the compression level of the encoder.
    ///
    /// If the implementation is omitted, `CompressionLevel::Balance` will be returned.
    fn compression_level(&self) -> CompressionLevel {
        CompressionLevel::Balance
    }

    /// Returns the window size of the encoder.
    ///
    /// If the implementation is omitted, `MAX_WINDOW_SIZE` will be returned.
    fn window_size(&self) -> u16 {
        MAX_WINDOW_SIZE
    }
}

/// A no compression implementation of `LZ77Encode` trait.
#[derive(Debug, Default)]
pub struct NoCompressionLz77Encoder;
impl NoCompressionLz77Encoder {
    /// Makes a new encoder instance.
    ///
    /// # Examples
    /// ```
    /// use libflate::deflate;
    /// use libflate::lz77::{Lz77Encode, NoCompressionLz77Encoder, CompressionLevel};
    ///
    /// let lz77 = NoCompressionLz77Encoder::new();
    /// assert_eq!(lz77.compression_level(), CompressionLevel::None);
    ///
    /// let options = deflate::EncodeOptions::with_lz77(lz77);
    /// let _deflate = deflate::Encoder::with_options(Vec::new(), options);
    /// ```
    pub fn new() -> Self {
        NoCompressionLz77Encoder
    }
}
impl Lz77Encode for NoCompressionLz77Encoder {
    fn encode<S>(&mut self, buf: &[u8], mut sink: S)
    where
        S: Sink,
    {
        for c in buf.iter().cloned().map(Code::Literal) {
            sink.consume(c);
        }
    }
    #[allow(unused_variables)]
    fn flush<S>(&mut self, sink: S)
    where
        S: Sink,
    {
    }
    fn compression_level(&self) -> CompressionLevel {
        CompressionLevel::None
    }
}
