//! AEAD encrypted data packets.
//!
//! An encryption container using [Authenticated Encryption with
//! Additional Data].
//!
//! The AED packet is a new packet specified in [Section 5.16 of RFC
//! 4880bis].  Its aim is to replace the [SEIP packet], whose security
//! has been partially compromised.  SEIP's weaknesses includes its
//! use of CFB mode (e.g., EFAIL-style CFB gadgets, see Section 5.3 of
//! the [EFAIL paper]), its use of [SHA-1] for integrity protection, and
//! the ability to [downgrade SEIP packets] to much weaker SED
//! packets.
//!
//! Although the decision to use AEAD is uncontroversial, the design
//! specified in RFC 4880bis is.  According to [RFC 5116], decrypted
//! AEAD data can only be released for processing after its
//! authenticity has been checked:
//!
//! > [The authenticated decryption operation] has only a single
//! > output, either a plaintext value P or a special symbol FAIL that
//! > indicates that the inputs are not authentic.
//!
//! The controversy has to do with streaming, which OpenPGP has
//! traditionally supported.  Streaming a message means that the
//! amount of data that needs to be buffered when processing a message
//! is independent of the message's length.
//!
//! At first glance, the AEAD mechanism in RFC 4880bis appears to
//! support this mode of operation: instead of encrypting the whole
//! message using AEAD, which would require buffering all of the
//! plaintext when decrypting the message, the message is chunked, the
//! individual chunks are linked together, and AEAD is used to encrypt
//! and protect each individual chunk.  Because the plaintext from an
//! individual chunk can be integrity checked, an implementation only
//! needs to buffer a chunk worth of data.
//!
//! Unfortunately, RFC 4880bis allows chunk sizes that are, in
//! practice, unbounded.  Specifically, a chunk can be up to 4
//! exbibytes in size.  Thus when encountering messages that can't be
//! buffered, an OpenPGP implementation has a choice: it can either
//! release data that has not been integrity checked and violate RFC
//! 5116, or it can fail to process the message.  As of 2020, [GnuPG]
//! and [RNP] process unauthenticated plaintext.  From a user
//! perspective, it then appears that implementations that choose to
//! follow RFC 5116 are impaired: "GnuPG can decrypt it," they think,
//! "why can't Sequoia?"  This creates pressure on other
//! implementations to also behave insecurely.
//!
//! [Werner argues] that AEAD is not about authenticating the data.
//! That is the purpose of the signature.  The reason to introduce
//! AEAD is to get the benefits of more modern cryptography, and to be
//! able to more quickly detect rare transmission errors.  Our
//! position is that an integrity check provides real protection: it
//! can detect modified ciphertext.  And, if we are going to stream,
//! then this protection is essential as it protects the user from
//! real, demonstrated attacks like [EFAIL].
//!
//! RFC 4880bis has not been finalized.  So, it is still possible that
//! the AEAD mechanism will change (which is why the AED packet is
//! marked as experimental).  Despite our concerns, because other
//! OpenPGP implementations already emit the AEAD packet, we provide
//! *experimental* support for it in Sequoia.
//!
//! [Authenticated Encryption with Additional Data]: https://en.wikipedia.org/wiki/Authenticated_encryption
//! [Section 5.16 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-5.16
//! [EFAIL paper]: https://www.usenix.org/conference/usenixsecurity18/presentation/poddebniak
//! [SHA-1]: https://sha-mbles.github.io/
//! [SEIP packet]: https://tools.ietf.org/html/rfc4880#section-5.13
//! [RFC 5116]: https://tools.ietf.org/html/rfc5116#section-2.2
//! [downgrade SEIP packets]: https://mailarchive.ietf.org/arch/msg/openpgp/JLn7sL6TqikUf-cD34lN7kof7_A/
//! [GnuPG]: https://mailarchive.ietf.org/arch/msg/openpgp/fmQgRm94jhvPLEOi0J-o7A8LpkY/
//! [RNP]: https://github.com/rnpgp/rnp/issues/807
//! [Werner argues]: https://mailarchive.ietf.org/arch/msg/openpgp/J428Mqq3-pHTU4C76EgP5sPkvtA
//! [EFAIL]: https://efail.de/

use crate::types::{
    AEADAlgorithm,
    SymmetricAlgorithm,
};
use crate::packet;
use crate::Packet;
use crate::Error;
use crate::Result;

