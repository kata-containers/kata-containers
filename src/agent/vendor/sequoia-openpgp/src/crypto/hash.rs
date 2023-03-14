//! Cryptographic hash functions and hashing of OpenPGP data
//! structures.
//!
//! This module provides trait [`Digest`] representing a hash function
//! context independent of the cryptographic backend, as well as trait
//! [`Hash`] that handles hashing of OpenPGP data structures.
//!
//!
//! # Examples
//!
//! ```rust
//! # fn main() -> sequoia_openpgp::Result<()> {
//! use sequoia_openpgp::types::HashAlgorithm;
//!
//! // Create a context and feed data to it.
//! let mut ctx = HashAlgorithm::SHA512.context()?;
//! ctx.update(&b"The quick brown fox jumps over the lazy dog."[..]);
//!
//! // Extract the digest.
//! let mut digest = vec![0; ctx.digest_size()];
//! ctx.digest(&mut digest);
//!
//! use sequoia_openpgp::fmt::hex;
//! assert_eq!(&hex::encode(digest),
//!            "91EA1245F20D46AE9A037A989F54F1F7\
//!             90F0A47607EEB8A14D12890CEA77A1BB\
//!             C6C7ED9CF205E67B7F2B8FD4C7DFD3A7\
//!             A8617E45F3C463D481C7E586C39AC1ED");
//! # Ok(()) }
//! ```

use std::convert::TryFrom;

use dyn_clone::DynClone;

use crate::HashAlgorithm;
use crate::packet::Key;
use crate::packet::UserID;
use crate::packet::UserAttribute;
use crate::packet::key;
use crate::packet::key::Key4;
use crate::packet::Signature;
use crate::packet::signature::{self, Signature3, Signature4};
use crate::Result;
use crate::types::Timestamp;

use std::fs::{File, OpenOptions};
use std::io::{self, Write};

// If set to e.g. Some("/tmp/hash"), we will dump everything that is
// hashed to files /tmp/hash-N, where N is a number.
const DUMP_HASHED_VALUES: Option<&str> = None;

// ASN.1 OID values copied from the nettle-rs crate:
// https://gitlab.com/sequoia-pgp/nettle-rs/-/blob/main/src/rsa/pkcs1.rs#L22

/// ASN.1 OID for MD5
const ASN1_OID_MD5: &[u8] = &[
    0x30, 0x20, 0x30, 0x0c, 0x06, 0x08, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d,
    0x02, 0x05, 0x05, 0x00, 0x04, 0x10,
];

/// ASN.1 OID for RipeMD160
const ASN1_OID_RIPEMD160: &[u8] = &[
    0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2B, 0x24, 0x03, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x14,
];

/// ASN.1 OID for SHA1
const ASN1_OID_SHA1: &[u8] = &[
    0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05,
    0x00, 0x04, 0x14,
];

/// ASN.1 OID for SHA224
const ASN1_OID_SHA224: &[u8] = &[
    0x30, 0x2D, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
    0x04, 0x02, 0x04, 0x05, 0x00, 0x04, 0x1C,
];

/// ASN.1 OID for SHA256
const ASN1_OID_SHA256: &[u8] = &[
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
    0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20,
];

/// ASN.1 OID for SHA384
const ASN1_OID_SHA384: &[u8] = &[
    0x30, 0x41, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
    0x04, 0x02, 0x02, 0x05, 0x00, 0x04, 0x30,
];

/// ASN.1 OID for SHA512
const ASN1_OID_SHA512: &[u8] = &[
    0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
    0x04, 0x02, 0x03, 0x05, 0x00, 0x04, 0x40,
];

lazy_static::lazy_static! {
    /// List of hashes that the signer may produce.
    /// This list is ordered by the preference so that the most preferred
    /// hash algorithm is first.
    pub(crate) static ref DEFAULT_HASHES: Vec<HashAlgorithm> = vec![
        HashAlgorithm::default(),
        HashAlgorithm::SHA512,
        HashAlgorithm::SHA384,
        HashAlgorithm::SHA256,
        HashAlgorithm::SHA224,
        HashAlgorithm::SHA1,
        HashAlgorithm::RipeMD,
        HashAlgorithm::MD5,
    ];

    pub(crate) static ref DEFAULT_HASHES_SORTED: Vec<HashAlgorithm> = {
        let mut hashes = DEFAULT_HASHES.clone();
        hashes.sort();
        hashes
    };
}

