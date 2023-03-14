use crate::Asn1DerError;
use std::io::{self, Read, Write};
use std::mem::size_of;

/// The byte size of an `usize`
const USIZE_LEN: usize = size_of::<usize>();

/// An extension for `io::Read`
pub trait ReadExt {
    /// Reads the next byte
    fn read_one(&mut self) -> io::Result<u8>;
}
impl<T: Read> ReadExt for T {
    fn read_one(&mut self) -> io::Result<u8> {
        let mut buf = [0];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}
/// An extension for `io::Write`
pub trait WriteExt {
    /// Writes on `byte`
    fn write_one(&mut self, byte: u8) -> io::Result<usize>;
    /// Writes all bytes in `data`
    fn write_exact(&mut self, data: &[u8]) -> io::Result<usize>;
}
impl<T: Write> WriteExt for T {
    fn write_one(&mut self, byte: u8) -> io::Result<usize> {
        self.write_exact(&[byte])
    }
    fn write_exact(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_all(data)?;
        Ok(data.len())
    }
}

const PEEKED_BUFFER_SIZE: usize = 10;

#[derive(Debug)]
pub struct PeekedContent {
    len: usize,
    buffer: [u8; PEEKED_BUFFER_SIZE],
}

impl PeekedContent {
    fn new() -> Self {
        Self {
            len: 0,
            buffer: [0; PEEKED_BUFFER_SIZE],
        }
    }

    pub fn take(&mut self) -> Self {
        let mut val = Self::new();
        std::mem::swap(&mut val, self);
        val
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn buffer(&self) -> [u8; PEEKED_BUFFER_SIZE] {
        self.buffer
    }
}

/// A peekable reader
pub struct PeekableReader<R: Read> {
    reader: R,
    peeked: PeekedContent,
    pos: usize,
}
impl<R: Read> PeekableReader<R> {
    /// Creates a new `PeekableReader` with `reader` as source
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            peeked: PeekedContent::new(),
            pos: 0,
        }
    }

    /// Peeks one byte without removing it from the `read`-queue
    ///
    /// Multiple successive calls to `peek_one` will always return the same next byte
    pub fn peek_one(&mut self) -> io::Result<u8> {
        // Check if we already have peeked data
        if self.peeked.len == 0 {
            self.peeked.buffer[0] = self.reader.read_one()?;
            self.peeked.len = 1;
        }
        Ok(self.peeked.buffer[0])
    }

    /// Peeks several bytes at once without removing them from the `read`-queue
    /// Buffer size is defined by `PeekedBuffer`.
    ///
    /// Successive calls to `peek_buffer` always return the same bytes.
    pub fn peek_buffer(&mut self) -> io::Result<&PeekedContent> {
        // Check if we already have peeked data
        if self.peeked.len < PEEKED_BUFFER_SIZE {
            let n = self.reader.read(&mut self.peeked.buffer[self.peeked.len..])?;
            self.peeked.len += n;
        }

        Ok(&self.peeked)
    }

    /// The current position (amount of bytes read)
    pub fn pos(&self) -> usize {
        self.pos
    }
}
impl<R: Read> Read for PeekableReader<R> {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let mut read = 0;

        let peeked = self.peeked.take();
        let new_start_index = if buf.len() <= peeked.len {
            buf.copy_from_slice(&peeked.buffer[..buf.len()]);

            // keep remaining peeked bytes
            let remaining_bytes = peeked.len - buf.len();
            if remaining_bytes > 0 {
                self.peeked.buffer[..remaining_bytes].copy_from_slice(&peeked.buffer[buf.len()..peeked.len]);
                self.peeked.len = remaining_bytes;
            }

            buf.len()
        } else {
            buf[..peeked.len].copy_from_slice(&peeked.buffer[..peeked.len]);
            peeked.len
        };
        read += new_start_index;
        buf = &mut buf[new_start_index..];

        // Read remaining bytes
        read += self.reader.read(buf)?;

        self.pos += read;

        Ok(read)
    }
}

/// An implementation of the ASN.1-DER length
pub struct Length;
impl Length {
    /// Deserializes a length from `reader`
    pub fn deserialized(mut reader: impl Read) -> Result<usize, Asn1DerError> {
        // Deserialize length
        Ok(match reader.read_one()? {
            n @ 128..=255 => {
                // Deserialize the amount of length bytes
                let len = n as usize & 127;
                if len > USIZE_LEN {
                    return Err(Asn1DerError::UnsupportedValue);
                }

                // Deserialize value
                let mut num = [0; USIZE_LEN];
                reader.read_exact(&mut num[USIZE_LEN - len..])?;
                usize::from_be_bytes(num)
            }
            n => n as usize,
        })
    }

    /// Serializes `len` to `writer`
    pub fn serialize(len: usize, mut writer: impl Write) -> Result<usize, Asn1DerError> {
        // Determine the serialized length
        let written = match len {
            0..=127 => writer.write_one(len as u8)?,
            _ => {
                let to_write = USIZE_LEN - (len.leading_zeros() / 8) as usize;
                // Write number of bytes used to encode length
                let mut written = writer.write_one(to_write as u8 | 0x80)?;

                // Write length
                let mut buf = [0; USIZE_LEN];
                buf.copy_from_slice(&len.to_be_bytes());
                written += writer.write_exact(&buf[USIZE_LEN - to_write..])?;

                written
            }
        };

        Ok(written)
    }

    /// Returns how many bytes are going to be needed to encode `len`.
    pub fn encoded_len(len: usize) -> usize {
        match len {
            0..=127 => 1,
            _ => 1 + USIZE_LEN - (len.leading_zeros() / 8) as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asn1_short_form_length() {
        let mut writer: Vec<u8> = Vec::new();
        let written = Length::serialize(10, &mut writer).expect("serialization failed");
        assert_eq!(written, 1);
        assert_eq!(writer.len(), 1);
        assert_eq!(writer[0], 10);
    }

    #[test]
    fn asn1_long_form_length_1_byte() {
        let mut writer: Vec<u8> = Vec::new();
        let written = Length::serialize(129, &mut writer).expect("serialization failed");
        assert_eq!(written, 2);
        assert_eq!(writer.len(), 2);
        assert_eq!(writer[0], 0x81);
        assert_eq!(writer[1], 0x81);
    }

    #[test]
    fn asn1_long_form_length_2_bytes() {
        let mut writer: Vec<u8> = Vec::new();
        let written = Length::serialize(290, &mut writer).expect("serialization failed");
        assert_eq!(written, 3);
        assert_eq!(writer.len(), 3);
        assert_eq!(writer[0], 0x82);
        assert_eq!(writer[1], 0x01);
        assert_eq!(writer[2], 0x22);
    }
}
