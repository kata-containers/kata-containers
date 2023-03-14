//! Padding for OpenPGP messages.
//!
//! To reduce the amount of information leaked via the message length,
//! encrypted OpenPGP messages (see [Section 11.3 of RFC 4880]) should
//! be padded.
//!
//!   [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3
//!
//! To pad a message using the streaming serialization interface, the
//! [`Padder`] needs to be inserted into the writing stack between the
//! [`Encryptor`] and [`Signer`].  This is illustrated in this
//! [example].
//!
//!   [`Encryptor`]: super::Encryptor
//!   [`Signer`]: super::Signer
//!   [example]: Padder#examples
//!
//! # Padding in OpenPGP
//!
//! There are a number of ways to pad messages within the boundaries
//! of the OpenPGP protocol, keeping an eye on backwards-compatibility
//! with common implementations:
//!
//!   - Add a decoy notation to a signature packet (up to about 60k)
//!
//!   - Add a signature with a private algorithm and store the decoy
//!     traffic in the MPIs (up to 4 GB)
//!
//!   - Use a compression container and store the decoy traffic in a
//!     chunk that decompresses to the empty string (unlimited)
//!
//!   - Use a bunch of marker packets, which are ignored (each packet:
//!     3 bytes for the body, 5 bytes for the header)
//!
//!   - Apparently, GnuPG understands a comment packet (tag: 61),
//!     which is not standardized (up to 64k)
//!
//! We believe that padding the compressed data stream is the best
//! option, because as far as OpenPGP is concerned, it is completely
//! transparent for the recipient (for example, no weird packets are
//! inserted).
//!
//! Unfortunately, [testing] discovered problems when the resulting
//! messages are consumed by (at the time of this writing) OpenPGP.js,
//! RNP, and GnuPG.  If compatibility with these implementations is a
//! concern, using this padding method is not advisable.
//!
//!   [testing]: https://tests.sequoia-pgp.org/#Packet_excess_consumption
//!
//! To be effective, the padding layer must be placed inside the
//! encryption container.  To increase compatibility, the padding
//! layer must not be signed.  That is to say, the message structure
//! should be `(encryption (padding ops literal signature))`, the
//! exact structure GnuPG emits by default.
use std::fmt;
use std::io::{self, Write};

use crate::{
    Result,
    packet::prelude::*,
};
use crate::packet::header::CTB;
use crate::serialize::{
    Marshal,
    stream::{
        writer,
        Cookie,
        Message,
        PartialBodyFilter,
    },
};
use crate::types::{
    CompressionAlgorithm,
    CompressionLevel,
};