/// Hasher capable of calculating a digest for the input byte stream.
///
/// This provides an abstract interface to the hash functions used in
/// OpenPGP.  `Digest`s can be are created using [`HashAlgorithm::context`].
///
///   [`HashAlgorithm::context`]: crate::types::HashAlgorithm::context()
pub trait Digest: DynClone + Write + Send + Sync {
    /// Returns the algorithm.
    fn algo(&self) -> HashAlgorithm;

    /// Size of the digest in bytes
    fn digest_size(&self) -> usize;

    /// Writes data into the hash function.
    fn update(&mut self, data: &[u8]);

    /// Finalizes the hash function and writes the digest into the
    /// provided slice.
    ///
    /// Resets the hash function contexts.
    ///
    /// `digest` must be at least `self.digest_size()` bytes large,
    /// otherwise the digest will be truncated.
    fn digest(&mut self, digest: &mut [u8]) -> Result<()>;

    /// Finalizes the hash function and computes the digest.
    fn into_digest(mut self) -> Result<Vec<u8>>
        where Self: std::marker::Sized
    {
        let mut digest = vec![0u8; self.digest_size()];
        self.digest(&mut digest)?;
        Ok(digest)
    }
}

dyn_clone::clone_trait_object!(Digest);

impl Digest for Box<dyn Digest> {
    fn algo(&self) -> HashAlgorithm {
        self.as_ref().algo()
    }
    fn digest_size(&self) -> usize {
        self.as_ref().digest_size()
    }

    /// Writes data into the hash function.
    fn update(&mut self, data: &[u8]) {
        self.as_mut().update(data)
    }

    /// Finalizes the hash function and writes the digest into the
    /// provided slice.
    ///
    /// Resets the hash function contexts.
    ///
    /// `digest` must be at least [`self.digest_size()`] bytes large,
    /// otherwise the digest will be truncated.
    ///
    ///   [`self.digest_size()`]: Box::digest_size()
    fn digest(&mut self, digest: &mut [u8]) -> Result<()>{
        self.as_mut().digest(digest)
    }
}

impl HashAlgorithm {
    /// Creates a new hash context for this algorithm.
    ///
    /// # Errors
    ///
    /// Fails with `Error::UnsupportedHashAlgorithm` if Sequoia does
    /// not support this algorithm. See
    /// [`HashAlgorithm::is_supported`].
    ///
    ///   [`HashAlgorithm::is_supported`]: HashAlgorithm::is_supported()
    pub fn context(self) -> Result<Box<dyn Digest>> {
        let hasher: Box<dyn Digest> = match self {
            HashAlgorithm::SHA1 =>
                Box::new(crate::crypto::backend::sha1cd::build()),
            _ => self.new_hasher()?,
        };
        Ok(if let Some(prefix) = DUMP_HASHED_VALUES {
            Box::new(HashDumper::new(hasher, prefix))
        } else {
            hasher
        })
    }

    /// Returns the prefix of a serialized `DigestInfo` structure
    /// that contains the ASN.1 OID of this hash algorithm.
    ///
    /// The prefix is used for encoding RSA signatures according to
    /// the `EMSA-PKCS1-v1_5` algorithm as specified in [RFC 8017].
    ///
    /// [RFC 8017]: https://www.rfc-editor.org/rfc/rfc8017.html#section-9.2
    ///
    /// ```
    /// # use sequoia_openpgp::types::HashAlgorithm;
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// let algo = HashAlgorithm::SHA512;
    /// let digest = // raw bytes of the digest
    /// # Vec::<u8>::new();
    /// let digest_info = Vec::from(algo.oid()?).extend(digest);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with `Error::UnsupportedHashAlgorithm` for unknown or
    /// private hash algorithms.
    pub fn oid(self) -> Result<&'static [u8]> {
        match self {
            HashAlgorithm::SHA1 => Ok(ASN1_OID_SHA1),
            HashAlgorithm::SHA224 => Ok(ASN1_OID_SHA224),
            HashAlgorithm::SHA256 => Ok(ASN1_OID_SHA256),
            HashAlgorithm::SHA384 => Ok(ASN1_OID_SHA384),
            HashAlgorithm::SHA512 => Ok(ASN1_OID_SHA512),
            HashAlgorithm::MD5 => Ok(ASN1_OID_MD5),
            HashAlgorithm::RipeMD => Ok(ASN1_OID_RIPEMD160),
            HashAlgorithm::Private(_) | HashAlgorithm::Unknown(_) =>
                Err(crate::Error::UnsupportedHashAlgorithm(self).into()),
        }
    }
}

