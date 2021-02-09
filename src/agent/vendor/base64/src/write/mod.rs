//! Implementations of `io::Write` to transparently handle base64.
mod encoder;
pub use self::encoder::EncoderWriter;

#[cfg(test)]
mod encoder_tests;
