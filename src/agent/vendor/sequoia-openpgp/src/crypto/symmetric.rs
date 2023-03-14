//! Symmetric encryption.

use std::io;
use std::cmp;
use std::fmt;

use crate::Result;
use crate::SymmetricAlgorithm;
use crate::vec_resize;
use crate::{
    parse::Cookie,
};

use buffered_reader::BufferedReader;

/// Block cipher mode of operation.
///
/// Block modes govern how a block cipher processes data spanning multiple blocks.
pub(crate) trait Mode: Send + Sync {
    /// Block size of the underlying cipher in bytes.
    fn block_size(&self) -> usize;

    /// Encrypt a single block `src` to a ciphertext block `dst`.
    /// The `dst` and `src` buffers are expected to be at least as large as
    /// the block size of the underlying cipher.
    fn encrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()>;

    /// Decrypt a single ciphertext block `src` to a plaintext block `dst`.
    /// The `dst` and `src` buffers are expected to be at least as large as
    /// the block size of the underlying cipher.
    fn decrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()>;
}

/// A `Read`er for decrypting symmetrically encrypted data.
pub struct Decryptor<'a> {
    // The encrypted data.
    source: Box<dyn BufferedReader<Cookie> + 'a>,

    dec: Box<dyn Mode>,
    block_size: usize,
    // Up to a block of unread data.
    buffer: Vec<u8>,
}
assert_send_and_sync!(Decryptor<'_>);

impl<'a> Decryptor<'a> {
    /// Instantiate a new symmetric decryptor.
    ///
    /// `reader` is the source to wrap.
    pub fn new<R>(algo: SymmetricAlgorithm, key: &[u8], source: R)
                  -> Result<Self>
    where
        R: io::Read + Send + Sync + 'a,
    {
        Self::from_buffered_reader(
            algo, key,
            Box::new(buffered_reader::Generic::with_cookie(
                source, None, Default::default())))
    }

    /// Instantiate a new symmetric decryptor.
    fn from_buffered_reader(algo: SymmetricAlgorithm, key: &[u8],
                            source: Box<dyn BufferedReader<Cookie> + 'a>)
                            -> Result<Self>
    {
        let block_size = algo.block_size()?;
        let iv = vec![0; block_size];
        let dec = algo.make_decrypt_cfb(key, iv)?;

        Ok(Decryptor {
            source,
            dec,
            block_size,
            buffer: Vec::with_capacity(block_size),
        })
    }
}

// Note: this implementation tries *very* hard to make sure we don't
// gratuitiously do a short read.  Specifically, if the return value
// is less than `plaintext.len()`, then it is either because we
// reached the end of the input or an error occurred.
impl<'a> io::Read for Decryptor<'a> {
    fn read(&mut self, plaintext: &mut [u8]) -> io::Result<usize> {
        let mut pos = 0;

        // 1. Copy any buffered data.
        if !self.buffer.is_empty() {
            let to_copy = cmp::min(self.buffer.len(), plaintext.len());
            plaintext[..to_copy].copy_from_slice(&self.buffer[..to_copy]);
            crate::vec_drain_prefix(&mut self.buffer, to_copy);
            pos = to_copy;
        }

        if pos == plaintext.len() {
            return Ok(pos);
        }

        // 2. Decrypt as many whole blocks as `plaintext` can hold.
        let mut to_copy
            = ((plaintext.len() - pos) / self.block_size) *  self.block_size;
        let result = self.source.data_consume(to_copy);
        let short_read;
        let ciphertext = match result {
            Ok(data) => {
                short_read = data.len() < to_copy;
                to_copy = data.len().min(to_copy);
                &data[..to_copy]
            },
            // We encountered an error, but we did read some.
            Err(_) if pos > 0 => return Ok(pos),
            Err(e) => return Err(e),
        };

        self.dec.decrypt(&mut plaintext[pos..pos + to_copy],
                         ciphertext)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput,
                                        format!("{}", e)))?;

        pos += to_copy;

        if short_read || pos == plaintext.len() {
            return Ok(pos);
        }

        // 3. The last bit is a partial block.  Buffer it.
        let mut to_copy = plaintext.len() - pos;
        assert!(0 < to_copy);
        assert!(to_copy < self.block_size);

        let to_read = self.block_size;
        let result = self.source.data_consume(to_read);
        let ciphertext = match result {
            Ok(data) => {
                // Make sure we don't read more than is available.
                to_copy = cmp::min(to_copy, data.len());
                &data[..data.len().min(to_read)]
            },
            // We encountered an error, but we did read some.
            Err(_) if pos > 0 => return Ok(pos),
            Err(e) => return Err(e),
        };
        assert!(ciphertext.len() <= self.block_size);

        vec_resize(&mut self.buffer, ciphertext.len());

        self.dec.decrypt(&mut self.buffer, ciphertext)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput,
                                        format!("{}", e)))?;

        plaintext[pos..pos + to_copy].copy_from_slice(&self.buffer[..to_copy]);
        crate::vec_drain_prefix(&mut self.buffer, to_copy);

        pos += to_copy;

        Ok(pos)
    }
}