/// Pads a packet stream.
///
/// Writes a compressed data packet containing all packets written to
/// this writer, and pads it according to the given policy.
///
/// The policy is a `Fn(u64) -> u64`, that given the number of bytes
/// written to this writer `N`, computes the size the compression
/// container should be padded up to.  It is an error to return a
/// number that is smaller than `N`.
///
/// # Compatibility
///
/// This implementation uses the [DEFLATE] compression format.  The
/// packet structure contains a flag signaling the end of the stream
/// (see [Section 3.2.3 of RFC 1951]), and any data appended after
/// that is not part of the stream.
///
/// [DEFLATE]: https://tools.ietf.org/html/rfc1951
/// [Section 3.2.3 of RFC 1951]: https://tools.ietf.org/html/rfc1951#page-9
///
/// [Section 9.3 of RFC 4880] recommends that this algorithm should be
/// implemented, therefore support across various implementations
/// should be good.
///
/// [Section 9.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.3
///
/// # Examples
///
/// This example illustrates the use of `Padder` with the [Padmé]
/// policy.  Note that for brevity, the encryption and signature
/// filters are omitted.
///
/// [Padmé]: padme()
///
/// ```
/// use std::io::Write;
/// use sequoia_openpgp as openpgp;
/// use openpgp::serialize::stream::{Message, LiteralWriter};
/// use openpgp::serialize::stream::padding::Padder;
/// use openpgp::types::CompressionAlgorithm;
/// # fn main() -> sequoia_openpgp::Result<()> {
///
/// let mut unpadded = vec![];
/// {
///     let message = Message::new(&mut unpadded);
///     // XXX: Insert Encryptor here.
///     // XXX: Insert Signer here.
///     let mut message = LiteralWriter::new(message).build()?;
///     message.write_all(b"Hello world.")?;
///     message.finalize()?;
/// }
///
/// let mut padded = vec![];
/// {
///     let message = Message::new(&mut padded);
///     // XXX: Insert Encryptor here.
///     let message = Padder::new(message).build()?;
///     // XXX: Insert Signer here.
///     let mut message = LiteralWriter::new(message).build()?;
///     message.write_all(b"Hello world.")?;
///     message.finalize()?;
/// }
/// assert!(unpadded.len() < padded.len());
/// # Ok(())
/// # }
pub struct Padder<'a> {
    inner: writer::BoxStack<'a, Cookie>,
    policy: fn(u64) -> u64,
}
assert_send_and_sync!(Padder<'_>);

impl<'a> Padder<'a> {
    /// Creates a new padder with the given policy.
    ///
    /// # Examples
    ///
    /// This example illustrates the use of `Padder` with the [Padmé]
    /// policy.
    ///
    /// [Padmé]: padme()
    ///
    /// The most useful filter to push to the writer stack next is the
    /// [`Signer`] or the [`LiteralWriter`].  Finally, literal data
    /// *must* be wrapped using the [`LiteralWriter`].
    ///
    ///   [`Signer`]: super::Signer
    ///   [`LiteralWriter`]: super::LiteralWriter
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::serialize::stream::padding::Padder;
    ///
    /// # let message = openpgp::serialize::stream::Message::new(vec![]);
    /// let message = Padder::new(message).build()?;
    /// // Optionally add a `Signer` here.
    /// // Add a `LiteralWriter` here.
    /// # let _ = message;
    /// # Ok(()) }
    /// ```
    pub fn new(inner: Message<'a>) -> Self {
        Self {
            inner: writer::BoxStack::from(inner),
            policy: padme,
        }
    }

    /// Sets padding policy, returning the padder.
    ///
    /// # Examples
    ///
    /// This example illustrates the use of `Padder` with an explicit policy.
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::serialize::stream::padding::{Padder, padme};
    ///
    /// # let message = openpgp::serialize::stream::Message::new(vec![]);
    /// let message = Padder::new(message).with_policy(padme).build()?;
    /// // Optionally add a `Signer` here.
    /// // Add a `LiteralWriter` here.
    /// # let _ = message;
    /// # Ok(()) }
    /// ```
    pub fn with_policy(mut self, p: fn(u64) -> u64) -> Self {
        self.policy = p;
        self
    }

    /// Builds the padder, returning the writer stack.
    ///
    /// # Examples
    ///
    /// This example illustrates the use of `Padder` with the [Padmé]
    /// policy.
    ///
    /// [Padmé]: padme()
    ///
    /// The most useful filter to push to the writer stack next is the
    /// [`Signer`] or the [`LiteralWriter`].  Finally, literal data
    /// *must* be wrapped using the [`LiteralWriter`].
    ///
    ///   [`Signer`]: super::Signer
    ///   [`LiteralWriter`]: super::LiteralWriter
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::serialize::stream::padding::Padder;
    ///
    /// # let message = openpgp::serialize::stream::Message::new(vec![]);
    /// let message = Padder::new(message).build()?;
    /// // Optionally add a `Signer` here.
    /// // Add a `LiteralWriter` here.
    /// # let _ = message;
    /// # Ok(()) }
    /// ```
    pub fn build(mut self) -> Result<Message<'a>> {
        let mut inner = self.inner;
        let level = inner.cookie_ref().level + 1;

        // Packet header.
        CTB::new(Tag::CompressedData).serialize(&mut inner)?;
        let mut inner: Message<'a>
            = PartialBodyFilter::new(Message::from(inner),
                                     Cookie::new(level));

        // Compressed data header.
        inner.as_mut().write_u8(CompressionAlgorithm::Zip.into())?;

        // Create an appropriate filter.
        self.inner =
            writer::ZIP::new(inner, Cookie::new(level),
                             CompressionLevel::none()).into();

        Ok(Message::from(Box::new(self)))
    }
}

