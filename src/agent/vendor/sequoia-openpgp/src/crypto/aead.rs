use std::cmp;
use std::convert::TryInto;
use std::fmt;
use std::io;

use buffered_reader::BufferedReader;

use crate::types::{
    AEADAlgorithm,
    SymmetricAlgorithm,
};
use crate::utils::{
    write_be_u64,
};
use crate::Error;
use crate::Result;
use crate::crypto::SessionKey;
use crate::crypto::mem::secure_cmp;
use crate::seal;
use crate::parse::Cookie;

/// Minimum AEAD chunk size.
///
/// Implementations MUST support chunk sizes down to 64B.
const MIN_CHUNK_SIZE: usize = 1 << 6; // 64B

/// Maximum AEAD chunk size.
///
/// Implementations MUST support chunk sizes up to 4MiB.
const MAX_CHUNK_SIZE: usize = 1 << 22; // 4MiB

/// Maximum size of any Nonce used by an AEAD mode.
pub const MAX_NONCE_LEN: usize = 16;

/// Disables authentication checks.
///
/// This is DANGEROUS, and is only useful for debugging problems with
/// malformed AEAD-encrypted messages.
const DANGER_DISABLE_AUTHENTICATION: bool = false;

/// Converts a chunk size to a usize.
pub(crate) fn chunk_size_usize(chunk_size: u64) -> Result<usize> {
    chunk_size.try_into()
        .map_err(|_| Error::InvalidOperation(
            format!("AEAD chunk size exceeds size of \
                     virtual memory: {}", chunk_size)).into())
}

/// An AEAD mode of operation.
///
/// # Sealed trait
///
/// This trait is [sealed] and cannot be implemented for types outside this crate.
/// Therefore it can be extended in a non-breaking way.
/// If you want to implement the trait inside the crate
/// you also need to implement the `seal::Sealed` marker trait.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait Aead : seal::Sealed {
    /// Adds associated data `ad`.
    fn update(&mut self, ad: &[u8]);

    /// Encrypts one block `src` to `dst`.
    fn encrypt(&mut self, dst: &mut [u8], src: &[u8]);
    /// Decrypts one block `src` to `dst`.
    fn decrypt(&mut self, dst: &mut [u8], src: &[u8]);

    /// Produce the digest.
    fn digest(&mut self, digest: &mut [u8]);

    /// Length of the digest in bytes.
    fn digest_size(&self) -> usize;
}

/// Whether AEAD cipher is used for data encryption or decryption.
pub(crate) enum CipherOp {
    /// Cipher is used for data encryption.
    Encrypt,
    /// Cipher is used for data decryption.
    Decrypt,
}

impl AEADAlgorithm {
    /// Returns the digest size of the AEAD algorithm.
    pub fn digest_size(&self) -> Result<usize> {
        use self::AEADAlgorithm::*;
        match self {
            // According to RFC4880bis, Section 5.16.1.
            EAX => Ok(16),
            // According to RFC4880bis, Section 5.16.2.
            OCB => Ok(16),
            _ => Err(Error::UnsupportedAEADAlgorithm(*self).into()),
        }
    }

    /// Returns the nonce size of the AEAD algorithm.
    pub fn nonce_size(&self) -> Result<usize> {
        use self::AEADAlgorithm::*;
        match self {
            // According to RFC4880bis, Section 5.16.1.
            EAX => Ok(16),
            // According to RFC4880bis, Section 5.16.2, the IV is "at
            // least 15 octets long".  GnuPG hardcodes 15 in
            // openpgp_aead_algo_info.
            OCB => Ok(15),
            _ => Err(Error::UnsupportedAEADAlgorithm(*self).into()),
        }
    }

    /// Returns the initialization vector size of the AEAD algorithm.
    ///
    /// This function is deprecated. Please use
    /// [`AEADAlgorithm::nonce_size`].
    #[deprecated = "Use nonce_size"]
    pub fn iv_size(&self) -> Result<usize> {
        self.nonce_size()
    }
}