/// A `BufferedReader` that decrypts symmetrically-encrypted data as
/// it is read.
pub(crate) struct BufferedReaderDecryptor<'a> {
    reader: buffered_reader::Generic<Decryptor<'a>, Cookie>,
}

impl<'a> BufferedReaderDecryptor<'a> {
    /// Like `new()`, but sets a cookie, which can be retrieved using
    /// the `cookie_ref` and `cookie_mut` methods, and set using
    /// the `cookie_set` method.
    pub fn with_cookie(algo: SymmetricAlgorithm, key: &[u8],
                       reader: Box<dyn BufferedReader<Cookie> + 'a>,
                       cookie: Cookie)
        -> Result<Self>
    {
        Ok(BufferedReaderDecryptor {
            reader: buffered_reader::Generic::with_cookie(
                Decryptor::from_buffered_reader(algo, key, reader)?,
                None, cookie),
        })
    }
}

impl<'a> io::Read for BufferedReaderDecryptor<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

impl<'a> fmt::Display for BufferedReaderDecryptor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BufferedReaderDecryptor")
    }
}

impl<'a> fmt::Debug for BufferedReaderDecryptor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BufferedReaderDecryptor")
            .field("reader", &self.get_ref().unwrap())
            .finish()
    }
}

impl<'a> BufferedReader<Cookie> for BufferedReaderDecryptor<'a> {
    fn buffer(&self) -> &[u8] {
        self.reader.buffer()
    }

    fn data(&mut self, amount: usize) -> io::Result<&[u8]> {
        self.reader.data(amount)
    }