impl<'a> fmt::Debug for Padder<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Padder")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a> io::Write for Padder<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> writer::Stackable<'a, Cookie> for Padder<'a>
{
    fn into_inner(self: Box<Self>)
                  -> Result<Option<writer::BoxStack<'a, Cookie>>> {
        // Make a note of the amount of data written to this filter.
        let uncompressed_size = self.position();

        // Pop-off us and the compression filter, leaving only our
        // partial body encoder on the stack.  This finalizes the
        // compression.
        let mut pb_writer = Box::new(self.inner).into_inner()?.unwrap();

        // Compressed size is what we've actually written out, modulo
        // partial body encoding.
        let compressed_size = pb_writer.position();

        // Sometimes, the compression step expands the data.  Handle
        // this by padding the maximum of both sizes.
        let size = std::cmp::max(uncompressed_size, compressed_size);

        // Compute the amount of padding required according to the
        // given policy.
        let padded_size = (self.policy)(size);
        if padded_size < size {
            return Err(crate::Error::InvalidOperation(
                format!("Padding policy({}) returned {}: smaller than argument",
                        size, padded_size)).into());
        }
        let mut amount = padded_size - compressed_size;

        if false {
            eprintln!("u: {}, c: {}, amount: {}",
                      uncompressed_size, compressed_size, amount);
        }

        // Write 'amount' of padding.
        const BUFFER_SIZE: usize = 4096;
        let mut padding = vec![0; BUFFER_SIZE];
        while amount > 0 {
            let n = std::cmp::min(BUFFER_SIZE as u64, amount) as usize;
            crate::crypto::random(&mut padding[..n]);
            pb_writer.write_all(&padding[..n])?;
            amount -= n as u64;
        }

        pb_writer.into_inner()
    }
    fn pop(&mut self) -> Result<Option<writer::BoxStack<'a, Cookie>>> {
        unreachable!("Only implemented by Signer")
    }
    /// Sets the inner stackable.
    fn mount(&mut self, _new: writer::BoxStack<'a, Cookie>) {
        unreachable!("Only implemented by Signer")
    }
    fn inner_ref(&self) -> Option<&(dyn writer::Stackable<'a, Cookie> + Send + Sync)> {
        Some(self.inner.as_ref())
    }
    fn inner_mut(&mut self) -> Option<&mut (dyn writer::Stackable<'a, Cookie> + Send + Sync)> {
        Some(self.inner.as_mut())
    }
    fn cookie_set(&mut self, cookie: Cookie) -> Cookie {
        self.inner.cookie_set(cookie)
    }
    fn cookie_ref(&self) -> &Cookie {
        self.inner.cookie_ref()
    }
    fn cookie_mut(&mut self) -> &mut Cookie {
        self.inner.cookie_mut()
    }
    fn position(&self) -> u64 {
        self.inner.position()
    }
}

/// Padmé padding scheme.
///
/// Padmé leaks at most O(log log M) bits of information (with M being
/// the maximum length of all messages) with an overhead of at most
/// 12%, decreasing with message size.
///
/// This scheme leaks the same order of information as padding to the
/// next power of two, while avoiding an overhead of up to 100%.
///
/// See Section 4 of [Reducing Metadata Leakage from Encrypted Files
/// and Communication with
/// PURBs](https://bford.info/pub/sec/purb.pdf).
///
/// This function is meant to be used with [`Padder`], see this
/// [example].
///
///   [example]: Padder#examples
#[allow(clippy::many_single_char_names)]
pub fn padme(l: u64) -> u64 {
    if l < 2 {
        return 1; // Avoid cornercase.
    }

    let e = log2(l);               // l's floating-point exponent
    let s = log2(e as u64) + 1;    // # of bits to represent e
    let z = e - s;                 // # of low bits to set to 0
    let m = (1 << z) - 1;          // mask of z 1's in LSB
    (l + (m as u64)) & !(m as u64) // round up using mask m to clear last z bits
}