/// Schedules nonce and additional authenticated data for use with
/// each AEAD chunk.
pub trait Schedule: Send + Sync {
    /// Calls `fun` with the appropriate nonce and additional
    /// authenticated data.
    ///
    /// This is appropriate for all but the last chunk.
    ///
    /// `index` is the current chunk index.
    fn next_chunk<F, R>(&self, index: u64, fun: F) -> R
    where
        F: FnMut(&[u8], &[u8]) -> R;

    /// Calls `fun` with the appropriate nonce and additional
    /// authenticated data for the last chunk.
    ///
    /// This is appropriate for the last chunk.
    ///
    /// `index` is the current chunk index.
    fn final_chunk<F, R>(&self, index: u64, length: u64, fun: F) -> R
    where
        F: FnMut(&[u8], &[u8]) -> R;
}

const AED1AD_PREFIX_LEN: usize = 5;
pub(crate) struct AEDv1Schedule {
    iv: Box<[u8]>,
    ad: [u8; AED1AD_PREFIX_LEN],
}

impl AEDv1Schedule {
    pub(crate) fn new(sym_algo: SymmetricAlgorithm,
                      aead: AEADAlgorithm,
                      chunk_size: usize,
                      iv: &[u8]) -> Result<Self>
    {
        if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&chunk_size) {
            return Err(Error::InvalidArgument(
                format!("Invalid AEAD chunk size: {}", chunk_size)).into());
        }

        Ok(Self {
            iv: iv.into(),
            ad: [
                // Prefix.
                0xd4, // Packet tag.
                1,    // Version.
                sym_algo.into(),
                aead.into(),
                chunk_size.trailing_zeros() as u8 - 6,
            ],
        })
    }
}

impl Schedule for AEDv1Schedule {
    fn next_chunk<F, R>(&self, index: u64, mut fun: F) -> R
    where
        F: FnMut(&[u8], &[u8]) -> R,
    {
        // Prepare the associated data.
        let mut ad = [0u8; AED1AD_PREFIX_LEN + 8];
        ad[..AED1AD_PREFIX_LEN].copy_from_slice(&self.ad);
        write_be_u64(&mut ad[AED1AD_PREFIX_LEN..],
                     index);

        // The nonce is computed by treating the starting
        // initialization vector as a 16-octet, big-endian value and
        // exclusive-oring the low eight octets of it with the chunk
        // index.
        let mut nonce_store = [0u8; MAX_NONCE_LEN];
        let nonce_len = self.iv.len();
        let nonce = &mut nonce_store[..nonce_len];
        nonce.copy_from_slice(&self.iv);
        let index_be: [u8; 8] = index.to_be_bytes();
        for (i, o) in &mut nonce[nonce_len.saturating_sub(8)..].iter_mut()
            .enumerate()
        {
            // The lower eight octets of the associated data
            // are the big endian representation of the chunk
            // index.
            *o ^= index_be[i];
        }

        fun(nonce, &ad)
    }

    fn final_chunk<F, R>(&self, index: u64, length: u64, mut fun: F) -> R
    where
        F: FnMut(&[u8], &[u8]) -> R,
    {
        // Prepare the associated data.
        let mut ad = [0u8; AED1AD_PREFIX_LEN + 8 + 8];
        ad[..AED1AD_PREFIX_LEN].copy_from_slice(&self.ad);
        write_be_u64(&mut ad[AED1AD_PREFIX_LEN..AED1AD_PREFIX_LEN + 8],
                     index);
        write_be_u64(&mut ad[AED1AD_PREFIX_LEN + 8..],
                     length);

        // The nonce is computed by treating the starting
        // initialization vector as a 16-octet, big-endian value and
        // exclusive-oring the low eight octets of it with the chunk
        // index.
        let mut nonce_store = [0u8; MAX_NONCE_LEN];
        let nonce_len = self.iv.len();
        let nonce = &mut nonce_store[..nonce_len];
        nonce.copy_from_slice(&self.iv);
        let index_be: [u8; 8] = index.to_be_bytes();
        for (i, o) in &mut nonce[nonce_len.saturating_sub(8)..].iter_mut()
            .enumerate()
        {
            // The lower eight octets of the associated data
            // are the big endian representation of the chunk
            // index.
            *o ^= index_be[i];
        }

        fun(nonce, &ad)
    }
}