    fn data_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        self.reader.data_hard(amount)
    }

    fn data_eof(&mut self) -> io::Result<&[u8]> {
        self.reader.data_eof()
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        self.reader.consume(amount)
    }

    fn data_consume(&mut self, amount: usize)
                    -> io::Result<&[u8]> {
        self.reader.data_consume(amount)
    }

    fn data_consume_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        self.reader.data_consume_hard(amount)
    }

    fn read_be_u16(&mut self) -> io::Result<u16> {
        self.reader.read_be_u16()
    }

    fn read_be_u32(&mut self) -> io::Result<u32> {
        self.reader.read_be_u32()
    }

    fn steal(&mut self, amount: usize) -> io::Result<Vec<u8>> {
        self.reader.steal(amount)
    }

    fn steal_eof(&mut self) -> io::Result<Vec<u8>> {
        self.reader.steal_eof()
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<Cookie>> {
        Some(&mut self.reader.reader_mut().source)
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<Cookie>> {
        Some(&self.reader.reader_ref().source)
    }

    fn into_inner<'b>(self: Box<Self>)
            -> Option<Box<dyn BufferedReader<Cookie> + 'b>> where Self: 'b {
        Some(self.reader.into_reader().source.as_boxed())
    }

    fn cookie_set(&mut self, cookie: Cookie) -> Cookie {
        self.reader.cookie_set(cookie)
    }

    fn cookie_ref(&self) -> &Cookie {
        self.reader.cookie_ref()
    }

    fn cookie_mut(&mut self) -> &mut Cookie {
        self.reader.cookie_mut()
    }
}

/// A `Write`r for symmetrically encrypting data.
pub struct Encryptor<W: io::Write> {
    inner: Option<W>,

    cipher: Box<dyn Mode>,
    block_size: usize,
    // Up to a block of unencrypted data.
    buffer: Vec<u8>,
    // A place to write encrypted data into.
    scratch: Vec<u8>,
}
assert_send_and_sync!(Encryptor<W> where W: io::Write);

impl<W: io::Write> Encryptor<W> {
    /// Instantiate a new symmetric encryptor.
    pub fn new(algo: SymmetricAlgorithm, key: &[u8], sink: W) -> Result<Self> {
        let block_size = algo.block_size()?;
        let iv = vec![0; block_size];
        let cipher = algo.make_encrypt_cfb(key, iv)?;

        Ok(Encryptor {
            inner: Some(sink),
            cipher,
            block_size,
            buffer: Vec::with_capacity(block_size),
            scratch: vec![0; 4096],
        })
    }

    /// Finish encryption and write last partial block.
    pub fn finish(&mut self) -> Result<W> {
        if let Some(mut inner) = self.inner.take() {
            if !self.buffer.is_empty() {
                let n = self.buffer.len();
                assert!(n <= self.block_size);
                self.cipher.encrypt(&mut self.scratch[..n], &self.buffer)?;
                crate::vec_truncate(&mut self.buffer, 0);
                inner.write_all(&self.scratch[..n])?;
                crate::vec_truncate(&mut self.scratch, 0);
            }
            Ok(inner)
        } else {
            Err(io::Error::new(io::ErrorKind::BrokenPipe,
                               "Inner writer was taken").into())
        }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> Option<&W> {
        self.inner.as_ref()
    }

    /// Acquires a mutable reference to the underlying writer.
    #[allow(dead_code)]
    pub fn get_mut(&mut self) -> Option<&mut W> {
        self.inner.as_mut()
    }
}

impl<W: io::Write> io::Write for Encryptor<W> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        if self.inner.is_none() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe,
                                      "Inner writer was taken"));
        }
        let inner = self.inner.as_mut().unwrap();
        let amount = buf.len();

        // First, fill the buffer if there is something in it.
        if !self.buffer.is_empty() {
            let n = cmp::min(buf.len(), self.block_size - self.buffer.len());
            self.buffer.extend_from_slice(&buf[..n]);
            assert!(self.buffer.len() <= self.block_size);
            buf = &buf[n..];

            // And possibly encrypt the block.
            if self.buffer.len() == self.block_size {
                self.cipher.encrypt(&mut self.scratch[..self.block_size],
                                    &self.buffer)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput,
                                                format!("{}", e)))?;
                crate::vec_truncate(&mut self.buffer, 0);
                inner.write_all(&self.scratch[..self.block_size])?;
            }
        }

        // Then, encrypt all whole blocks.
        let whole_blocks = (buf.len() / self.block_size) * self.block_size;
        if whole_blocks > 0 {
            // Encrypt whole blocks.
            if self.scratch.len() < whole_blocks {
                vec_resize(&mut self.scratch, whole_blocks);
            }

            self.cipher.encrypt(&mut self.scratch[..whole_blocks],
                                &buf[..whole_blocks])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput,
                                            format!("{}", e)))?;
            inner.write_all(&self.scratch[..whole_blocks])?;
        }

        // Stash rest for later.
        assert!(buf.is_empty() || self.buffer.is_empty());
        self.buffer.extend_from_slice(&buf[whole_blocks..]);

        Ok(amount)
    }

    fn flush(&mut self) -> io::Result<()> {
        // It is not clear how we can implement this, because we can
        // only operate on block sizes.  We will, however, ask our
        // inner writer to flush.
        if let Some(ref mut inner) = self.inner {
            inner.flush()
        } else {
            Err(io::Error::new(io::ErrorKind::BrokenPipe,
                               "Inner writer was taken"))
        }
    }
}

impl<W: io::Write> Drop for Encryptor<W> {
    fn drop(&mut self) {
        // Unfortunately, we cannot handle errors here.  If error
        // handling is a concern, call finish() and properly handle
        // errors there.
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read, Write};

    #[test]
    fn smoke_test() {
        use crate::fmt::hex;

        let algo = SymmetricAlgorithm::AES128;
        let key = &hex::decode("2b7e151628aed2a6abf7158809cf4f3c").unwrap();
        assert_eq!(key.len(), 16);

        // Ensure we use CFB128 by default
        let iv = hex::decode("000102030405060708090A0B0C0D0E0F").unwrap();
        let mut cfb = algo.make_encrypt_cfb(key, iv).unwrap();
        let msg = hex::decode("6bc1bee22e409f96e93d7e117393172a").unwrap();
        let mut dst = vec![0; msg.len()];
        cfb.encrypt(&mut dst, &*msg).unwrap();
        assert_eq!(&dst[..16], &*hex::decode("3b3fd92eb72dad20333449f8e83cfb4a").unwrap());

        // 32-byte long message
        let iv = hex::decode("000102030405060708090A0B0C0D0E0F").unwrap();
        let mut cfb = algo.make_encrypt_cfb(key, iv).unwrap();
        let msg = b"This is a very important message";
        let mut dst = vec![0; msg.len()];
        cfb.encrypt(&mut dst, &*msg).unwrap();
        assert_eq!(&dst, &hex::decode(
            "04960ebfb9044196bb29418ce9d6cc0939d5ccb1d0712fa8e45fe5673456fded"
        ).unwrap());

        // 33-byte (uneven) long message
        let iv = hex::decode("000102030405060708090A0B0C0D0E0F").unwrap();
        let mut cfb = algo.make_encrypt_cfb(key, iv).unwrap();
        let msg = b"This is a very important message!";
        let mut dst = vec![0; msg.len()];
        cfb.encrypt(&mut dst, &*msg).unwrap();
        assert_eq!(&dst, &hex::decode(
            "04960ebfb9044196bb29418ce9d6cc0939d5ccb1d0712fa8e45fe5673456fded0b"
        ).unwrap());

        // 33-byte (uneven) long message, chunked
        let iv = hex::decode("000102030405060708090A0B0C0D0E0F").unwrap();
        let mut cfb = algo.make_encrypt_cfb(key, iv).unwrap();
        let mut dst = vec![0; msg.len()];
        for (mut dst, msg) in dst.chunks_mut(16).zip(msg.chunks(16)) {
            cfb.encrypt(&mut dst, msg).unwrap();
        }
        assert_eq!(&dst, &hex::decode(
            "04960ebfb9044196bb29418ce9d6cc0939d5ccb1d0712fa8e45fe5673456fded0b"
        ).unwrap());
    }