struct HashDumper {
    hasher: Box<dyn Digest>,
    sink: File,
    filename: String,
    written: usize,
}

impl HashDumper {
    fn new(hasher: Box<dyn Digest>, prefix: &str) -> Self {
        let mut n = 0;
        let mut filename;
        let sink = loop {
            filename = format!("{}-{}", prefix, n);
            match OpenOptions::new().write(true).create_new(true)
                .open(&filename)
            {
                Ok(f) => break f,
                Err(_) => n += 1,
            }
        };
        eprintln!("HashDumper: Writing to {}...", &filename);
        HashDumper {
            hasher,
            sink,
            filename,
            written: 0,
        }
    }
}

impl Clone for HashDumper {
    fn clone(&self) -> HashDumper {
        // We only ever create instances of HashDumper when debugging.
        // Whenever we're cloning an instance, just open another file for
        // inspection.
        let prefix = DUMP_HASHED_VALUES
            .expect("cloning a HashDumper but DUMP_HASHED_VALUES wasn't specified");
        HashDumper::new(self.hasher.clone(), prefix)
    }
}

impl Drop for HashDumper {
    fn drop(&mut self) {
        eprintln!("HashDumper: Wrote {} bytes to {}...", self.written,
                  self.filename);
    }
}

impl Digest for HashDumper {
    fn algo(&self) -> HashAlgorithm {
        self.hasher.algo()
    }

    fn digest_size(&self) -> usize {
        self.hasher.digest_size()
    }
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
        self.sink.write_all(data).unwrap();
        self.written += data.len();
    }
    fn digest(&mut self, digest: &mut [u8]) -> Result<()> {
        self.hasher.digest(digest)
    }
}

impl io::Write for HashDumper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.hasher.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.hasher.flush()
    }
}

/// Hashes OpenPGP packets and related types.
///
/// Some OpenPGP data structures need to be hashed to be covered by
/// OpenPGP signatures.  Hashing is often based on the serialized
/// form, with some aspects fixed to ensure consistent results.  This
/// trait implements hashing as specified by OpenPGP.
///
/// Most of the time it is not necessary to manually compute hashes.
/// Instead, higher level functionality, like the streaming
/// [`Verifier`], [`DetachedVerifier`], or [`Signature`'s verification
/// functions] should be used, which handle the hashing internally.
///
///   [`Verifier`]: crate::parse::stream::Verifier
///   [`DetachedVerifier`]: crate::parse::stream::DetachedVerifier
///   [`Signature`'s verification functions]: crate::packet::Signature#verification-functions
///
/// This is a low-level mechanism.  See [`Signature`'s hashing
/// functions] for how to hash compounds like (Key,UserID)-bindings.
///
///   [`Signature`'s hashing functions]: crate::packet::Signature#hashing-functions
pub trait Hash {
    /// Updates the given hash with this object.
    fn hash(&self, hash: &mut dyn Digest);
}

impl Hash for UserID {
    fn hash(&self, hash: &mut dyn Digest) {
        let len = self.value().len() as u32;

        let mut header = [0; 5];
        header[0] = 0xB4;
        header[1..5].copy_from_slice(&len.to_be_bytes());

        hash.update(&header);
        hash.update(self.value());
    }
}

impl Hash for UserAttribute {
    fn hash(&self, hash: &mut dyn Digest) {
        let len = self.value().len() as u32;

        let mut header = [0; 5];
        header[0] = 0xD1;
        header[1..5].copy_from_slice(&len.to_be_bytes());

        hash.update(&header);
        hash.update(self.value());
    }
}