/// A `Read`er for decrypting AEAD-encrypted data.
pub struct Decryptor<'a, S: Schedule> {
    // The encrypted data.
    source: Box<dyn BufferedReader<Cookie> + 'a>,

    sym_algo: SymmetricAlgorithm,
    aead: AEADAlgorithm,
    key: SessionKey,
    schedule: S,

    digest_size: usize,
    chunk_size: usize,
    chunk_index: u64,
    bytes_decrypted: u64,
    // Up to a chunk of unread data.
    buffer: Vec<u8>,
}
assert_send_and_sync!(Decryptor<'_, S> where S: Schedule);


impl<'a, S: Schedule> Decryptor<'a, S> {
    /// Instantiate a new AEAD decryptor.
    ///
    /// `source` is the source to wrap.
    pub fn new<R>(sym_algo: SymmetricAlgorithm,
                  aead: AEADAlgorithm, chunk_size: usize,
                  schedule: S, key: SessionKey, source: R)
        -> Result<Self>
        where R: io::Read + Send + Sync + 'a
    {
        Self::from_buffered_reader(
            sym_algo, aead, chunk_size, schedule, key,
            Box::new(buffered_reader::Generic::with_cookie(
                source, None, Default::default())))
    }

    pub fn from_buffered_reader(sym_algo: SymmetricAlgorithm,
                            aead: AEADAlgorithm, chunk_size: usize,
                            schedule: S, key: SessionKey,
                            source: Box<dyn 'a + BufferedReader<Cookie>>)
        -> Result<Self>
    {
        Ok(Decryptor {
            source,
            sym_algo,
            aead,
            key,
            schedule,
            digest_size: aead.digest_size()?,
            chunk_size,
            chunk_index: 0,
            bytes_decrypted: 0,
            buffer: Vec::with_capacity(chunk_size),
        })
    }

    // Note: this implementation tries *very* hard to make sure we don't
    // gratuitiously do a short read.  Specifically, if the return value
    // is less than `plaintext.len()`, then it is either because we
    // reached the end of the input or an error occurred.
    fn read_helper(&mut self, plaintext: &mut [u8]) -> Result<usize> {
        use std::cmp::Ordering;

        let mut pos = 0;

        // Buffer to hold a digest.
        let mut digest = vec![0u8; self.digest_size];

        // 1. Copy any buffered data.
        if !self.buffer.is_empty() {
            let to_copy = cmp::min(self.buffer.len(), plaintext.len());
            plaintext[..to_copy].copy_from_slice(&self.buffer[..to_copy]);
            crate::vec_drain_prefix(&mut self.buffer, to_copy);

            pos = to_copy;
            if pos == plaintext.len() {
                return Ok(pos);
            }
        }

        // 2. Decrypt the data a chunk at a time until we've filled
        // `plaintext`.
        //
        // Unfortunately, framing is hard.
        //
        // Recall: AEAD data is of the form:
        //
        //   [ chunk1 ][ tag1 ] ... [ chunkN ][ tagN ][ tagF ]
        //
        // And, all chunks are the same size except for the last
        // chunk, which may be shorter.
        //
        // The naive approach to decryption is to read a chunk and a
        // tag at a time.  Unfortunately, this may not work if the
        // last chunk is a partial chunk.
        //
        // Assume that the chunk size is 32 bytes and the digest size
        // is 16 bytes, and consider a message with 17 bytes of data.
        // That message will be encrypted as follows:
        //
        //   [ chunk1 ][ tag1 ][ tagF ]
        //       17B     16B     16B
        //
        // If we read a chunk and a digest, we'll successfully read 48
        // bytes of data.  Unfortunately, we'll have over read: the
        // last 15 bytes are from the final tag.
        //
        // To correctly handle this case, we have to make sure that
        // there are at least a tag worth of bytes left over when we
        // read a chunk and a tag.

        let n_chunks
            = (plaintext.len() - pos + self.chunk_size - 1) / self.chunk_size;
        let chunk_digest_size = self.chunk_size + self.digest_size;
        let final_digest_size = self.digest_size;

        for _ in 0..n_chunks {
            // Do a little dance to avoid exclusively locking
            // `self.source`.
            let to_read = chunk_digest_size + final_digest_size;
            let result = {
                match self.source.data(to_read) {
                    Ok(_) => Ok(self.source.buffer()),
                    Err(err) => Err(err),
                }
            };

            let check_final_tag;
            let chunk = match result {
                Ok(chunk) => {
                    if chunk.is_empty() {
                        // Exhausted source.
                        return Ok(pos);
                    }

                    if chunk.len() < final_digest_size {
                        return Err(Error::ManipulatedMessage.into());
                    }

                    check_final_tag = chunk.len() < to_read;

                    // Return the chunk.
                    &chunk[..cmp::min(chunk.len(), to_read) - final_digest_size]
                },
                Err(e) => return Err(e.into()),
            };

            assert!(chunk.len() <= chunk_digest_size);

            if chunk.is_empty() {
                // There is nothing to decrypt: all that is left is
                // the final tag.
            } else if chunk.len() <= self.digest_size {
                // A chunk has to include at least one byte and a tag.
                return Err(Error::ManipulatedMessage.into());
            } else {
                let mut aead = self.schedule.next_chunk(
                    self.chunk_index,
                    |iv, ad| {
                        self.aead.context(self.sym_algo, &self.key, iv,
                                          CipherOp::Decrypt)
                            .map(|mut aead| {
                                aead.update(ad);
                                aead
                            })
                    })?;

                // Decrypt the chunk and check the tag.
                let to_decrypt = chunk.len() - self.digest_size;

                // If plaintext doesn't have enough room for the whole
                // chunk, then we have to double buffer.
                let double_buffer = to_decrypt > plaintext.len() - pos;
                let buffer = if double_buffer {
                    self.buffer.resize(to_decrypt, 0);
                    &mut self.buffer[..]
                } else {
                    &mut plaintext[pos..pos + to_decrypt]
                };

                aead.decrypt(buffer, &chunk[..to_decrypt]);

                // Check digest.
                aead.digest(&mut digest);
                if secure_cmp(&digest[..], &chunk[to_decrypt..])
                    != Ordering::Equal && ! DANGER_DISABLE_AUTHENTICATION
                {
                    return Err(Error::ManipulatedMessage.into());
                }

                if double_buffer {
                    let to_copy = plaintext.len() - pos;
                    assert!(0 < to_copy);
                    assert!(to_copy < self.chunk_size);

                    plaintext[pos..pos + to_copy]
                        .copy_from_slice(&self.buffer[..to_copy]);
                    crate::vec_drain_prefix(&mut self.buffer, to_copy);
                    pos += to_copy;
                } else {
                    pos += to_decrypt;
                }

                // Increase index, update position in plaintext.
                self.chunk_index += 1;
                self.bytes_decrypted += to_decrypt as u64;

                // Consume the data only on success so that we keep
                // returning the error.
                let chunk_len = chunk.len();
                self.source.consume(chunk_len);
            }

            if check_final_tag {
                // We read the whole ciphertext, now check the final digest.
                let mut aead = self.schedule.final_chunk(
                    self.chunk_index, self.bytes_decrypted,
                    |iv, ad| {
                        self.aead.context(self.sym_algo, &self.key, iv,
                                          CipherOp::Decrypt)
                            .map(|mut aead| {
                                aead.update(ad);
                                aead
                            })
                    })?;

                aead.digest(&mut digest);

                let final_digest = self.source.data(final_digest_size)?;
                if final_digest.len() != final_digest_size
                    || secure_cmp(&digest[..], final_digest) != Ordering::Equal
                    && ! DANGER_DISABLE_AUTHENTICATION
                {
                    return Err(Error::ManipulatedMessage.into());
                }

                // Consume the data only on success so that we keep
                // returning the error.
                self.source.consume(final_digest_size);
                break;
            }
        }

        Ok(pos)
    }
}

// Note: this implementation tries *very* hard to make sure we don't
// gratuitiously do a short read.  Specifically, if the return value
// is less than `plaintext.len()`, then it is either because we
// reached the end of the input or an error occurred.
impl<'a, S: Schedule> io::Read for Decryptor<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.read_helper(buf) {
            Ok(n) => Ok(n),
            Err(e) => match e.downcast::<io::Error>() {
                // An io::Error.  Pass as-is.
                Ok(e) => Err(e),
                // A failure.  Wrap it.
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
            },
        }
    }
}