    /// This test is designed to test the buffering logic in Decryptor
    /// by reading directly from it (i.e. without any buffering
    /// introduced by the BufferedReaderDecryptor or any other source
    /// of buffering).
    #[test]
    fn decryptor() {
        for algo in [SymmetricAlgorithm::AES128,
                     SymmetricAlgorithm::AES192,
                     SymmetricAlgorithm::AES256].iter() {
            // The keys are [0, 1, 2, ...].
            let mut key = vec![0u8; algo.key_size().unwrap()];
            for i in 0..key.len() {
                key[0] = i as u8;
            }

            let filename = &format!(
                    "raw/a-cypherpunks-manifesto.aes{}.key_ascending_from_0",
                algo.key_size().unwrap() * 8);
            let ciphertext = Cursor::new(crate::tests::file(filename));
            let decryptor = Decryptor::new(*algo, &key, ciphertext).unwrap();

            // Read bytewise to test the buffer logic.
            let mut plaintext = Vec::new();
            for b in decryptor.bytes() {
                plaintext.push(b.unwrap());
            }

            assert_eq!(crate::tests::manifesto(), &plaintext[..]);
        }
    }

    /// This test is designed to test the buffering logic in Encryptor
    /// by writing directly to it.
    #[test]
    fn encryptor() {
        for algo in [SymmetricAlgorithm::AES128,
                     SymmetricAlgorithm::AES192,
                     SymmetricAlgorithm::AES256].iter() {
            // The keys are [0, 1, 2, ...].
            let mut key = vec![0u8; algo.key_size().unwrap()];
            for i in 0..key.len() {
                key[0] = i as u8;
            }

            let mut ciphertext = Vec::new();
            {
                let mut encryptor = Encryptor::new(*algo, &key, &mut ciphertext)
                    .unwrap();

                // Write bytewise to test the buffer logic.
                for b in crate::tests::manifesto().chunks(1) {
                    encryptor.write_all(b).unwrap();
                }
            }

            let filename = format!(
                "raw/a-cypherpunks-manifesto.aes{}.key_ascending_from_0",
                algo.key_size().unwrap() * 8);
            let mut cipherfile = Cursor::new(crate::tests::file(&filename));
            let mut reference = Vec::new();
            cipherfile.read_to_end(&mut reference).unwrap();
            assert_eq!(&reference[..], &ciphertext[..]);
        }
    }

    /// This test tries to encrypt, then decrypt some data.
    #[test]
    fn roundtrip() {
        use std::io::Cursor;

        for algo in [SymmetricAlgorithm::TripleDES,
                     SymmetricAlgorithm::CAST5,
                     SymmetricAlgorithm::Blowfish,
                     SymmetricAlgorithm::AES128,
                     SymmetricAlgorithm::AES192,
                     SymmetricAlgorithm::AES256,
                     SymmetricAlgorithm::Twofish,
                     SymmetricAlgorithm::Camellia128,
                     SymmetricAlgorithm::Camellia192,
                     SymmetricAlgorithm::Camellia256]
                     .iter()
                     .filter(|x| x.is_supported()) {
            let mut key = vec![0; algo.key_size().unwrap()];
            crate::crypto::random(&mut key);

            let mut ciphertext = Vec::new();
            {
                let mut encryptor = Encryptor::new(*algo, &key, &mut ciphertext)
                    .unwrap();

                encryptor.write_all(crate::tests::manifesto()).unwrap();
            }

            let mut plaintext = Vec::new();
            {
                let mut decryptor = Decryptor::new(*algo, &key,
                                                   Cursor::new(&mut ciphertext))
                    .unwrap();

                decryptor.read_to_end(&mut plaintext).unwrap();
            }

            assert_eq!(&plaintext[..], crate::tests::manifesto());
        }
    }
}