impl<P, R> Hash for Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn hash(&self, hash: &mut dyn Digest) {
        use crate::serialize::MarshalInto;

        // We hash 9 bytes plus the MPIs.  But, the len doesn't
        // include the tag (1 byte) or the length (2 bytes).
        let len = (9 - 3) + self.mpis().serialized_len() as u16;

        let mut header: Vec<u8> = Vec::with_capacity(9);

        // Tag.  Note: we use this whether
        header.push(0x99);

        // Length (2 bytes, big endian).
        header.extend_from_slice(&len.to_be_bytes());

        // Version.
        header.push(4);

        // Creation time.
        let creation_time: u32 =
            Timestamp::try_from(self.creation_time())
            .unwrap_or_else(|_| Timestamp::from(0))
            .into();
        header.extend_from_slice(&creation_time.to_be_bytes());

        // Algorithm.
        header.push(self.pk_algo().into());

        hash.update(&header[..]);

        // MPIs.
        self.mpis().hash(hash);
    }
}

impl Hash for Signature {
    fn hash(&self, hash: &mut dyn Digest) {
        match self {
            Signature::V3(sig) => sig.hash(hash),
            Signature::V4(sig) => sig.hash(hash),
        }
    }
}

impl Hash for Signature3 {
    fn hash(&self, hash: &mut dyn Digest) {
        // XXX: Annoyingly, we have no proper way of handling errors
        // here.

        let mut buffer = [0u8; 5];

        // Signature type.
        buffer[0] = u8::from(self.typ());

        // Creation time.
        let creation_time: u32 =
            Timestamp::try_from(
                self.signature_creation_time()
                    .unwrap_or(std::time::UNIX_EPOCH))
            .unwrap_or_else(|_| Timestamp::from(0))
            .into();

        buffer[1] = (creation_time >> 24) as u8;
        buffer[2] = (creation_time >> 16) as u8;
        buffer[3] = (creation_time >>  8) as u8;
        buffer[4] = (creation_time      ) as u8;

        hash.update(&buffer[..]);
    }
}

impl Hash for Signature4 {
    fn hash(&self, hash: &mut dyn Digest) {
        self.fields.hash(hash);
    }
}

impl Hash for signature::SignatureFields {
    fn hash(&self, hash: &mut dyn Digest) {
        use crate::serialize::MarshalInto;

        // XXX: Annoyingly, we have no proper way of handling errors
        // here.
        let hashed_area = self.hashed_area().to_vec()
            .unwrap_or_else(|_| Vec::new());

        // A version 4 signature packet is laid out as follows:
        //
        //   version - 1 byte                    \
        //   type - 1 byte                        \
        //   pk_algo - 1 byte                      \
        //   hash_algo - 1 byte                      Included in the hash
        //   hashed_area_len - 2 bytes (big endian)/
        //   hashed_area                         _/
        //   ...                                 <- Not included in the hash

        let mut header = [0u8; 6];

        // Version.
        header[0] = 4;
        header[1] = self.typ().into();
        header[2] = self.pk_algo().into();
        header[3] = self.hash_algo().into();

        // The length of the hashed area, as a 16-bit big endian number.
        let len = hashed_area.len() as u16;
        header[4..6].copy_from_slice(&len.to_be_bytes());

        hash.update(&header[..]);
        hash.update(&hashed_area);

        // A version 4 signature trailer is:
        //
        //   version - 1 byte
        //   0xFF (constant) - 1 byte
        //   amount - 4 bytes (big endian)
        //
        // The amount field is the amount of hashed from this
        // packet (this excludes the message content, and this
        // trailer).
        //
        // See https://tools.ietf.org/html/rfc4880#section-5.2.4
        let mut trailer = [0u8; 6];

        trailer[0] = 4;
        trailer[1] = 0xff;
        // The signature packet's length, not including the previous
        // two bytes and the length.
        let len = (header.len() + hashed_area.len()) as u32;
        trailer[2..6].copy_from_slice(&len.to_be_bytes());

        hash.update(&trailer[..]);
    }
}