/// A `BufferedReader` that decrypts AEAD-encrypted data as it is
/// read.
pub(crate) struct BufferedReaderDecryptor<'a, S: Schedule> {
    reader: buffered_reader::Generic<Decryptor<'a, S>, Cookie>,
}

impl<'a, S: Schedule> BufferedReaderDecryptor<'a, S> {
    /// Like `new()`, but sets a cookie, which can be retrieved using
    /// the `cookie_ref` and `cookie_mut` methods, and set using
    /// the `cookie_set` method.
    pub fn with_cookie(sym_algo: SymmetricAlgorithm,
                       aead: AEADAlgorithm, chunk_size: usize, schedule: S,
                       key: SessionKey, source: Box<dyn BufferedReader<Cookie> + 'a>,
                       cookie: Cookie)
        -> Result<Self>
    {
        Ok(BufferedReaderDecryptor {
            reader: buffered_reader::Generic::with_cookie(
                Decryptor::from_buffered_reader(
                    sym_algo, aead, chunk_size, schedule, key, source)?,
                None, cookie),
        })
    }
}

impl<'a, S: Schedule> io::Read for BufferedReaderDecryptor<'a, S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

impl<'a, S: Schedule> fmt::Display for BufferedReaderDecryptor<'a, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BufferedReaderDecryptor")
    }
}