/// Holds an AEAD encrypted data packet.
///
/// An AEAD encrypted data packet holds encrypted data.  The data
/// contains additional OpenPGP packets.  See [Section 5.16 of RFC
/// 4880bis] for details.
///
/// An AED packet is not normally instantiated directly.  In most
/// cases, you'll create one as a side-effect of encrypting a message
/// using the [streaming serializer], or parsing an encrypted message
/// using the [`PacketParser`].
///
/// This feature is
/// [experimental](super::super#experimental-features).  It has
/// not been standardized and we advise users to not emit AED packets.
///
/// [Section 5.16 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-05#section-5.16
/// [streaming serializer]: crate::serialize::stream
/// [`PacketParser`]: crate::parse::PacketParser
///
/// # A note on equality
///
/// An unprocessed (encrypted) `AED` packet is never considered equal
/// to a processed (decrypted) one.  Likewise, a processed (decrypted)
/// packet is never considered equal to a structured (parsed) one.
// IMPORTANT: If you add fields to this struct, you need to explicitly
// IMPORTANT: implement PartialEq, Eq, and Hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AED1 {
    /// CTB packet header fields.
    pub(crate) common: packet::Common,
    /// Symmetric algorithm.
    sym_algo: SymmetricAlgorithm,
    /// AEAD algorithm.
    aead: AEADAlgorithm,
    /// Chunk size.
    chunk_size: u64,
    /// Initialization vector for the AEAD algorithm.
    iv: Box<[u8]>,

    /// This is a container packet.
    container: packet::Container,
}
assert_send_and_sync!(AED1);

impl std::ops::Deref for AED1 {
    type Target = packet::Container;
    fn deref(&self) -> &Self::Target {
        &self.container
    }
}

impl std::ops::DerefMut for AED1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.container
    }
}

impl AED1 {
    /// Creates a new AED1 object.
    pub fn new(sym_algo: SymmetricAlgorithm,
               aead: AEADAlgorithm,
               chunk_size: u64,
               iv: Box<[u8]>) -> Result<Self> {
        if chunk_size.count_ones() != 1 {
            return Err(Error::InvalidArgument(
                format!("chunk size is not a power of two: {}", chunk_size))
                .into());
        }

        if chunk_size < 64 {
            return Err(Error::InvalidArgument(
                format!("chunk size is too small: {}", chunk_size))
                .into());
        }

        Ok(AED1 {
            common: Default::default(),
            sym_algo,
            aead,
            chunk_size,
            iv,
            container: Default::default(),
        })
    }

    /// Gets the symmetric algorithm.
    pub fn symmetric_algo(&self) -> SymmetricAlgorithm {
        self.sym_algo
    }

    /// Sets the symmetric algorithm.
    pub fn set_symmetric_algo(&mut self, sym_algo: SymmetricAlgorithm)
                              -> SymmetricAlgorithm {
        ::std::mem::replace(&mut self.sym_algo, sym_algo)
    }

    /// Gets the AEAD algorithm.
    pub fn aead(&self) -> AEADAlgorithm {
        self.aead
    }

    /// Sets the AEAD algorithm.
    pub fn set_aead(&mut self, aead: AEADAlgorithm) -> AEADAlgorithm {
        ::std::mem::replace(&mut self.aead, aead)
    }

    /// Gets the chunk size.
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    /// Sets the chunk size.
    pub fn set_chunk_size(&mut self, chunk_size: u64) -> Result<()> {
        if chunk_size.count_ones() != 1 {
            return Err(Error::InvalidArgument(
                format!("chunk size is not a power of two: {}", chunk_size))
                .into());
        }

        if chunk_size < 64 {
            return Err(Error::InvalidArgument(
                format!("chunk size is too small: {}", chunk_size))
                .into());
        }

        self.chunk_size = chunk_size;
        Ok(())
    }

    /// Gets the size of a chunk with a digest.
    pub fn chunk_digest_size(&self) -> Result<u64> {
        Ok(self.chunk_size + self.aead.digest_size()? as u64)
    }

    /// Gets the initialization vector for the AEAD algorithm.
    pub fn iv(&self) -> &[u8] {
        &self.iv
    }

    /// Sets the initialization vector for the AEAD algorithm.
    pub fn set_iv(&mut self, iv: Box<[u8]>) -> Box<[u8]> {
        ::std::mem::replace(&mut self.iv, iv)
    }
}

impl From<AED1> for Packet {
    fn from(p: AED1) -> Self {
        super::AED::from(p).into()
    }
}

impl From<AED1> for super::AED {
    fn from(p: AED1) -> Self {
        super::AED::V1(p)
    }
}