/// Compute the log2 of an integer.  (This is simply the most
/// significant bit.)  Note: log2(0) = -Inf, but this function returns
/// log2(0) as 0 (which is the closest number that we can represent).
fn log2(x: u64) -> usize {
    if x == 0 {
        0
    } else {
        63 - x.leading_zeros() as usize
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn log2_test() {
        for i in 0..64 {
            assert_eq!(log2(1u64 << i), i);
            if i > 0 {
                assert_eq!(log2((1u64 << i) - 1), i - 1);
                assert_eq!(log2((1u64 << i) + 1), i);
            }
        }
    }

    fn padme_multiplicative_overhead(p: u64) -> f32 {
        let c = padme(p);
        let (p, c) = (p as f32, c as f32);
        (c - p) / p
    }

    /// Experimentally, we observe the maximum overhead to be ~11.63%
    /// when padding 129 bytes to 144.
    const MAX_OVERHEAD: f32 = 0.1163;

    #[test]
    fn padme_max_overhead() {
        // The paper says the maximum multiplicative overhead is
        // 11.(11)% when padding 9 bytes to 10.
        assert!(0.111 < padme_multiplicative_overhead(9));
        assert!(padme_multiplicative_overhead(9) < 0.112);

        // Contrary to that, we observe the maximum overhead to be
        // ~11.63% when padding 129 bytes to 144.
        assert!(padme_multiplicative_overhead(129) < MAX_OVERHEAD);
    }

    quickcheck! {
        fn padme_overhead(l: u32) -> bool {
            if l < 2 {
                return true; // Avoid cornercase.
            }

            let o = padme_multiplicative_overhead(l as u64);
            let l_ = l as f32;
            let e = l_.log2().floor();     // l's floating-point exponent
            let s = e.log2().floor() + 1.; // # of bits to represent e
            let max_overhead = (2.0_f32.powf(e-s) - 1.) / l_;

            assert!(o < MAX_OVERHEAD,
                    "padme({}) => {}: overhead {} exceeds maximum overhead {}",
                    l, padme(l.into()), o, MAX_OVERHEAD);
            assert!(o <= max_overhead,
                    "padme({}) => {}: overhead {} exceeds maximum overhead {}",
                    l, padme(l.into()), o, max_overhead);
            true
        }
    }

    /// Asserts that we can consume the padded messages.
    #[test]
    fn roundtrip() {
        use std::io::Write;
        use crate::parse::Parse;
        use crate::serialize::stream::*;

        let mut msg = vec![0; rand::random::<usize>() % 1024];
        crate::crypto::random(&mut msg);

        let mut padded = vec![];
        {
            let message = Message::new(&mut padded);
            let padder = Padder::new(message).with_policy(padme).build().unwrap();
            let mut w = LiteralWriter::new(padder).build().unwrap();
            w.write_all(&msg).unwrap();
            w.finalize().unwrap();
        }

        let m = crate::Message::from_bytes(&padded).unwrap();
        assert_eq!(m.body().unwrap().body(), &msg[..]);
    }

    /// Asserts that no actual compression is done.
    ///
    /// We want to avoid having the size of the data stream depend on
    /// the data's compressibility, therefore it is best to disable
    /// the compression.
    #[test]
    fn no_compression() {
        use std::io::Write;
        use crate::serialize::stream::*;
        const MSG: &[u8] = b"@@@@@@@@@@@@@@";
        let mut padded = vec![];
        {
            let message = Message::new(&mut padded);
            let padder = Padder::new(message).build().unwrap();
            let mut w = LiteralWriter::new(padder).build().unwrap();
            w.write_all(MSG).unwrap();
            w.finalize().unwrap();
        }

        assert!(padded.windows(MSG.len()).any(|ch| ch == MSG),
                "Could not find uncompressed message");
    }
}