impl<'a, S: Schedule> fmt::Debug for BufferedReaderDecryptor<'a, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BufferedReaderDecryptor")
            .field("reader", &self.get_ref().unwrap())
            .finish()
    }
}

impl<'a, S: Schedule> BufferedReader<Cookie> for BufferedReaderDecryptor<'a, S> {
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

/// A `Write`r for AEAD encrypting data.
pub struct Encryptor<W: io::Write, S: Schedule> {
    inner: Option<W>,

    sym_algo: SymmetricAlgorithm,
    aead: AEADAlgorithm,
    key: SessionKey,
    schedule: S,

    digest_size: usize,
    chunk_size: usize,
    chunk_index: u64,
    bytes_encrypted: u64,
    // Up to a chunk of unencrypted data.
    buffer: Vec<u8>,

    // A place to write encrypted data into.
    scratch: Vec<u8>,
}
assert_send_and_sync!(Encryptor<W, S> where W: io::Write, S: Schedule);

impl<W: io::Write, S: Schedule> Encryptor<W, S> {
    /// Instantiate a new AEAD encryptor.
    pub fn new(sym_algo: SymmetricAlgorithm, aead: AEADAlgorithm,
               chunk_size: usize, schedule: S, key: SessionKey, sink: W)
               -> Result<Self> {
        Ok(Encryptor {
            inner: Some(sink),
            sym_algo,
            aead,
            key,
            schedule,
            digest_size: aead.digest_size()?,
            chunk_size,
            chunk_index: 0,
            bytes_encrypted: 0,
            buffer: Vec::with_capacity(chunk_size),
            scratch: vec![0; chunk_size],
        })
    }