/// Hashing-related functionality.
///
/// <a id="hashing-functions"></a>
impl signature::SignatureFields {
    /// Hashes this standalone signature.
    pub fn hash_standalone(&self, hash: &mut dyn Digest)
    {
        self.hash(hash);
    }

    /// Hashes this timestamp signature.
    pub fn hash_timestamp(&self, hash: &mut dyn Digest)
    {
        self.hash_standalone(hash);
    }

    /// Hashes this direct key signature over the specified primary
    /// key, and the primary key.
    pub fn hash_direct_key<P>(&self, hash: &mut dyn Digest,
                              key: &Key<P, key::PrimaryRole>)
        where P: key::KeyParts,
    {
        key.hash(hash);
        self.hash(hash);
    }

    /// Hashes this subkey binding over the specified primary key and
    /// subkey, the primary key, and the subkey.
    pub fn hash_subkey_binding<P, Q>(&self, hash: &mut dyn Digest,
                                     key: &Key<P, key::PrimaryRole>,
                                     subkey: &Key<Q, key::SubordinateRole>)
        where P: key::KeyParts,
              Q: key::KeyParts,
    {
        key.hash(hash);
        subkey.hash(hash);
        self.hash(hash);
    }

    /// Hashes this primary key binding over the specified primary key
    /// and subkey, the primary key, and the subkey.
    pub fn hash_primary_key_binding<P, Q>(&self, hash: &mut dyn Digest,
                                          key: &Key<P, key::PrimaryRole>,
                                          subkey: &Key<Q, key::SubordinateRole>)
        where P: key::KeyParts,
              Q: key::KeyParts,
    {
        self.hash_subkey_binding(hash, key, subkey);
    }

    /// Hashes this user ID binding over the specified primary key and
    /// user ID, the primary key, and the userid.
    pub fn hash_userid_binding<P>(&self, hash: &mut dyn Digest,
                                  key: &Key<P, key::PrimaryRole>,
                                  userid: &UserID)
        where P: key::KeyParts,
    {
        key.hash(hash);
        userid.hash(hash);
        self.hash(hash);
    }

    /// Hashes this user attribute binding over the specified primary
    /// key and user attribute, the primary key, and the user
    /// attribute.
    pub fn hash_user_attribute_binding<P>(
        &self,
        hash: &mut dyn Digest,
        key: &Key<P, key::PrimaryRole>,
        ua: &UserAttribute)
        where P: key::KeyParts,
    {
        key.hash(hash);
        ua.hash(hash);
        self.hash(hash);
    }
}

/// Hashing-related functionality.
///
/// <a id="hashing-functions"></a>
impl Signature {
    /// Hashes this signature for use in a Third-Party Confirmation
    /// signature.
    pub fn hash_for_confirmation(&self, hash: &mut dyn Digest) {
        match self {
            Signature::V3(s) => s.hash_for_confirmation(hash),
            Signature::V4(s) => s.hash_for_confirmation(hash),
        }
    }
}