    // Like io::Write, but returns our Result.
    fn write_helper(&mut self, mut buf: &[u8]) -> Result<usize> {
        if self.inner.is_none() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe,
                                      "Inner writer was taken").into());
        }
        let amount = buf.len();

        // First, fill the buffer if there is something in it.
        if !self.buffer.is_empty() {
            let n = cmp::min(buf.len(), self.chunk_size - self.buffer.len());
            self.buffer.extend_from_slice(&buf[..n]);
            assert!(self.buffer.len() <= self.chunk_size);
            buf = &buf[n..];

            // And possibly encrypt the chunk.
            if self.buffer.len() == self.chunk_size {
                let mut aead =
                    self.schedule.next_chunk(self.chunk_index, |iv, ad| {
                        self.aead.context(self.sym_algo, &self.key, iv,
                                          CipherOp::Encrypt)
                            .map(|mut aead| {
                                aead.update(ad);
                                aead
                            })
                    })?;

                let inner = self.inner.as_mut().unwrap();

                // Encrypt the chunk.
                aead.encrypt(&mut self.scratch, &self.buffer);
                self.bytes_encrypted += self.scratch.len() as u64;
                self.chunk_index += 1;
                crate::vec_truncate(&mut self.buffer, 0);
                inner.write_all(&self.scratch)?;

                // Write digest.
                aead.digest(&mut self.scratch[..self.digest_size]);
                inner.write_all(&self.scratch[..self.digest_size])?;
            }
        }

        // Then, encrypt all whole chunks.
        for chunk in buf.chunks(self.chunk_size) {
            if chunk.len() == self.chunk_size {
                // Complete chunk.
                let mut aead =
                    self.schedule.next_chunk(self.chunk_index, |iv, ad| {
                        self.aead.context(self.sym_algo, &self.key, iv,
                                          CipherOp::Encrypt)
                            .map(|mut aead| {
                                aead.update(ad);
                                aead
                            })
                    })?;

                let inner = self.inner.as_mut().unwrap();

                // Encrypt the chunk.
                aead.encrypt(&mut self.scratch, chunk);
                self.bytes_encrypted += self.scratch.len() as u64;
                self.chunk_index += 1;
                inner.write_all(&self.scratch)?;

                // Write digest.
                aead.digest(&mut self.scratch[..self.digest_size]);
                inner.write_all(&self.scratch[..self.digest_size])?;
            } else {
                // Stash for later.
                assert!(self.buffer.is_empty());
                self.buffer.extend_from_slice(chunk);
            }
        }

        Ok(amount)
    }

    /// Finish encryption and write last partial chunk.
    pub fn finish(&mut self) -> Result<W> {
        if let Some(mut inner) = self.inner.take() {
            if !self.buffer.is_empty() {
                let mut aead =
                    self.schedule.next_chunk(self.chunk_index, |iv, ad| {
                        self.aead.context(self.sym_algo, &self.key, iv,
                                          CipherOp::Encrypt)
                            .map(|mut aead| {
                                aead.update(ad);
                                aead
                            })
                    })?;

                // Encrypt the chunk.
                unsafe { self.scratch.set_len(self.buffer.len()) }
                aead.encrypt(&mut self.scratch, &self.buffer);
                self.bytes_encrypted += self.scratch.len() as u64;
                self.chunk_index += 1;
                crate::vec_truncate(&mut self.buffer, 0);
                inner.write_all(&self.scratch)?;

                // Write digest.
                unsafe { self.scratch.set_len(self.digest_size) }
                aead.digest(&mut self.scratch[..self.digest_size]);
                inner.write_all(&self.scratch[..self.digest_size])?;
            }

            // Write final digest.
            let mut aead = self.schedule.final_chunk(
                self.chunk_index, self.bytes_encrypted,
                |iv, ad| {
                    self.aead.context(self.sym_algo, &self.key, iv,
                                      CipherOp::Encrypt)
                        .map(|mut aead| {
                            aead.update(ad);
                            aead
                        })
                })?;
            aead.digest(&mut self.scratch[..self.digest_size]);
            inner.write_all(&self.scratch[..self.digest_size])?;

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

impl<W: io::Write, S: Schedule> io::Write for Encryptor<W, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.write_helper(buf) {
            Ok(n) => Ok(n),
            Err(e) => match e.downcast::<io::Error>() {
                // An io::Error.  Pass as-is.
                Ok(e) => Err(e),
                // A failure.  Wrap it.
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
            },
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // It is not clear how we can implement this, because we can
        // only operate on chunk sizes.  We will, however, ask our
        // inner writer to flush.
        if let Some(ref mut inner) = self.inner {
            inner.flush()
        } else {
            Err(io::Error::new(io::ErrorKind::BrokenPipe,
                               "Inner writer was taken"))
        }
    }
}

impl<W: io::Write, S: Schedule> Drop for Encryptor<W, S> {
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
    use std::io::{Read, Write};

    /// This test tries to encrypt, then decrypt some data.
    #[test]
    fn roundtrip() {
        use std::io::Cursor;

        // EAX and OCB can be used with all symmetric algorithms using
        // a 16-byte block size.
        for sym_algo in [SymmetricAlgorithm::AES128,
                         SymmetricAlgorithm::AES192,
                         SymmetricAlgorithm::AES256,
                         SymmetricAlgorithm::Twofish,
                         SymmetricAlgorithm::Camellia128,
                         SymmetricAlgorithm::Camellia192,
                         SymmetricAlgorithm::Camellia256]
                         .iter()
                         .filter(|algo| algo.is_supported()) {

            if cfg!(feature = "crypto-rust")
                && sym_algo == &SymmetricAlgorithm::Twofish {
                    eprintln!("XXX: Skipping Twofish until Twofish \
                               implements Clone");
                    continue;
            }

            for aead in [
                AEADAlgorithm::EAX,
                AEADAlgorithm::OCB,
            ].iter().filter(|algo| algo.is_supported()) {
                let chunk_size = 64;
                let mut key = vec![0; sym_algo.key_size().unwrap()];
                crate::crypto::random(&mut key);
                let key: SessionKey = key.into();
                let mut iv = vec![0; aead.nonce_size().unwrap()];
                crate::crypto::random(&mut iv);

                let mut ciphertext = Vec::new();
                {
                    let schedule = AEDv1Schedule::new(
                        *sym_algo,
                        *aead,
                        chunk_size,
                        &iv).expect("valid parameters");
                    let mut encryptor = Encryptor::new(*sym_algo,
                                                       *aead,
                                                       chunk_size,
                                                       schedule,
                                                       key.clone(),
                                                       &mut ciphertext)
                        .unwrap();

                    encryptor.write_all(crate::tests::manifesto()).unwrap();
                }

                let mut plaintext = Vec::new();
                {
                    let schedule = AEDv1Schedule::new(
                        *sym_algo,
                        *aead,
                        chunk_size,
                        &iv).expect("valid parameters");
                    let mut decryptor = Decryptor::new(*sym_algo,
                                                       *aead,
                                                       chunk_size,
                                                       schedule,
                                                       key,
                                                       Cursor::new(&ciphertext))
                        .unwrap();

                    decryptor.read_to_end(&mut plaintext).unwrap();
                }

                assert_eq!(&plaintext[..], crate::tests::manifesto());
            }
        }
    }
}