/// Hashing-related functionality.
///
/// <a id="hashing-functions"></a>
impl Signature4 {
    /// Hashes this signature for use in a Third-Party Confirmation
    /// signature.
    pub fn hash_for_confirmation(&self, hash: &mut dyn Digest) {
        use crate::serialize::{Marshal, MarshalInto};
        // Section 5.2.4 of RFC4880:
        //
        // > When a signature is made over a Signature packet (type
        // > 0x50), the hash data starts with the octet 0x88, followed
        // > by the four-octet length of the signature, and then the
        // > body of the Signature packet.  (Note that this is an
        // > old-style packet header for a Signature packet with the
        // > length-of-length set to zero.)  The unhashed subpacket
        // > data of the Signature packet being hashed is not included
        // > in the hash, and the unhashed subpacket data length value
        // > is set to zero.

        // This code assumes that the signature has been verified
        // prior to being confirmed, so it is well-formed.
        let mut body = vec![
            self.version(),
            self.typ().into(),
            self.pk_algo().into(),
            self.hash_algo().into(),
        ];

        // The hashed area.
        let l = self.hashed_area().serialized_len()
             // Assumes well-formedness.
            .min(std::u16::MAX as usize);
        body.extend(&(l as u16).to_be_bytes());
         // Assumes well-formedness.
        let _ = self.hashed_area().serialize(&mut body);

        // The unhashed area.
        body.extend(&[0, 0]); // Size replaced by zero.
        // Unhashed packets omitted.

        body.extend(self.digest_prefix());
        let _ = self.mpis().serialize(&mut body);

        hash.update(&[0x88]);
        hash.update(&(body.len() as u32).to_be_bytes());
        hash.update(&body);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Cert;
    use crate::parse::Parse;

    #[test]
    fn hash_verification() {
        fn check(cert: Cert) -> (usize, usize, usize) {
            let mut userid_sigs = 0;
            for (i, binding) in cert.userids().enumerate() {
                for selfsig in binding.self_signatures() {
                    let mut hash = selfsig.hash_algo().context().unwrap();
                    selfsig.hash_userid_binding(
                        &mut hash,
                        cert.primary_key().key(),
                        binding.userid());
                    let h = hash.into_digest().unwrap();
                    if &h[..2] != selfsig.digest_prefix() {
                        eprintln!("{:?}: {:?} / {:?}",
                                  i, binding.userid(), selfsig);
                        eprintln!("  Hash: {:?}", h);
                    }
                    assert_eq!(&h[..2], selfsig.digest_prefix());
                    userid_sigs += 1;
                }
            }
            let mut ua_sigs = 0;
            for (i, a) in cert.user_attributes().enumerate()
            {
                for selfsig in a.self_signatures() {
                    let mut hash = selfsig.hash_algo().context().unwrap();
                    selfsig.hash_user_attribute_binding(
                        &mut hash,
                        cert.primary_key().key(),
                        a.user_attribute());
                    let h = hash.into_digest().unwrap();
                    if &h[..2] != selfsig.digest_prefix() {
                        eprintln!("{:?}: {:?} / {:?}",
                                  i, a.user_attribute(), selfsig);
                        eprintln!("  Hash: {:?}", h);
                    }
                    assert_eq!(&h[..2], selfsig.digest_prefix());
                    ua_sigs += 1;
                }
            }
            let mut subkey_sigs = 0;
            for (i, binding) in cert.subkeys().enumerate() {
                for selfsig in binding.self_signatures() {
                    let mut hash = selfsig.hash_algo().context().unwrap();
                    selfsig.hash_subkey_binding(
                        &mut hash,
                        cert.primary_key().key(),
                        binding.key());
                    let h = hash.into_digest().unwrap();
                    if &h[..2] != selfsig.digest_prefix() {
                        eprintln!("{:?}: {:?}", i, binding);
                        eprintln!("  Hash: {:?}", h);
                    }
                    assert_eq!(h[0], selfsig.digest_prefix()[0]);
                    assert_eq!(h[1], selfsig.digest_prefix()[1]);
                    subkey_sigs += 1;
                }
            }

            (userid_sigs, ua_sigs, subkey_sigs)
        }

        check(Cert::from_bytes(crate::tests::key("hash-algos/MD5.gpg")).unwrap());
        check(Cert::from_bytes(crate::tests::key("hash-algos/RipeMD160.gpg")).unwrap());
        check(Cert::from_bytes(crate::tests::key("hash-algos/SHA1.gpg")).unwrap());
        check(Cert::from_bytes(crate::tests::key("hash-algos/SHA224.gpg")).unwrap());
        check(Cert::from_bytes(crate::tests::key("hash-algos/SHA256.gpg")).unwrap());
        check(Cert::from_bytes(crate::tests::key("hash-algos/SHA384.gpg")).unwrap());
        check(Cert::from_bytes(crate::tests::key("hash-algos/SHA512.gpg")).unwrap());
        check(Cert::from_bytes(crate::tests::key("bannon-all-uids-subkeys.gpg")).unwrap());
        let (_userid_sigs, ua_sigs, _subkey_sigs)
            = check(Cert::from_bytes(crate::tests::key("dkg.gpg")).unwrap());
        assert!(ua_sigs > 0);
    }
}
