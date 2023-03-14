//! Streaming decryption and verification.
//!
//! This module provides convenient filters for decryption and
//! verification of OpenPGP messages (see [Section 11.3 of RFC 4880]).
//! It is the preferred interface to process OpenPGP messages:
//!
//!   [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3
//!
//!   - Use the [`Verifier`] to verify a signed message,
//!   - [`DetachedVerifier`] to verify a detached signature,
//!   - or [`Decryptor`] to decrypt and verify an encrypted and
//!     possibly signed message.
//!
//!
//! Consuming OpenPGP messages is more difficult than producing them.
//! When we produce the message, we control the packet structure being
//! generated using our programs control flow.  However, when we
//! consume a message, the control flow is determined by the message
//! being processed.
//!
//! To use Sequoia's streaming [`Verifier`] and [`Decryptor`], you
//! need to provide an object that implements [`VerificationHelper`],
//! and for the [`Decryptor`] also [`DecryptionHelper`].
//!
//!
//! The [`VerificationHelper`] trait give certificates for the
//! signature verification to the [`Verifier`] or [`Decryptor`], let
//! you inspect the message structure (see [Section 11.3 of RFC
//! 4880]), and implements the signature verification policy.
//!
//! The [`DecryptionHelper`] trait is concerned with producing the
//! session key to decrypt a message, most commonly by decrypting one
//! of the messages' [`PKESK`] or [`SKESK`] packets.  It could also
//! use a cached session key, or one that has been explicitly provided
//! to the decryption operation.
//!
//!   [`PKESK`]: crate::packet::PKESK
//!   [`SKESK`]: crate::packet::SKESK
//!
//! The [`Verifier`] and [`Decryptor`] are filters: they consume
//! OpenPGP data from a reader, file, or bytes, and implement
//! [`io::Read`] that can be used to read the verified and/or
//! decrypted data.
//!
//!   [`io::Read`]: std::io::Read
//!
//! [`DetachedVerifier`] does not provide the [`io::Read`] interface,
//! because in this case, the data to be verified is easily available
//! without any transformation.  Not providing a filter-like interface
//! allows for a very performant implementation of the verification.
//!
//! # Examples
//!
//! This example demonstrates how to use the streaming interface using
//! the [`Verifier`].  For brevity, no certificates are fed to the
//! verifier, and the message structure is not verified, i.e. this
//! merely extracts the literal data.  See the [`Verifier` examples]
//! and the [`Decryptor` examples] for how to verify the message and
//! its structure.
//!
//!   [`Verifier` examples]: Verifier#examples
//!   [`Decryptor` examples]: Decryptor#examples
//!
//! ```
//! # fn main() -> sequoia_openpgp::Result<()> {
//! use std::io::Read;
//! use sequoia_openpgp as openpgp;
//! use openpgp::{KeyHandle, Cert, Result};
//! use openpgp::parse::{Parse, stream::*};
//! use openpgp::policy::StandardPolicy;
//!
//! let p = &StandardPolicy::new();
//!
//! // This fetches keys and computes the validity of the verification.
//! struct Helper {};
//! impl VerificationHelper for Helper {
//!     fn get_certs(&mut self, _ids: &[KeyHandle]) -> Result<Vec<Cert>> {
//!         Ok(Vec::new()) // Feed the Certs to the verifier here...
//!     }
//!     fn check(&mut self, structure: MessageStructure) -> Result<()> {
//!         Ok(()) // Implement your verification policy here.
//!     }
//! }
//!
//! let message =
//!    b"-----BEGIN PGP MESSAGE-----
//!
//!      xA0DAAoWBpwMNI3YLBkByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoAJwWCW37P
//!      8RahBI6MM/pGJjN5dtl5eAacDDSN2CwZCZAGnAw0jdgsGQAAeZQA/2amPbBXT96Q
//!      O7PFms9DRuehsVVrFkaDtjN2WSxI4RGvAQDq/pzNdCMpy/Yo7AZNqZv5qNMtDdhE
//!      b2WH5lghfKe/AQ==
//!      =DjuO
//!      -----END PGP MESSAGE-----";
//!
//! let h = Helper {};
//! let mut v = VerifierBuilder::from_bytes(&message[..])?
//!     .with_policy(p, None, h)?;
//!
//! let mut content = Vec::new();
//! v.read_to_end(&mut content)?;
//! assert_eq!(content, b"Hello World!");
//! # Ok(()) }
//! ```
use std::cmp;
use std::io;
use std::path::Path;
use std::time;

use buffered_reader::BufferedReader;
use crate::{
    Error,
    Fingerprint,
    types::{
        AEADAlgorithm,
        CompressionAlgorithm,
        RevocationStatus,
        SymmetricAlgorithm,
    },
    packet::{
        key,
        OnePassSig,
        PKESK,
        SKESK,
    },
    KeyHandle,
    Packet,
    Result,
    packet,
    packet::Signature,
    cert::prelude::*,
    crypto::SessionKey,
    policy::Policy,
};
use crate::parse::{
    Cookie,
    HashingMode,
    PacketParser,
    PacketParserBuilder,
    PacketParserResult,
    Parse,
};

/// Whether to trace execution by default (on stderr).
const TRACE : bool = false;

/// Indentation level for tracing in this module.
const TRACE_INDENT: isize = 5;

/// How much data to buffer before giving it to the caller.
///
/// Signature verification and detection of ciphertext tampering
/// requires processing the whole message first.  Therefore, OpenPGP
/// implementations supporting streaming operations necessarily must
/// output unverified data.  This has been a source of problems in the
/// past.  To alleviate this, we buffer the message first (up to 25
/// megabytes of net message data by default), and verify the
/// signatures if the message fits into our buffer.  Nevertheless it
/// is important to treat the data as unverified and untrustworthy
/// until you have seen a positive verification.
///
/// The default can be changed using [`VerifierBuilder::buffer_size`]
/// and [`DecryptorBuilder::buffer_size`].
///
///   [`VerifierBuilder::buffer_size`]: VerifierBuilder::buffer_size()
///   [`DecryptorBuilder::buffer_size`]: DecryptorBuilder::buffer_size()
pub const DEFAULT_BUFFER_SIZE: usize = 25 * 1024 * 1024;

/// Result of a signature verification.
///
/// A signature verification is either successful yielding a
/// [`GoodChecksum`], or there was some [`VerificationError`]
/// explaining the verification failure.
///
pub type VerificationResult<'a> =
    std::result::Result<GoodChecksum<'a>, VerificationError<'a>>;

/// A good signature.
///
/// Represents the result of a successful signature verification.  It
/// includes the signature and the signing key with all the necessary
/// context (i.e. certificate, time, policy) to evaluate the
/// trustworthiness of the signature using a trust model.
///
/// `GoodChecksum` is used in [`VerificationResult`].  See also
/// [`VerificationError`].
///
///
/// A signature is considered good if and only if all of the following
/// conditions are met:
///
///   - The signature has a Signature Creation Time subpacket.
///
///   - The signature is alive at the specified time (the time
///     parameter passed to, e.g., [`VerifierBuilder::with_policy`]).
///
///       [`VerifierBuilder::with_policy`]: VerifierBuilder::with_policy()
///
///   - The certificate is alive and not revoked as of the signature's
///     creation time.
///
///   - The signing key is alive, not revoked, and signing capable as
///     of the signature's creation time.
///
///   - The signature was generated by the signing key.
///
/// **Note**: This doesn't mean that the key that generated the
/// signature is in anyway trustworthy in the sense that it
/// belongs to the person or entity that the user thinks it
/// belongs to.  This property can only be evaluated within a
/// trust model, such as the [web of trust] (WoT).  This policy is
/// normally implemented in the [`VerificationHelper::check`]
/// method.
///
///   [web of trust]: https://en.wikipedia.org/wiki/Web_of_trust
#[derive(Debug)]
pub struct GoodChecksum<'a> {
    /// The signature.
    pub sig: &'a Signature,

    /// The signing key that made the signature.
    ///
    /// The amalgamation of the signing key includes the necessary
    /// context (i.e. certificate, time, policy) to evaluate the
    /// trustworthiness of the signature using a trust model.
    pub ka: ValidErasedKeyAmalgamation<'a, key::PublicParts>,
}
assert_send_and_sync!(GoodChecksum<'_>);

/// A bad signature.
///
/// Represents the result of an unsuccessful signature verification.
/// It contains all the context that could be gathered until the
/// verification process failed.
///
/// `VerificationError` is used in [`VerificationResult`].  See also
/// [`GoodChecksum`].
///
///
/// You can either explicitly match on the variants, or convert to
/// [`Error`] using [`From`].
///
///   [`Error`]: super::super::Error
///   [`From`]: std::convert::From
#[derive(Debug)]
pub enum VerificationError<'a> {
    /// Malformed signature (no signature creation subpacket, etc.)
    MalformedSignature {
        /// The signature.
        sig: &'a Signature,

        /// The reason why the signature is malformed.
        error: anyhow::Error,
    },
    /// Missing Key
    MissingKey {
        /// The signature.
        sig: &'a Signature,
    },
    /// Unbound key.
    ///
    /// There is no valid binding signature at the time the signature
    /// was created under the given policy.
    UnboundKey {
        /// The signature.
        sig: &'a Signature,

        /// The certificate that made the signature.
        cert: &'a Cert,

        /// The reason why the key is not bound.
        error: anyhow::Error,
    },
    /// Bad key (have a key, but it is not alive, etc.)
    BadKey {
        /// The signature.
        sig: &'a Signature,

        /// The signing key that made the signature.
        ka: ValidErasedKeyAmalgamation<'a, key::PublicParts>,

        /// The reason why the key is bad.
        error: anyhow::Error,
    },
    /// Bad signature (have a valid key, but the signature didn't check out)
    BadSignature {
        /// The signature.
        sig: &'a Signature,

        /// The signing key that made the signature.
        ka: ValidErasedKeyAmalgamation<'a, key::PublicParts>,

        /// The reason why the signature is bad.
        error: anyhow::Error,
    },
}
assert_send_and_sync!(VerificationError<'_>);

impl<'a> std::fmt::Display for VerificationError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use self::VerificationError::*;
        match self {
            MalformedSignature { error, .. } =>
                write!(f, "Malformed signature: {}", error),
            MissingKey { sig } =>
                if let Some(issuer) = sig.get_issuers().get(0) {
                    write!(f, "Missing key: {}", issuer)
                } else {
                    write!(f, "Missing key")
                },
            UnboundKey { cert, error, .. } =>
                write!(f, "Subkey of {} not bound: {}", cert, error),
            BadKey { ka, error, .. } =>
                write!(f, "Subkey of {} is bad: {}", ka.cert(), error),
            BadSignature { error, .. } =>
                write!(f, "Bad signature: {}", error),
        }
    }
}

impl<'a> From<VerificationError<'a>> for Error {
    fn from(e: VerificationError<'a>) -> Self {
        use self::VerificationError::*;
        match e {
            MalformedSignature { .. } =>
                Error::MalformedPacket(e.to_string()),
            MissingKey { .. } =>
                Error::InvalidKey(e.to_string()),
            UnboundKey { .. } =>
                Error::InvalidKey(e.to_string()),
            BadKey { .. } =>
                Error::InvalidKey(e.to_string()),
            BadSignature { .. } =>
                Error::BadSignature(e.to_string()),
        }
    }
}

/// Like VerificationError, but without referencing the signature.
///
/// This avoids borrowing the signature, so that we can continue to
/// mutably borrow the signature trying other keys.  After all keys
/// are tried, we attach the reference to the signature, yielding a
/// `VerificationError`.
enum VerificationErrorInternal<'a> {
    // MalformedSignature is not used, so it is omitted here.

    /// Missing Key
    MissingKey {
    },
    /// Unbound key.
    ///
    /// There is no valid binding signature at the time the signature
    /// was created under the given policy.
    UnboundKey {
        /// The certificate that made the signature.
        cert: &'a Cert,

        /// The reason why the key is not bound.
        error: anyhow::Error,
    },
    /// Bad key (have a key, but it is not alive, etc.)
    BadKey {
        /// The signing key that made the signature.
        ka: ValidErasedKeyAmalgamation<'a, key::PublicParts>,

        /// The reason why the key is bad.
        error: anyhow::Error,
    },
    /// Bad signature (have a valid key, but the signature didn't check out)
    BadSignature {
        /// The signing key that made the signature.
        ka: ValidErasedKeyAmalgamation<'a, key::PublicParts>,

        /// The reason why the signature is bad.
        error: anyhow::Error,
    },
}

impl<'a> VerificationErrorInternal<'a> {
    fn attach_sig(self, sig: &'a Signature) -> VerificationError<'a> {
        use self::VerificationErrorInternal::*;
        match self {
            MissingKey {} =>
                VerificationError::MissingKey { sig },
            UnboundKey { cert, error } =>
                VerificationError::UnboundKey { sig, cert, error },
            BadKey { ka, error } =>
                VerificationError::BadKey { sig, ka, error },
            BadSignature { ka, error } =>
                VerificationError::BadSignature { sig, ka, error },
        }
    }
}

/// Communicates the message structure to the VerificationHelper.
///
/// A valid OpenPGP message contains one literal data packet with
/// optional [encryption, signing, and compression layers] freely
/// combined on top.  This structure is passed to
/// [`VerificationHelper::check`] for verification.
///
///  [encryption, signing, and compression layers]: MessageLayer
///
/// The most common structure is an optionally encrypted, optionally
/// compressed, and optionally signed message, i.e. if the message is
/// encrypted, then the encryption is the outermost layer; if the
/// message is signed, then the signature group is the innermost
/// layer.  This is a sketch of such a message:
///
/// ```text
/// [ encryption layer: [ compression layer: [ signature group: [ literal data ]]]]
/// ```
///
/// However, OpenPGP allows encryption, signing, and compression
/// operations to be freely combined (see [Section 11.3 of RFC 4880]).
/// This is represented as a stack of [`MessageLayer`]s, where
/// signatures of the same level (i.e. those over the same data:
/// either directly over the literal data, or over other signatures
/// and the literal data) are grouped into one layer.  See also
/// [`Signature::level`].
///
///   [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3
///   [`Signature::level`]: crate::packet::Signature#method.level
///
/// Consider the following structure.  This is a set of notarizing
/// signatures *N* over a set of signatures *S* over the literal data:
///
/// ```text
/// [ signature group: [ signature group: [ literal data ]]]
/// ```
///
/// The notarizing signatures *N* are said to be of level 1,
/// i.e. signatures over the signatures *S* and the literal data.  The
/// signatures *S* are level 0 signatures, i.e. signatures over the
/// literal data.
///
/// OpenPGP's flexibility allows adaption to new use cases, but also
/// presents a challenge to implementations and downstream users.  The
/// message structure must be both validated, and possibly
/// communicated to the application's user.  Note that if
/// compatibility is a concern, generated messages must be restricted
/// to a narrow subset of possible structures, see this [test of
/// unusual message structures].
///
///   [test of unusual message structures]: https://tests.sequoia-pgp.org/#Unusual_Message_Structure
#[derive(Debug)]
pub struct MessageStructure<'a>(Vec<MessageLayer<'a>>);
assert_send_and_sync!(MessageStructure<'_>);

impl<'a> MessageStructure<'a> {
    fn new() -> Self {
        MessageStructure(Vec::new())
    }

    fn new_compression_layer(&mut self, algo: CompressionAlgorithm) {
        self.0.push(MessageLayer::Compression {
            algo,
        })
    }

    fn new_encryption_layer(&mut self, sym_algo: SymmetricAlgorithm,
                            aead_algo: Option<AEADAlgorithm>) {
        self.0.push(MessageLayer::Encryption {
            sym_algo,
            aead_algo,
        })
    }

    fn new_signature_group(&mut self) {
        self.0.push(MessageLayer::SignatureGroup {
            results: Vec::new(),
        })
    }

    fn push_verification_result(&mut self, sig: VerificationResult<'a>) {
        if let Some(MessageLayer::SignatureGroup { ref mut results }) =
            self.0.iter_mut().last()
        {
            results.push(sig);
        } else {
            panic!("cannot push to encryption or compression layer");
        }
    }
}

impl<'a> std::ops::Deref for MessageStructure<'a> {
    type Target = [MessageLayer<'a>];

    fn deref(&self) -> &Self::Target {
        &self.0[..]
    }
}

impl<'a> IntoIterator for MessageStructure<'a> {
    type Item = MessageLayer<'a>;
    type IntoIter = std::vec::IntoIter<MessageLayer<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Represents a layer of the message structure.
///
/// A valid OpenPGP message contains one literal data packet with
/// optional encryption, signing, and compression layers freely
/// combined on top (see [Section 11.3 of RFC 4880]).  This enum
/// represents the layers.  The [`MessageStructure`] is communicated
/// to the [`VerificationHelper::check`].  Iterating over the
/// [`MessageStructure`] yields the individual message layers.
///
///   [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3
#[derive(Debug)]
pub enum MessageLayer<'a> {
    /// Represents an compression container.
    ///
    /// Compression is usually transparent in OpenPGP, though it may
    /// sometimes be interesting for advanced users to indicate that
    /// the message was compressed, and how (see [Section 5.6 of RFC
    /// 4880]).
    ///
    ///   [Section 5.6 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.6
    Compression {
        /// Compression algorithm used.
        algo: CompressionAlgorithm,
    },
    /// Represents an encryption container.
    ///
    /// Indicates the fact that the message was encrypted (see
    /// [Section 5.13 of RFC 4880]).  If you expect encrypted
    /// messages, make sure that there is at least one encryption
    /// container present.
    ///
    ///   [Section 5.13 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.13
    Encryption {
        /// Symmetric algorithm used.
        sym_algo: SymmetricAlgorithm,
        /// AEAD algorithm used, if any.
        ///
        /// This feature is [experimental](super::super#experimental-features).
        aead_algo: Option<AEADAlgorithm>,
    },
    /// Represents a signature group.
    ///
    /// A signature group consists of all signatures with the same
    /// level (see [Section 5.2 of RFC 4880]).  Each
    /// [`VerificationResult`] represents the result of a single
    /// signature verification.  In your [`VerificationHelper::check`]
    /// method, iterate over the verification results, see if it meets
    /// your policies' demands, and communicate it to the user, if
    /// applicable.
    ///
    ///   [Section 5.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2
    SignatureGroup {
        /// The results of the signature verifications.
        results: Vec<VerificationResult<'a>>,
    }
}
assert_send_and_sync!(MessageLayer<'_>);

/// Internal version of the message structure.
///
/// In contrast to MessageStructure, this owns unverified
/// signature packets.
#[derive(Debug)]
struct IMessageStructure {
    layers: Vec<IMessageLayer>,

    // We insert a SignatureGroup layer every time we see a OnePassSig
    // packet with the last flag.
    //
    // However, we need to make sure that we insert a SignatureGroup
    // layer even if the OnePassSig packet has the last flag set to
    // false.  To do that, we keep track of the fact that we saw such
    // a OPS packet.
    sig_group_counter: usize,
}

impl IMessageStructure {
    fn new() -> Self {
        IMessageStructure {
            layers: Vec::new(),
            sig_group_counter: 0,
        }
    }

    fn new_compression_layer(&mut self, algo: CompressionAlgorithm) {
        self.insert_missing_signature_group();
        self.layers.push(IMessageLayer::Compression {
            algo,
        });
    }

    fn new_encryption_layer(&mut self,
                            depth: isize,
                            expect_mdc: bool,
                            sym_algo: SymmetricAlgorithm,
                            aead_algo: Option<AEADAlgorithm>) {
        self.insert_missing_signature_group();
        self.layers.push(IMessageLayer::Encryption {
            depth,
            expect_mdc,
            sym_algo,
            aead_algo,
        });
    }

    /// Returns whether or not we expect an MDC packet in an
    /// encryption container at this recursion depth.
    ///
    /// Handling MDC packets has to be done carefully, otherwise, we
    /// may create a decryption oracle.
    fn expect_mdc_at(&self, at: isize) -> bool {
        for l in &self.layers {
            match l {
                IMessageLayer::Encryption {
                    depth,
                    expect_mdc,
                    ..
                } if *depth == at && *expect_mdc => return true,
                _ => (),
            }
        }
        false
    }

    /// Makes sure that we insert a signature group even if the
    /// previous OPS packet had the last flag set to false.
    fn insert_missing_signature_group(&mut self) {
        if self.sig_group_counter > 0 {
            self.layers.push(IMessageLayer::SignatureGroup {
                sigs: Vec::new(),
                count: self.sig_group_counter,
            });
        }
        self.sig_group_counter = 0;
    }

    fn push_ops(&mut self, ops: &OnePassSig) {
        self.sig_group_counter += 1;
        if ops.last() {
            self.layers.push(IMessageLayer::SignatureGroup {
                sigs: Vec::new(),
                count: self.sig_group_counter,
            });
            self.sig_group_counter = 0;
        }
    }

    fn push_signature(&mut self, sig: Signature, csf_message: bool) {
        for layer in self.layers.iter_mut().rev() {
            match layer {
                IMessageLayer::SignatureGroup {
                    ref mut sigs, ref mut count,
                } if *count > 0 => {
                    sigs.push(sig);
                    if csf_message {
                        // The CSF transformation does not know how
                        // many signatures will follow, so we may end
                        // up with too few synthesized OPS packets.
                        // But, we only have one layer anyway, and no
                        // notarizations, so we don't need to concern
                        // ourself with the counter.
                    } else {
                        *count -= 1;
                    }
                    return;
                },
                _ => (),
            }
        }
        panic!("signature unaccounted for");
    }

    fn push_bare_signature(&mut self, sig: Signature) {
        if let Some(IMessageLayer::SignatureGroup { .. }) = self.layers.iter().last() {
            // The last layer is a SignatureGroup.  We will append the
            // signature there without accounting for it.
        } else {
            // The last layer is not a SignatureGroup, or there is no
            // layer at all.  Create one.
            self.layers.push(IMessageLayer::SignatureGroup {
                sigs: Vec::new(),
                count: 0,
            });
        }

        if let IMessageLayer::SignatureGroup { ref mut sigs, .. } =
            self.layers.iter_mut().last().expect("just checked or created")
        {
            sigs.push(sig);
        } else {
            unreachable!("just checked or created")
        }
    }

}

/// Internal version of a layer of the message structure.
///
/// In contrast to MessageLayer, this owns unverified signature packets.
#[derive(Debug)]
enum IMessageLayer {
    Compression {
        algo: CompressionAlgorithm,
    },
    Encryption {
        /// Recursion depth of this container.
        depth: isize,
        /// Do we expect an MDC packet?
        ///
        /// I.e. is this a SEIPv1 container?
        expect_mdc: bool,
        sym_algo: SymmetricAlgorithm,
        aead_algo: Option<AEADAlgorithm>,
    },
    SignatureGroup {
        sigs: Vec<Signature>,
        count: usize,
    }
}

/// Helper for signature verification.
///
/// This trait abstracts over signature and message structure
/// verification.  It allows us to provide the [`Verifier`],
/// [`DetachedVerifier`], and [`Decryptor`] without imposing a policy
/// on how certificates for signature verification are looked up, or
/// what message structure is considered acceptable.
///
///
/// It also allows you to inspect each packet that is processed during
/// verification or decryption, optionally providing a [`Map`] for
/// each packet.
///
///   [`Map`]: super::map::Map
pub trait VerificationHelper {
    /// Inspects the message.
    ///
    /// Called once per packet.  Can be used to inspect and dump
    /// packets in encrypted messages.
    ///
    /// The default implementation does nothing.
    fn inspect(&mut self, pp: &PacketParser) -> Result<()> {
        // Do nothing.
        let _ = pp;
        Ok(())
    }

    /// Retrieves the certificates containing the specified keys.
    ///
    /// When implementing this method, you should return as many
    /// certificates corresponding to the `ids` as you can.
    ///
    /// If an identifier is ambiguous, because, for instance, there
    /// are multiple certificates with the same Key ID, then you
    /// should return all of them.
    ///
    /// You should only return an error if processing should be
    /// aborted.  In general, you shouldn't return an error if you
    /// don't have a certificate for a given identifier: if there are
    /// multiple signatures, then, depending on your policy, verifying
    /// a subset of them may be sufficient.
    ///
    /// This method will be called at most once per message.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to look up the certificates for
    /// the signature verification given the list of signature
    /// issuers.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::stream::*;
    /// # fn lookup_cert_by_handle(_: &KeyHandle) -> Result<Cert> {
    /// #     unimplemented!()
    /// # }
    ///
    /// struct Helper { /* ... */ };
    /// impl VerificationHelper for Helper {
    ///     fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    ///         let mut certs = Vec::new();
    ///         for id in ids {
    ///             certs.push(lookup_cert_by_handle(id)?);
    ///         }
    ///         Ok(certs)
    ///     }
    ///     // ...
    /// #    fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #        unimplemented!()
    /// #    }
    /// }
    /// ```
    fn get_certs(&mut self, ids: &[crate::KeyHandle]) -> Result<Vec<Cert>>;

    /// Validates the message structure.
    ///
    /// This function must validate the message's structure according
    /// to an application specific policy.  For example, it could
    /// check that the required number of signatures or notarizations
    /// were confirmed as good, and evaluate every signature's
    /// validity under an trust model.
    ///
    /// A valid OpenPGP message contains one literal data packet with
    /// optional encryption, signing, and compression layers on top.
    /// Notably, the message structure contains the results of
    /// signature verifications.  See [`MessageStructure`] for more
    /// information.
    ///
    ///
    /// When verifying a message, this callback will be called exactly
    /// once per message *after* the last signature has been verified
    /// and *before* all of the data has been returned.  Any error
    /// returned by this function will abort reading, and the error
    /// will be propagated via the [`io::Read`] operation.
    ///
    ///   [`io::Read`]: std::io::Read
    ///
    /// After this method was called, [`Verifier::message_processed`]
    /// and [`Decryptor::message_processed`] return `true`.
    ///
    ///   [`Verifier::message_processed`]: Verifier::message_processed()
    ///   [`Decryptor::message_processed`]: Decryptor::message_processed()
    ///
    /// When verifying a detached signature using the
    /// [`DetachedVerifier`], this method will be called with a
    /// [`MessageStructure`] containing exactly one layer, a signature
    /// group.
    ///
    ///
    /// # Examples
    ///
    /// This example demonstrates how to verify that the message is an
    /// encrypted, optionally compressed, and signed message that has
    /// at least one valid signature.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::stream::*;
    ///
    /// struct Helper { /* ... */ };
    /// impl VerificationHelper for Helper {
    /// #    fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #        unimplemented!();
    /// #    }
    ///     fn check(&mut self, structure: MessageStructure) -> Result<()> {
    ///         for (i, layer) in structure.into_iter().enumerate() {
    ///             match layer {
    ///                 MessageLayer::Encryption { .. } if i == 0 => (),
    ///                 MessageLayer::Compression { .. } if i == 1 => (),
    ///                 MessageLayer::SignatureGroup { ref results }
    ///                     if i == 1 || i == 2 =>
    ///                 {
    ///                     if ! results.iter().any(|r| r.is_ok()) {
    ///                         return Err(anyhow::anyhow!(
    ///                                        "No valid signature"));
    ///                     }
    ///                 }
    ///                 _ => return Err(anyhow::anyhow!(
    ///                                     "Unexpected message structure")),
    ///             }
    ///         }
    ///         Ok(())
    ///     }
    ///     // ...
    /// }
    /// ```
    fn check(&mut self, structure: MessageStructure) -> Result<()>;
}

/// Wraps a VerificationHelper and adds a non-functional
/// DecryptionHelper implementation.
struct NoDecryptionHelper<V: VerificationHelper> {
    v: V,
}

impl<V: VerificationHelper> VerificationHelper for NoDecryptionHelper<V> {
    fn get_certs(&mut self, ids: &[crate::KeyHandle]) -> Result<Vec<Cert>>
    {
        self.v.get_certs(ids)
    }
    fn check(&mut self, structure: MessageStructure) -> Result<()>
    {
        self.v.check(structure)
    }
    fn inspect(&mut self, pp: &PacketParser) -> Result<()> {
        self.v.inspect(pp)
    }
}

impl<V: VerificationHelper> DecryptionHelper for NoDecryptionHelper<V> {
    fn decrypt<D>(&mut self, _: &[PKESK], _: &[SKESK],
                  _: Option<SymmetricAlgorithm>,
                  _: D) -> Result<Option<Fingerprint>>
        where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
    {
        unreachable!("This is not used for verifications")
    }
}

/// Verifies a signed OpenPGP message.
///
/// To create a `Verifier`, create a [`VerifierBuilder`] using
/// [`Parse`], and customize it to your needs.
///
///   [`Parse`]: super::Parse
///
/// Signature verification requires processing the whole message
/// first.  Therefore, OpenPGP implementations supporting streaming
/// operations necessarily must output unverified data.  This has been
/// a source of problems in the past.  To alleviate this, we buffer
/// the message first (up to 25 megabytes of net message data by
/// default, see [`DEFAULT_BUFFER_SIZE`]), and verify the signatures
/// if the message fits into our buffer.  Nevertheless it is important
/// to treat the data as unverified and untrustworthy until you have
/// seen a positive verification.  See [`Verifier::message_processed`]
/// for more information.
///
///   [`Verifier::message_processed`]: Verifier::message_processed()
///
/// See [`GoodChecksum`] for what it means for a signature to be
/// considered valid.
///
///
/// # Examples
///
/// ```
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use std::io::Read;
/// use sequoia_openpgp as openpgp;
/// use openpgp::{KeyHandle, Cert, Result};
/// use openpgp::parse::{Parse, stream::*};
/// use openpgp::policy::StandardPolicy;
/// # fn lookup_cert_by_handle(_: &KeyHandle) -> Result<Cert> {
/// #     Cert::from_bytes(
/// #       &b"-----BEGIN PGP PUBLIC KEY BLOCK-----
/// #
/// #          xjMEWlNvABYJKwYBBAHaRw8BAQdA+EC2pvebpEbzPA9YplVgVXzkIG5eK+7wEAez
/// #          lcBgLJrNMVRlc3R5IE1jVGVzdGZhY2UgKG15IG5ldyBrZXkpIDx0ZXN0eUBleGFt
/// #          cGxlLm9yZz7CkAQTFggAOBYhBDnRAKtn1b2MBAECBfs3UfFYfa7xBQJaU28AAhsD
/// #          BQsJCAcCBhUICQoLAgQWAgMBAh4BAheAAAoJEPs3UfFYfa7xJHQBAO4/GABMWUcJ
/// #          5D/DZ9b+6YiFnysSjCT/gILJgxMgl7uoAPwJherI1pAAh49RnPHBR1IkWDtwzX65
/// #          CJG8sDyO2FhzDs44BFpTbwASCisGAQQBl1UBBQEBB0B+A0GRHuBgdDX50T1nePjb
/// #          mKQ5PeqXJbWEtVrUtVJaPwMBCAfCeAQYFggAIBYhBDnRAKtn1b2MBAECBfs3UfFY
/// #          fa7xBQJaU28AAhsMAAoJEPs3UfFYfa7xzjIBANX2/FgDX3WkmvwpEHg/sn40zACM
/// #          W2hrBY5x0sZ8H7JlAP47mCfCuRVBqyaePuzKbxLJeLe2BpDdc0n2izMVj8t9Cg==
/// #          =QetZ
/// #          -----END PGP PUBLIC KEY BLOCK-----"[..])
/// # }
///
/// let p = &StandardPolicy::new();
///
/// // This fetches keys and computes the validity of the verification.
/// struct Helper {};
/// impl VerificationHelper for Helper {
///     fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
///         let mut certs = Vec::new();
///         for id in ids {
///             certs.push(lookup_cert_by_handle(id)?);
///         }
///         Ok(certs)
///     }
///
///     fn check(&mut self, structure: MessageStructure) -> Result<()> {
///         for (i, layer) in structure.into_iter().enumerate() {
///             match layer {
///                 MessageLayer::Encryption { .. } if i == 0 => (),
///                 MessageLayer::Compression { .. } if i == 1 => (),
///                 MessageLayer::SignatureGroup { ref results } => {
///                     if ! results.iter().any(|r| r.is_ok()) {
///                         return Err(anyhow::anyhow!(
///                                        "No valid signature"));
///                     }
///                 }
///                 _ => return Err(anyhow::anyhow!(
///                                     "Unexpected message structure")),
///             }
///         }
///         Ok(())
///     }
/// }
///
/// let message =
///    b"-----BEGIN PGP MESSAGE-----
///
///      xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
///      AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
///      UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
///      YmAFv/UfO0vYBw==
///      =+l94
///      -----END PGP MESSAGE-----
///      ";
///
/// let h = Helper {};
/// let mut v = VerifierBuilder::from_bytes(&message[..])?
///     .with_policy(p, None, h)?;
///
/// let mut content = Vec::new();
/// v.read_to_end(&mut content)?;
/// assert_eq!(content, b"Hello World!");
/// # Ok(()) }
pub struct Verifier<'a, H: VerificationHelper> {
    decryptor: Decryptor<'a, NoDecryptionHelper<H>>,
}
assert_send_and_sync!(Verifier<'_, H> where H: VerificationHelper);

/// A builder for `Verifier`.
///
/// This allows the customization of [`Verifier`], which can
/// be built using [`VerifierBuilder::with_policy`].
///
///   [`VerifierBuilder::with_policy`]: VerifierBuilder::with_policy()
pub struct VerifierBuilder<'a> {
    message: Box<dyn BufferedReader<Cookie> + 'a>,
    buffer_size: usize,
    mapping: bool,
}
assert_send_and_sync!(VerifierBuilder<'_>);

impl<'a> Parse<'a, VerifierBuilder<'a>>
    for VerifierBuilder<'a>
{
    fn from_reader<R>(reader: R) -> Result<VerifierBuilder<'a>>
        where R: io::Read + 'a + Send + Sync,
    {
        VerifierBuilder::new(buffered_reader::Generic::with_cookie(
            reader, None, Default::default()))
    }

    fn from_file<P>(path: P) -> Result<VerifierBuilder<'a>>
        where P: AsRef<Path>,
    {
        VerifierBuilder::new(buffered_reader::File::with_cookie(
            path, Default::default())?)
    }

    fn from_bytes<D>(data: &'a D) -> Result<VerifierBuilder<'a>>
        where D: AsRef<[u8]> + ?Sized,
    {
        VerifierBuilder::new(buffered_reader::Memory::with_cookie(
            data.as_ref(), Default::default()))
    }
}

impl<'a> VerifierBuilder<'a> {
    fn new<B>(signatures: B) -> Result<Self>
        where B: buffered_reader::BufferedReader<Cookie> + 'a
    {
        Ok(VerifierBuilder {
            message: Box::new(signatures),
            buffer_size: DEFAULT_BUFFER_SIZE,
            mapping: false,
        })
    }

    /// Changes the amount of buffered data.
    ///
    /// By default, we buffer up to 25 megabytes of net message data
    /// (see [`DEFAULT_BUFFER_SIZE`]).  This changes the default.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    ///
    /// let message =
    ///     // ...
    /// # &b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..];
    ///
    /// let h = Helper {};
    /// let mut v = VerifierBuilder::from_bytes(message)?
    ///     .buffer_size(1 << 12)
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Enables mapping.
    ///
    /// If mapping is enabled, the packet parser will create a [`Map`]
    /// of the packets that can be inspected in
    /// [`VerificationHelper::inspect`].  Note that this buffers the
    /// packets contents, and is not recommended unless you know that
    /// the packets are small.
    ///
    ///   [`Map`]: super::map::Map
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    ///
    /// let message =
    ///     // ...
    /// # &b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..];
    ///
    /// let h = Helper {};
    /// let mut v = VerifierBuilder::from_bytes(message)?
    ///     .mapping(true)
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn mapping(mut self, enabled: bool) -> Self {
        self.mapping = enabled;
        self
    }

    /// Creates the `Verifier`.
    ///
    /// Signature verifications are done under the given `policy` and
    /// relative to time `time`, or the current time, if `time` is
    /// `None`.  `helper` is the [`VerificationHelper`] to use.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    ///
    /// let message =
    ///     // ...
    /// # &b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..];
    ///
    /// let h = Helper {};
    /// let mut v = VerifierBuilder::from_bytes(message)?
    ///     // Customize the `Verifier` here.
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn with_policy<T, H>(self, policy: &'a dyn Policy, time: T, helper: H)
                             -> Result<Verifier<'a, H>>
        where H: VerificationHelper,
              T: Into<Option<time::SystemTime>>,
    {
        // Do not eagerly map `t` to the current time.
        let t = time.into();
        Ok(Verifier {
            decryptor: Decryptor::from_buffered_reader(
                policy,
                self.message,
                NoDecryptionHelper { v: helper, },
                t, Mode::Verify, self.buffer_size, self.mapping, true)?,
        })
    }
}

impl<'a, H: VerificationHelper> Verifier<'a, H> {
    /// Returns a reference to the helper.
    pub fn helper_ref(&self) -> &H {
        &self.decryptor.helper_ref().v
    }

    /// Returns a mutable reference to the helper.
    pub fn helper_mut(&mut self) -> &mut H {
        &mut self.decryptor.helper_mut().v
    }

    /// Recovers the helper.
    pub fn into_helper(self) -> H {
        self.decryptor.into_helper().v
    }

    /// Returns true if the whole message has been processed and
    /// authenticated.
    ///
    /// If the function returns `true`, the whole message has been
    /// processed, the signatures are verified, and the message
    /// structure has been passed to [`VerificationHelper::check`].
    /// Data read from this `Verifier` using [`io::Read`] has been
    /// authenticated.
    ///
    ///   [`io::Read`]: std::io::Read
    ///
    /// If the function returns `false`, the message did not fit into
    /// the internal buffer, and therefore data read from this
    /// `Verifier` using [`io::Read`] has **not yet been
    /// authenticated**.  It is important to treat this data as
    /// attacker controlled and not use it until it has been
    /// authenticated.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to verify a message in a
    /// streaming fashion, writing the data to a temporary file and
    /// only commit the result once the data is authenticated.
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use std::io::{Read, Seek, SeekFrom};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    /// #
    /// # // Mock of `tempfile::tempfile`.
    /// # mod tempfile {
    /// #     pub fn tempfile() -> sequoia_openpgp::Result<std::fs::File> {
    /// #         unimplemented!()
    /// #     }
    /// # }
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// // This fetches keys and computes the validity of the verification.
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #   fn check(&mut self, _: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    ///
    /// let mut source =
    ///    // ...
    /// #  std::io::Cursor::new(&b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..]);
    ///
    /// fn consume(r: &mut dyn Read) -> Result<()> {
    ///    // ...
    /// #   let _ = r; Ok(())
    /// }
    ///
    /// let h = Helper {};
    /// let mut v = VerifierBuilder::from_reader(&mut source)?
    ///     .with_policy(p, None, h)?;
    ///
    /// if v.message_processed() {
    ///     // The data has been authenticated.
    ///     consume(&mut v)?;
    /// } else {
    ///     let mut tmp = tempfile::tempfile()?;
    ///     std::io::copy(&mut v, &mut tmp)?;
    ///
    ///     // If the copy succeeds, the message has been fully
    ///     // processed and the data has been authenticated.
    ///     assert!(v.message_processed());
    ///
    ///     // Rewind and consume.
    ///     tmp.seek(SeekFrom::Start(0))?;
    ///     consume(&mut tmp)?;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn message_processed(&self) -> bool {
        self.decryptor.message_processed()
    }
}

impl<'a, H: VerificationHelper> io::Read for Verifier<'a, H> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.decryptor.read(buf)
    }
}


/// Verifies a detached signature.
///
/// To create a `DetachedVerifier`, create a
/// [`DetachedVerifierBuilder`] using [`Parse`], and customize it to
/// your needs.
///
///   [`Parse`]: super::Parse
///
/// See [`GoodChecksum`] for what it means for a signature to be
/// considered valid.  When the signature(s) are processed,
/// [`VerificationHelper::check`] will be called with a
/// [`MessageStructure`] containing exactly one layer, a signature
/// group.
///
///
/// # Examples
///
/// ```
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use std::io::{self, Read};
/// use sequoia_openpgp as openpgp;
/// use openpgp::{KeyHandle, Cert, Result};
/// use openpgp::parse::{Parse, stream::*};
/// use sequoia_openpgp::policy::StandardPolicy;
///
/// let p = &StandardPolicy::new();
///
/// // This fetches keys and computes the validity of the verification.
/// struct Helper {};
/// impl VerificationHelper for Helper {
///     fn get_certs(&mut self, _ids: &[KeyHandle]) -> Result<Vec<Cert>> {
///         Ok(Vec::new()) // Feed the Certs to the verifier here...
///     }
///     fn check(&mut self, structure: MessageStructure) -> Result<()> {
///         Ok(()) // Implement your verification policy here.
///     }
/// }
///
/// let signature =
///    b"-----BEGIN PGP SIGNATURE-----
///
///      wnUEABYKACcFglt+z/EWoQSOjDP6RiYzeXbZeXgGnAw0jdgsGQmQBpwMNI3YLBkA
///      AHmUAP9mpj2wV0/ekDuzxZrPQ0bnobFVaxZGg7YzdlksSOERrwEA6v6czXQjKcv2
///      KOwGTamb+ajTLQ3YRG9lh+ZYIXynvwE=
///      =IJ29
///      -----END PGP SIGNATURE-----";
///
/// let data = b"Hello World!";
/// let h = Helper {};
/// let mut v = DetachedVerifierBuilder::from_bytes(&signature[..])?
///     .with_policy(p, None, h)?;
/// v.verify_bytes(data)?;
/// # Ok(()) }
pub struct DetachedVerifier<'a, H: VerificationHelper> {
    decryptor: Decryptor<'a, NoDecryptionHelper<H>>,
}
assert_send_and_sync!(DetachedVerifier<'_, H> where H: VerificationHelper);

/// A builder for `DetachedVerifier`.
///
/// This allows the customization of [`DetachedVerifier`], which can
/// be built using [`DetachedVerifierBuilder::with_policy`].
///
///   [`DetachedVerifierBuilder::with_policy`]: DetachedVerifierBuilder::with_policy()
pub struct DetachedVerifierBuilder<'a> {
    signatures: Box<dyn BufferedReader<Cookie> + 'a>,
    mapping: bool,
}
assert_send_and_sync!(DetachedVerifierBuilder<'_>);

impl<'a> Parse<'a, DetachedVerifierBuilder<'a>>
    for DetachedVerifierBuilder<'a>
{
    fn from_reader<R>(reader: R) -> Result<DetachedVerifierBuilder<'a>>
        where R: io::Read + 'a + Send + Sync,
    {
        DetachedVerifierBuilder::new(buffered_reader::Generic::with_cookie(
            reader, None, Default::default()))
    }

    fn from_file<P>(path: P) -> Result<DetachedVerifierBuilder<'a>>
        where P: AsRef<Path>,
    {
        DetachedVerifierBuilder::new(buffered_reader::File::with_cookie(
            path, Default::default())?)
    }

    fn from_bytes<D>(data: &'a D) -> Result<DetachedVerifierBuilder<'a>>
        where D: AsRef<[u8]> + ?Sized,
    {
        DetachedVerifierBuilder::new(buffered_reader::Memory::with_cookie(
            data.as_ref(), Default::default()))
    }
}

impl<'a> DetachedVerifierBuilder<'a> {
    fn new<B>(signatures: B) -> Result<Self>
        where B: buffered_reader::BufferedReader<Cookie> + 'a
    {
        Ok(DetachedVerifierBuilder {
            signatures: Box::new(signatures),
            mapping: false,
        })
    }

    /// Enables mapping.
    ///
    /// If mapping is enabled, the packet parser will create a [`Map`]
    /// of the packets that can be inspected in
    /// [`VerificationHelper::inspect`].  Note that this buffers the
    /// packets contents, and is not recommended unless you know that
    /// the packets are small.
    ///
    ///   [`Map`]: super::map::Map
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    ///
    /// let signature =
    ///     // ...
    /// #  b"-----BEGIN PGP SIGNATURE-----
    /// #
    /// #    wnUEABYKACcFglt+z/EWoQSOjDP6RiYzeXbZeXgGnAw0jdgsGQmQBpwMNI3YLBkA
    /// #    AHmUAP9mpj2wV0/ekDuzxZrPQ0bnobFVaxZGg7YzdlksSOERrwEA6v6czXQjKcv2
    /// #    KOwGTamb+ajTLQ3YRG9lh+ZYIXynvwE=
    /// #    =IJ29
    /// #    -----END PGP SIGNATURE-----";
    ///
    /// let h = Helper {};
    /// let mut v = DetachedVerifierBuilder::from_bytes(&signature[..])?
    ///     .mapping(true)
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn mapping(mut self, enabled: bool) -> Self {
        self.mapping = enabled;
        self
    }

    /// Creates the `DetachedVerifier`.
    ///
    /// Signature verifications are done under the given `policy` and
    /// relative to time `time`, or the current time, if `time` is
    /// `None`.  `helper` is the [`VerificationHelper`] to use.
    /// [`VerificationHelper::check`] will be called with a
    /// [`MessageStructure`] containing exactly one layer, a signature
    /// group.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    ///
    /// let signature =
    ///     // ...
    /// #  b"-----BEGIN PGP SIGNATURE-----
    /// #
    /// #    wnUEABYKACcFglt+z/EWoQSOjDP6RiYzeXbZeXgGnAw0jdgsGQmQBpwMNI3YLBkA
    /// #    AHmUAP9mpj2wV0/ekDuzxZrPQ0bnobFVaxZGg7YzdlksSOERrwEA6v6czXQjKcv2
    /// #    KOwGTamb+ajTLQ3YRG9lh+ZYIXynvwE=
    /// #    =IJ29
    /// #    -----END PGP SIGNATURE-----";
    ///
    /// let h = Helper {};
    /// let mut v = DetachedVerifierBuilder::from_bytes(&signature[..])?
    ///     // Customize the `DetachedVerifier` here.
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn with_policy<T, H>(self, policy: &'a dyn Policy, time: T, helper: H)
                             -> Result<DetachedVerifier<'a, H>>
        where H: VerificationHelper,
              T: Into<Option<time::SystemTime>>,
    {
        // Do not eagerly map `t` to the current time.
        let t = time.into();
        Ok(DetachedVerifier {
            decryptor: Decryptor::from_buffered_reader(
                policy,
                self.signatures,
                NoDecryptionHelper { v: helper, },
                t, Mode::VerifyDetached, 0, self.mapping, false)?,
        })
    }
}

impl<'a, H: VerificationHelper> DetachedVerifier<'a, H> {
    /// Verifies the given data.
    pub fn verify_reader<R: io::Read + Send + Sync>(&mut self, reader: R) -> Result<()> {
        self.verify(buffered_reader::Generic::with_cookie(
            reader, None, Default::default()).as_boxed())
    }

    /// Verifies the given data.
    pub fn verify_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.verify(buffered_reader::File::with_cookie(
            path, Default::default())?.as_boxed())
    }

    /// Verifies the given data.
    pub fn verify_bytes<B: AsRef<[u8]>>(&mut self, buf: B) -> Result<()> {
        self.verify(buffered_reader::Memory::with_cookie(
            buf.as_ref(), Default::default()).as_boxed())
    }

    /// Verifies the given data.
    fn verify<R>(&mut self, reader: R) -> Result<()>
        where R: BufferedReader<Cookie>,
    {
        self.decryptor.verify_detached(reader)
    }

    /// Returns a reference to the helper.
    pub fn helper_ref(&self) -> &H {
        &self.decryptor.helper_ref().v
    }

    /// Returns a mutable reference to the helper.
    pub fn helper_mut(&mut self) -> &mut H {
        &mut self.decryptor.helper_mut().v
    }

    /// Recovers the helper.
    pub fn into_helper(self) -> H {
        self.decryptor.into_helper().v
    }
}


/// Modes of operation for the Decryptor.
#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Decrypt,
    Verify,
    VerifyDetached,
}

/// Decrypts and verifies an encrypted and optionally signed OpenPGP
/// message.
///
/// To create a `Decryptor`, create a [`DecryptorBuilder`] using
/// [`Parse`], and customize it to your needs.
///
///   [`Parse`]: super::Parse
///
/// Signature verification and detection of ciphertext tampering
/// requires processing the whole message first.  Therefore, OpenPGP
/// implementations supporting streaming operations necessarily must
/// output unverified data.  This has been a source of problems in the
/// past.  To alleviate this, we buffer the message first (up to 25
/// megabytes of net message data by default, see
/// [`DEFAULT_BUFFER_SIZE`]), and verify the signatures if the message
/// fits into our buffer.  Nevertheless it is important to treat the
/// data as unverified and untrustworthy until you have seen a
/// positive verification.  See [`Decryptor::message_processed`] for
/// more information.
///
///   [`Decryptor::message_processed`]: Decryptor::message_processed()
///
/// See [`GoodChecksum`] for what it means for a signature to be
/// considered valid.
///
///
/// # Examples
///
/// ```
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use std::io::Read;
/// use sequoia_openpgp as openpgp;
/// use openpgp::crypto::SessionKey;
/// use openpgp::types::SymmetricAlgorithm;
/// use openpgp::{KeyID, Cert, Result, packet::{Key, PKESK, SKESK}};
/// use openpgp::parse::{Parse, stream::*};
/// use sequoia_openpgp::policy::StandardPolicy;
///
/// let p = &StandardPolicy::new();
///
/// // This fetches keys and computes the validity of the verification.
/// struct Helper {};
/// impl VerificationHelper for Helper {
///     fn get_certs(&mut self, _ids: &[openpgp::KeyHandle]) -> Result<Vec<Cert>> {
///         Ok(Vec::new()) // Feed the Certs to the verifier here...
///     }
///     fn check(&mut self, structure: MessageStructure) -> Result<()> {
///         Ok(()) // Implement your verification policy here.
///     }
/// }
/// impl DecryptionHelper for Helper {
///     fn decrypt<D>(&mut self, _: &[PKESK], skesks: &[SKESK],
///                   _sym_algo: Option<SymmetricAlgorithm>,
///                   mut decrypt: D) -> Result<Option<openpgp::Fingerprint>>
///         where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
///     {
///         skesks[0].decrypt(&"streng geheim".into())
///             .map(|(algo, session_key)| decrypt(algo, &session_key));
///         Ok(None)
///     }
/// }
///
/// let message =
///    b"-----BEGIN PGP MESSAGE-----
///
///      wy4ECQMIY5Zs8RerVcXp85UgoUKjKkevNPX3WfcS5eb7rkT9I6kw6N2eEc5PJUDh
///      0j0B9mnPKeIwhp2kBHpLX/en6RfNqYauX9eSeia7aqsd/AOLbO9WMCLZS5d2LTxN
///      rwwb8Aggyukj13Mi0FF5
///      =OB/8
///      -----END PGP MESSAGE-----";
///
/// let h = Helper {};
/// let mut v = DecryptorBuilder::from_bytes(&message[..])?
///     .with_policy(p, None, h)?;
///
/// let mut content = Vec::new();
/// v.read_to_end(&mut content)?;
/// assert_eq!(content, b"Hello World!");
/// # Ok(()) }
pub struct Decryptor<'a, H: VerificationHelper + DecryptionHelper> {
    helper: H,

    /// The issuers collected from OPS and Signature packets.
    issuers: Vec<KeyHandle>,

    /// The certificates used for signature verification.
    certs: Vec<Cert>,

    oppr: Option<PacketParserResult<'a>>,
    identity: Option<Fingerprint>,
    structure: IMessageStructure,

    /// We want to hold back some data until the signatures checked
    /// out.  We buffer this here, cursor is the offset of unread
    /// bytes in the buffer.
    buffer_size: usize,
    reserve: Option<Vec<u8>>,
    cursor: usize,

    /// The mode of operation.
    mode: Mode,

    /// Whether we are actually processing a cleartext signature
    /// framework message.  If so, we need to tweak our behavior a
    /// bit.
    processing_csf_message: Option<bool>,

    /// Signature verification relative to this time.
    ///
    /// This is needed for checking the signature's liveness.
    ///
    /// We want the same semantics as `Subpacket::signature_alive`.
    /// Specifically, when using the current time, we want to tolerate
    /// some clock skew, but when using some specific time, we don't.
    /// (See `Subpacket::signature_alive` for an explanation.)
    ///
    /// These semantics can be realized by making `time` an
    /// `Option<time::SystemTime>` and passing that as is to
    /// `Subpacket::signature_alive`.  But that approach has two new
    /// problems.  First, if we are told to use the current time, then
    /// we want to use the time at which the Verifier was
    /// instantiated, not the time at which we call
    /// `Subpacket::signature_alive`.  Second, if we call
    /// `Subpacket::signature_alive` multiple times, they should all
    /// use the same time.  To work around these issues, when a
    /// Verifier is instantiated, we evaluate `time` and we record how
    /// much we want to tolerate clock skew in the same way as
    /// `Subpacket::signature_alive`.
    time: time::SystemTime,
    clock_skew_tolerance: time::Duration,

    policy: &'a dyn Policy,
}
assert_send_and_sync!(Decryptor<'_, H>
      where H: VerificationHelper + DecryptionHelper);

/// A builder for `Decryptor`.
///
/// This allows the customization of [`Decryptor`], which can
/// be built using [`DecryptorBuilder::with_policy`].
///
///   [`DecryptorBuilder::with_policy`]: DecryptorBuilder::with_policy()
pub struct DecryptorBuilder<'a> {
    message: Box<dyn BufferedReader<Cookie> + 'a>,
    buffer_size: usize,
    mapping: bool,
}
assert_send_and_sync!(DecryptorBuilder<'_>);

impl<'a> Parse<'a, DecryptorBuilder<'a>>
    for DecryptorBuilder<'a>
{
    fn from_reader<R>(reader: R) -> Result<DecryptorBuilder<'a>>
        where R: io::Read + 'a + Send + Sync,
    {
        DecryptorBuilder::new(buffered_reader::Generic::with_cookie(
            reader, None, Default::default()))
    }

    fn from_file<P>(path: P) -> Result<DecryptorBuilder<'a>>
        where P: AsRef<Path>,
    {
        DecryptorBuilder::new(buffered_reader::File::with_cookie(
            path, Default::default())?)
    }

    fn from_bytes<D>(data: &'a D) -> Result<DecryptorBuilder<'a>>
        where D: AsRef<[u8]> + ?Sized,
    {
        DecryptorBuilder::new(buffered_reader::Memory::with_cookie(
            data.as_ref(), Default::default()))
    }
}

impl<'a> DecryptorBuilder<'a> {
    fn new<B>(signatures: B) -> Result<Self>
        where B: buffered_reader::BufferedReader<Cookie> + 'a
    {
        Ok(DecryptorBuilder {
            message: Box::new(signatures),
            buffer_size: DEFAULT_BUFFER_SIZE,
            mapping: false,
        })
    }

    /// Changes the amount of buffered data.
    ///
    /// By default, we buffer up to 25 megabytes of net message data
    /// (see [`DEFAULT_BUFFER_SIZE`]).  This changes the default.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{*, crypto::*, packet::prelude::*, types::*};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    /// impl DecryptionHelper for Helper {
    ///     // ...
    /// #   fn decrypt<D>(&mut self, _: &[PKESK], skesks: &[SKESK],
    /// #                 _sym_algo: Option<SymmetricAlgorithm>,
    /// #                 mut decrypt: D) -> Result<Option<Fingerprint>>
    /// #       where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
    /// #   {
    /// #       Ok(None)
    /// #   }
    /// }
    ///
    /// let message =
    ///     // ...
    /// # &b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..];
    ///
    /// let h = Helper {};
    /// let mut v = DecryptorBuilder::from_bytes(message)?
    ///     .buffer_size(1 << 12)
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Enables mapping.
    ///
    /// If mapping is enabled, the packet parser will create a [`Map`]
    /// of the packets that can be inspected in
    /// [`VerificationHelper::inspect`].  Note that this buffers the
    /// packets contents, and is not recommended unless you know that
    /// the packets are small.
    ///
    ///   [`Map`]: super::map::Map
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{*, crypto::*, packet::prelude::*, types::*};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    /// impl DecryptionHelper for Helper {
    ///     // ...
    /// #   fn decrypt<D>(&mut self, _: &[PKESK], skesks: &[SKESK],
    /// #                 _sym_algo: Option<SymmetricAlgorithm>,
    /// #                 mut decrypt: D) -> Result<Option<Fingerprint>>
    /// #       where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
    /// #   {
    /// #       Ok(None)
    /// #   }
    /// }
    ///
    /// let message =
    ///     // ...
    /// # &b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..];
    ///
    /// let h = Helper {};
    /// let mut v = DecryptorBuilder::from_bytes(message)?
    ///     .mapping(true)
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn mapping(mut self, enabled: bool) -> Self {
        self.mapping = enabled;
        self
    }

    /// Creates the `Decryptor`.
    ///
    /// Signature verifications are done under the given `policy` and
    /// relative to time `time`, or the current time, if `time` is
    /// `None`.  `helper` is the [`VerificationHelper`] and
    /// [`DecryptionHelper`] to use.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::{*, crypto::*, packet::prelude::*, types::*};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #
    /// #   fn check(&mut self, structure: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    /// impl DecryptionHelper for Helper {
    ///     // ...
    /// #   fn decrypt<D>(&mut self, _: &[PKESK], skesks: &[SKESK],
    /// #                 _sym_algo: Option<SymmetricAlgorithm>,
    /// #                 mut decrypt: D) -> Result<Option<Fingerprint>>
    /// #       where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
    /// #   {
    /// #       Ok(None)
    /// #   }
    /// }
    ///
    /// let message =
    ///     // ...
    /// # &b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..];
    ///
    /// let h = Helper {};
    /// let mut v = DecryptorBuilder::from_bytes(message)?
    ///     // Customize the `Decryptor` here.
    ///     .with_policy(p, None, h)?;
    /// # let _ = v;
    /// # Ok(()) }
    /// ```
    pub fn with_policy<T, H>(self, policy: &'a dyn Policy, time: T, helper: H)
                             -> Result<Decryptor<'a, H>>
        where H: VerificationHelper + DecryptionHelper,
              T: Into<Option<time::SystemTime>>,
    {
        // Do not eagerly map `t` to the current time.
        let t = time.into();
        Decryptor::from_buffered_reader(
            policy,
            self.message,
            helper,
            t, Mode::Decrypt, self.buffer_size, self.mapping, false)
    }
}

/// Helper for decrypting messages.
///
/// This trait abstracts over session key decryption.  It allows us to
/// provide the [`Decryptor`] without imposing any policy on how the
/// session key is decrypted.
///
pub trait DecryptionHelper {
    /// Decrypts the message.
    ///
    /// This function is called with every [`PKESK`] and [`SKESK`]
    /// packet found in the message.  The implementation must decrypt
    /// the symmetric algorithm and session key from one of the
    /// [`PKESK`] packets, the [`SKESK`] packets, or retrieve it from
    /// a cache, and then call `decrypt` with the symmetric algorithm
    /// and session key.  `decrypt` returns `true` if the decryption
    /// was successful.
    ///
    ///   [`PKESK`]: crate::packet::PKESK
    ///   [`SKESK`]: crate::packet::SKESK
    ///
    /// If a symmetric algorithm is given, it should be passed on to
    /// [`PKESK::decrypt`].
    ///
    ///   [`PKESK::decrypt`]: crate::packet::PKESK#method.decrypt
    ///
    /// If the message is decrypted using a [`PKESK`] packet, then the
    /// fingerprint of the certificate containing the encryption
    /// subkey should be returned.  This is used in conjunction with
    /// the intended recipient subpacket (see [Section 5.2.3.29 of RFC
    /// 4880bis]) to prevent [*Surreptitious Forwarding*].
    ///
    ///   [Section 5.2.3.29 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-08#section-5.2.3.29
    ///   [*Surreptitious Forwarding*]: http://world.std.com/~dtd/sign_encrypt/sign_encrypt7.html
    ///
    /// This method will be called once per encryption layer.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to decrypt a message using local
    /// keys (i.e. excluding remote keys like smart cards) while
    /// maximizing convenience for the user.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::{Fingerprint, Cert, Result};
    /// # use openpgp::KeyID;
    /// use openpgp::crypto::SessionKey;
    /// use openpgp::types::SymmetricAlgorithm;
    /// use openpgp::packet::{PKESK, SKESK};
    /// # use openpgp::packet::{Key, key::*};
    /// use openpgp::parse::stream::*;
    /// # fn lookup_cache(_: &[PKESK], _: &[SKESK])
    /// #                 -> Option<(Option<Fingerprint>, SymmetricAlgorithm, SessionKey)> {
    /// #     unimplemented!()
    /// # }
    /// # fn lookup_key(_: &KeyID)
    /// #               -> Option<(Fingerprint, Key<SecretParts, UnspecifiedRole>)> {
    /// #     unimplemented!()
    /// # }
    /// # fn all_keys() -> impl Iterator<Item = (Fingerprint, Key<SecretParts, UnspecifiedRole>)> {
    /// #     Vec::new().into_iter()
    /// # }
    ///
    /// struct Helper { /* ... */ };
    /// impl DecryptionHelper for Helper {
    ///     fn decrypt<D>(&mut self, pkesks: &[PKESK], skesks: &[SKESK],
    ///                   sym_algo: Option<SymmetricAlgorithm>,
    ///                   mut decrypt: D) -> Result<Option<Fingerprint>>
    ///         where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
    ///     {
    ///         // Try to decrypt, from the most convenient method to the
    ///         // least convenient one.
    ///
    ///         // First, see if it is in the cache.
    ///         if let Some((fp, algo, sk)) = lookup_cache(pkesks, skesks) {
    ///             if decrypt(algo, &sk) {
    ///                 return Ok(fp);
    ///             }
    ///         }
    ///
    ///         // Second, we try those keys that we can use without
    ///         // prompting for a password.
    ///         for pkesk in pkesks {
    ///             if let Some((fp, key)) = lookup_key(pkesk.recipient()) {
    ///                 if ! key.secret().is_encrypted() {
    ///                     let mut keypair = key.clone().into_keypair()?;
    ///                     if pkesk.decrypt(&mut keypair, sym_algo)
    ///                         .map(|(algo, sk)| decrypt(algo, &sk))
    ///                         .unwrap_or(false)
    ///                     {
    ///                         return Ok(Some(fp));
    ///                     }
    ///                 }
    ///             }
    ///         }
    ///
    ///         // Third, we try to decrypt PKESK packets with
    ///         // wildcard recipients using those keys that we can
    ///         // use without prompting for a password.
    ///         for pkesk in pkesks.iter().filter(|p| p.recipient().is_wildcard()) {
    ///             for (fp, key) in all_keys() {
    ///                 if ! key.secret().is_encrypted() {
    ///                     let mut keypair = key.clone().into_keypair()?;
    ///                     if pkesk.decrypt(&mut keypair, sym_algo)
    ///                         .map(|(algo, sk)| decrypt(algo, &sk))
    ///                         .unwrap_or(false)
    ///                     {
    ///                         return Ok(Some(fp));
    ///                     }
    ///                 }
    ///             }
    ///         }
    ///
    ///         // Fourth, we try to decrypt all PKESK packets that we
    ///         // need encrypted keys for.
    ///         // [...]
    ///
    ///         // Fifth, we try to decrypt all PKESK packets with
    ///         // wildcard recipients using encrypted keys.
    ///         // [...]
    ///
    ///         // At this point, we have exhausted our options at
    ///         // decrypting the PKESK packets.
    ///         if skesks.is_empty() {
    ///             return
    ///                 Err(anyhow::anyhow!("No key to decrypt message"));
    ///         }
    ///
    ///         // Finally, try to decrypt using the SKESKs.
    ///         loop {
    ///             let password = // Prompt for a password.
    /// #               "".into();
    ///
    ///             for skesk in skesks {
    ///                 if skesk.decrypt(&password)
    ///                     .map(|(algo, sk)| decrypt(algo, &sk))
    ///                     .unwrap_or(false)
    ///                 {
    ///                     return Ok(None);
    ///                 }
    ///             }
    ///
    ///             eprintln!("Bad password.");
    ///         }
    ///     }
    /// }
    /// ```
    fn decrypt<D>(&mut self, pkesks: &[PKESK], skesks: &[SKESK],
                  sym_algo: Option<SymmetricAlgorithm>,
                  decrypt: D) -> Result<Option<Fingerprint>>
        where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool;
}

impl<'a, H: VerificationHelper + DecryptionHelper> Decryptor<'a, H> {
    /// Returns a reference to the helper.
    pub fn helper_ref(&self) -> &H {
        &self.helper
    }

    /// Returns a mutable reference to the helper.
    pub fn helper_mut(&mut self) -> &mut H {
        &mut self.helper
    }

    /// Recovers the helper.
    pub fn into_helper(self) -> H {
        self.helper
    }

    /// Returns true if the whole message has been processed and
    /// authenticated.
    ///
    /// If the function returns `true`, the whole message has been
    /// processed, the signatures are verified, and the message
    /// structure has been passed to [`VerificationHelper::check`].
    /// Data read from this `Verifier` using [`io::Read`] has been
    /// authenticated.
    ///
    ///   [`io::Read`]: std::io::Read
    ///
    /// If the function returns `false`, the message did not fit into
    /// the internal buffer, and therefore data read from this
    /// `Verifier` using [`io::Read`] has **not yet been
    /// authenticated**.  It is important to treat this data as
    /// attacker controlled and not use it until it has been
    /// authenticated.
    ///
    /// # Examples
    ///
    /// This example demonstrates how to verify a message in a
    /// streaming fashion, writing the data to a temporary file and
    /// only commit the result once the data is authenticated.
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use std::io::{Read, Seek, SeekFrom};
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::{KeyHandle, Cert, Result};
    /// use openpgp::parse::{Parse, stream::*};
    /// use openpgp::policy::StandardPolicy;
    /// #
    /// # // Mock of `tempfile::tempfile`.
    /// # mod tempfile {
    /// #     pub fn tempfile() -> sequoia_openpgp::Result<std::fs::File> {
    /// #         unimplemented!()
    /// #     }
    /// # }
    ///
    /// let p = &StandardPolicy::new();
    ///
    /// // This fetches keys and computes the validity of the verification.
    /// struct Helper {};
    /// impl VerificationHelper for Helper {
    ///     // ...
    /// #   fn get_certs(&mut self, ids: &[KeyHandle]) -> Result<Vec<Cert>> {
    /// #       Ok(Vec::new())
    /// #   }
    /// #   fn check(&mut self, _: MessageStructure) -> Result<()> {
    /// #       Ok(())
    /// #   }
    /// }
    ///
    /// let mut source =
    ///    // ...
    /// #  std::io::Cursor::new(&b"-----BEGIN PGP MESSAGE-----
    /// #
    /// #    xA0DAAoW+zdR8Vh9rvEByxJiAAAAAABIZWxsbyBXb3JsZCHCdQQAFgoABgWCXrLl
    /// #    AQAhCRD7N1HxWH2u8RYhBDnRAKtn1b2MBAECBfs3UfFYfa7xRUsBAJaxkU/RCstf
    /// #    UD7TM30IorO1Mb9cDa/hPRxyzipulT55AQDN1m9LMqi9yJDjHNHwYYVwxDcg+pLY
    /// #    YmAFv/UfO0vYBw==
    /// #    =+l94
    /// #    -----END PGP MESSAGE-----
    /// #    "[..]);
    ///
    /// fn consume(r: &mut dyn Read) -> Result<()> {
    ///    // ...
    /// #   let _ = r; Ok(())
    /// }
    ///
    /// let h = Helper {};
    /// let mut v = VerifierBuilder::from_reader(&mut source)?
    ///     .with_policy(p, None, h)?;
    ///
    /// if v.message_processed() {
    ///     // The data has been authenticated.
    ///     consume(&mut v)?;
    /// } else {
    ///     let mut tmp = tempfile::tempfile()?;
    ///     std::io::copy(&mut v, &mut tmp)?;
    ///
    ///     // If the copy succeeds, the message has been fully
    ///     // processed and the data has been authenticated.
    ///     assert!(v.message_processed());
    ///
    ///     // Rewind and consume.
    ///     tmp.seek(SeekFrom::Start(0))?;
    ///     consume(&mut tmp)?;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn message_processed(&self) -> bool {
        // oppr is only None after we've processed the packet sequence.
        self.oppr.is_none()
    }

    /// Creates the `Decryptor`, and buffers the data up to `buffer_size`.
    #[allow(clippy::redundant_pattern_matching)]
    fn from_buffered_reader<T>(
        policy: &'a dyn Policy,
        bio: Box<dyn BufferedReader<Cookie> + 'a>,
        helper: H, time: T,
        mode: Mode,
        buffer_size: usize,
        mapping: bool,
        csf_transformation: bool,
    )
        -> Result<Decryptor<'a, H>>
        where T: Into<Option<time::SystemTime>>
    {
        tracer!(TRACE, "Decryptor::from_buffered_reader", TRACE_INDENT);

        let time = time.into();
        let tolerance = time
            .map(|_| time::Duration::new(0, 0))
            .unwrap_or(
                *crate::packet::signature::subpacket::CLOCK_SKEW_TOLERANCE);
        let time = time.unwrap_or_else(crate::now);

        let mut ppr = PacketParserBuilder::from_buffered_reader(bio)?
            .map(mapping)
            .csf_transformation(csf_transformation)
            .build()?;

        let mut v = Decryptor {
            helper,
            issuers: Vec::new(),
            certs: Vec::new(),
            oppr: None,
            identity: None,
            structure: IMessageStructure::new(),
            buffer_size,
            reserve: None,
            cursor: 0,
            mode,
            time,
            clock_skew_tolerance: tolerance,
            policy,
            processing_csf_message: None, // We don't know yet.
        };

        let mut pkesks: Vec<packet::PKESK> = Vec::new();
        let mut skesks: Vec<packet::SKESK> = Vec::new();

        while let PacketParserResult::Some(mut pp) = ppr {
            // Check whether we are actually processing a cleartext
            // signature framework message.
            if v.processing_csf_message.is_none() {
                v.processing_csf_message = Some(pp.processing_csf_message());
            }

            v.policy.packet(&pp.packet)?;
            v.helper.inspect(&pp)?;

            // When verifying detached signatures, we parse only the
            // signatures here, which on their own are not a valid
            // message.
            if v.mode == Mode::VerifyDetached {
                if pp.packet.tag() != packet::Tag::Signature
                    && pp.packet.tag() != packet::Tag::Marker
                {
                    return Err(Error::MalformedMessage(
                        format!("Expected signature, got {}", pp.packet.tag()))
                               .into());
                }
            } else if let Err(err) = pp.possible_message() {
                if v.processing_csf_message.expect("set by now") {
                    // Our CSF transformation yields just one OPS
                    // packet per encountered 'Hash' algorithm header,
                    // and it cannot know how many signatures are in
                    // fact following.  Therefore, the message will
                    // not be well-formed according to the grammar.
                    // But, since we created the message structure
                    // during the transformation, we know it is good,
                    // even if it is a little out of spec.
                } else {
                    t!("Malformed message: {}", err);
                    return Err(err.context("Malformed OpenPGP message"));
                }
            }

            let sym_algo_hint = if let Packet::AED(ref aed) = pp.packet {
                Some(aed.symmetric_algo())
            } else {
                None
            };

            match pp.packet {
                Packet::CompressedData(ref p) =>
                    v.structure.new_compression_layer(p.algo()),
                Packet::SEIP(_) | Packet::AED(_) if v.mode == Mode::Decrypt => {
                    // Get the symmetric algorithm from the decryption
                    // proxy function.  This is necessary because we
                    // cannot get the algorithm from the SEIP packet.
                    let mut sym_algo = None;
                    {
                        let decryption_proxy = |algo, secret: &SessionKey| {
                            // Take the algo from the AED packet over
                            // the dummy one from the SKESK5 packet.
                            let algo = sym_algo_hint.unwrap_or(algo);
                            let result = pp.decrypt(algo, secret);
                            if let Ok(_) = result {
                                sym_algo = Some(algo);
                                true
                            } else {
                                false
                            }
                        };

                        v.identity =
                            v.helper.decrypt(&pkesks[..], &skesks[..],
                                             sym_algo_hint,
                                             decryption_proxy)?;
                    }
                    if ! pp.processed() {
                        return Err(
                            Error::MissingSessionKey(
                                "No session key decrypted".into()).into());
                    }

                    let sym_algo =
                        sym_algo.expect("if we got here, sym_algo is set");
                    v.policy.symmetric_algorithm(sym_algo)?;
                    if let Packet::AED(ref p) = pp.packet {
                        v.policy.aead_algorithm(p.aead())?;
                    }

                    v.structure.new_encryption_layer(
                        pp.recursion_depth(),
                        pp.packet.tag() == packet::Tag::SEIP
                            && pp.packet.version() == Some(1),
                        sym_algo,
                        if let Packet::AED(ref p) = pp.packet {
                            Some(p.aead())
                        } else {
                            None
                        });
                },
                Packet::OnePassSig(ref ops) => {
                    v.structure.push_ops(ops);
                    v.push_issuer(ops.issuer().clone());
                },
                Packet::Literal(_) => {
                    v.structure.insert_missing_signature_group();
                    v.oppr = Some(PacketParserResult::Some(pp));
                    v.finish_maybe()?;

                    return Ok(v);
                },
                Packet::MDC(ref mdc) => if ! mdc.valid() {
                    return Err(Error::ManipulatedMessage.into());
                },
                _ => (),
            }

            let (p, ppr_tmp) = pp.recurse()?;
            match p {
                Packet::PKESK(pkesk) => pkesks.push(pkesk),
                Packet::SKESK(skesk) => skesks.push(skesk),
                Packet::Signature(sig) => {
                    // The following structure is allowed:
                    //
                    //   SIG LITERAL
                    //
                    // In this case, we get the issuer from the
                    // signature itself.
                    sig.get_issuers().into_iter()
                        .for_each(|i| v.push_issuer(i));
                    v.structure.push_bare_signature(sig);
                }
                _ => (),
            }
            ppr = ppr_tmp;
        }

        if v.mode == Mode::VerifyDetached && !v.structure.layers.is_empty() {
            return Ok(v);
        }

        // We can only get here if we didn't encounter a literal data
        // packet.
        Err(Error::MalformedMessage(
            "Malformed OpenPGP message".into()).into())
    }

    /// Verifies the given data in detached verification mode.
    fn verify_detached<D>(&mut self, data: D) -> Result<()>
        where D: BufferedReader<Cookie>
    {
        assert_eq!(self.mode, Mode::VerifyDetached);

        let sigs = if let IMessageLayer::SignatureGroup {
            sigs, .. } = &mut self.structure.layers[0] {
            sigs
        } else {
            unreachable!("There is exactly one signature group layer")
        };

        // Compute the necessary hashes.
        let algos: Vec<_> = sigs.iter().map(|s| {
            HashingMode::for_signature(s.hash_algo(), s.typ())
        }).collect();
        let hashes =
            crate::parse::hashed_reader::hash_buffered_reader(data, &algos)?;

        // Attach the digests.
        for sig in sigs.iter_mut() {
            let need_hash =
                HashingMode::for_signature(sig.hash_algo(), sig.typ());
            // Note: |hashes| < 10, most likely 1.
            for mode in hashes.iter()
                .filter(|m| m.map(|c| c.algo()) == need_hash)
            {
                // Clone the hash context, update it with the
                // signature.
                use crate::crypto::hash::Hash;
                let mut hash = mode.as_ref().clone();
                sig.hash(&mut hash);

                // Attach digest to the signature.
                let mut digest = vec![0; hash.digest_size()];
                let _ = hash.digest(&mut digest);
                sig.set_computed_digest(Some(digest));
            }
        }

        self.verify_signatures()
    }

    /// Stashes the given Signature (if it is one) for later
    /// verification.
    #[allow(clippy::single_match)]
    fn push_sig(&mut self, p: Packet) -> Result<()> {
        match p {
            Packet::Signature(sig) => {
                sig.get_issuers().into_iter().for_each(|i| self.push_issuer(i));
                self.structure.push_signature(
                    sig, self.processing_csf_message.expect("set by now"));
            },
            _ => (),
        }
        Ok(())
    }

    /// Records the issuer for the later certificate lookup.
    fn push_issuer<I: Into<KeyHandle>>(&mut self, issuer: I) {
        let issuer = issuer.into();
        match issuer {
            KeyHandle::KeyID(id) if id.is_wildcard() => {
                // Ignore, they are not useful for lookups.
            },

            KeyHandle::KeyID(_) => {
                for known in self.issuers.iter() {
                    if known.aliases(&issuer) {
                        return;
                    }
                }

                // Unknown, record.
                self.issuers.push(issuer);
            },

            KeyHandle::Fingerprint(_) => {
                for known in self.issuers.iter_mut() {
                    if known.aliases(&issuer) {
                        // Replace.  We may upgrade a KeyID to a
                        // Fingerprint.
                        *known = issuer;
                        return;
                    }
                }

                // Unknown, record.
                self.issuers.push(issuer);
            },
        }
    }

    // If the amount of remaining data does not exceed the reserve,
    // finish processing the OpenPGP packet sequence.
    //
    // Note: once this call succeeds, you may not call it again.
    fn finish_maybe(&mut self) -> Result<()> {
        tracer!(TRACE, "Decryptor::finish_maybe", TRACE_INDENT);
        if let Some(PacketParserResult::Some(mut pp)) = self.oppr.take() {
            // Check if we hit EOF.
            let data_len = pp.data(self.buffer_size + 1)?.len();
            if data_len - self.cursor <= self.buffer_size {
                // Stash the reserve.
                t!("Hit eof with {} bytes of the current buffer consumed.",
                   self.cursor);
                pp.consume(self.cursor);
                self.cursor = 0;
                self.reserve = Some(pp.steal_eof()?);

                // Process the rest of the packets.
                let mut ppr = PacketParserResult::Some(pp);
                let mut first = true;
                while let PacketParserResult::Some(pp) = ppr {
                    // The literal data packet was already inspected.
                    if first {
                        assert_eq!(pp.packet.tag(), packet::Tag::Literal);
                        first = false;
                    } else {
                        self.helper.inspect(&pp)?;
                    }

                    let possible_message = pp.possible_message();

                    // If we are ascending, and the packet was the
                    // last packet in a SEIP container, we need to be
                    // extra careful with reporting errors to avoid
                    // creating a decryption oracle.

                    let last_recursion_depth = pp.recursion_depth();
                    let (p, ppr_tmp) = match pp.recurse() {
                        Ok(v) => v,
                        Err(e) => {
                            // Assuming we just tried to ascend,
                            // should there have been a MDC packet?
                            // If so, this may be an attack.
                            if self.structure.expect_mdc_at(
                                last_recursion_depth - 1)
                            {
                                return Err(Error::ManipulatedMessage.into());
                            } else {
                                return Err(e);
                            }
                        },
                    };
                    ppr = ppr_tmp;
                    let recursion_depth = ppr.as_ref()
                        .map(|pp| pp.recursion_depth()).unwrap_or(0);

                    // Did we just ascend?
                    if recursion_depth + 1 == last_recursion_depth
                        && self.structure.expect_mdc_at(recursion_depth)
                    {
                        match &p {
                            Packet::MDC(mdc) if mdc.valid() =>
                                (), // Good.
                            _ =>    // Bad.
                                return Err(Error::ManipulatedMessage.into()),
                        }

                        if possible_message.is_err() {
                            return Err(Error::ManipulatedMessage.into());
                        }
                    }

                    if let Err(err) = possible_message {
                        if self.processing_csf_message.expect("set by now") {
                            // CSF transformation creates slightly out
                            // of spec message structure.  See above
                            // for longer explanation.
                        } else {
                            return Err(err.context(
                                "Malformed OpenPGP message"));
                        }
                    }

                    self.push_sig(p)?;
                }

                self.verify_signatures()
            } else {
                t!("Didn't hit EOF.");
                self.oppr = Some(PacketParserResult::Some(pp));
                Ok(())
            }
        } else {
            panic!("No ppr.");
        }
    }

    /// Verifies the signatures.
    #[allow(clippy::blocks_in_if_conditions)]
    fn verify_signatures(&mut self) -> Result<()> {
        tracer!(TRACE, "Decryptor::verify_signatures", TRACE_INDENT);
        t!("called");

        self.certs = self.helper.get_certs(&self.issuers)?;
        t!("VerificationHelper::get_certs produced {} certs", self.certs.len());

        let mut results = MessageStructure::new();
        for layer in self.structure.layers.iter_mut() {
            match layer {
                IMessageLayer::Compression { algo } =>
                    results.new_compression_layer(*algo),
                IMessageLayer::Encryption { sym_algo, aead_algo, .. } =>
                    results.new_encryption_layer(*sym_algo, *aead_algo),
                IMessageLayer::SignatureGroup { sigs, .. } => {
                    results.new_signature_group();
                    'sigs: for sig in sigs.iter_mut() {
                        let sigid = *sig.digest_prefix();

                        let sig_time = if let Some(t) = sig.signature_creation_time() {
                            t
                        } else {
                            // Invalid signature.
                            results.push_verification_result(
                                Err(VerificationError::MalformedSignature {
                                    sig,
                                    error: Error::MalformedPacket(
                                        "missing a Signature Creation Time \
                                         subpacket"
                                            .into()).into(),
                                }));
                            t!("{:02X}{:02X}: Missing a signature creation time subpacket",
                               sigid[0], sigid[1]);
                            continue;
                        };

                        let mut err = VerificationErrorInternal::MissingKey {};

                        let issuers = sig.get_issuers();
                        // Note: If there are no issuers, the only way
                        // to verify the signature is to try every key
                        // that could possibly have created the
                        // signature.  While this may be feasible if
                        // the set of potential signing keys is small,
                        // the use case of hiding the signer's
                        // identity seems better solved using
                        // encryption.  Furthermore, no other OpenPGP
                        // implementation seems to support this kind
                        // of wildcard signatures.
                        //
                        // If there are no issuers, this iterator will
                        // not yield any keys, hence this verification
                        // will fail with the default error,
                        // `VerificationError::MissingKey`.
                        for ka in self.certs.iter()
                            .flat_map(|cert| {
                                cert.keys().key_handles(issuers.iter())
                            })
                        {
                            let cert = ka.cert();
                            let fingerprint = ka.fingerprint();
                            let ka = match ka.with_policy(self.policy, sig_time) {
                                Err(policy_err) => {
                                    t!("{:02X}{:02X}: key {} rejected by policy: {}",
                                       sigid[0], sigid[1], fingerprint, policy_err);
                                    err = VerificationErrorInternal::UnboundKey {
                                        cert,
                                        error: policy_err,
                                    };
                                    continue;
                                }
                                Ok(ka) => {
                                    t!("{:02X}{:02X}: key {} accepted by policy",
                                       sigid[0], sigid[1], fingerprint);
                                    ka
                                }
                            };

                            err = if let Err(error) = ka.cert().alive() {
                                t!("{:02X}{:02X}: cert {} not alive: {}",
                                   sigid[0], sigid[1], ka.cert().fingerprint(), error);
                                VerificationErrorInternal::BadKey {
                                    ka,
                                    error,
                                }
                            } else if let Err(error) = ka.alive() {
                                t!("{:02X}{:02X}: key {} not alive: {}",
                                   sigid[0], sigid[1], ka.fingerprint(), error);
                                VerificationErrorInternal::BadKey {
                                    ka,
                                    error,
                                }
                            } else if let
                                RevocationStatus::Revoked(rev) = ka.cert().revocation_status()
                            {
                                t!("{:02X}{:02X}: cert {} revoked: {:?}",
                                   sigid[0], sigid[1], ka.cert().fingerprint(), rev);
                                VerificationErrorInternal::BadKey {
                                    ka,
                                    error: Error::InvalidKey(
                                        "certificate is revoked".into())
                                        .into(),
                                }
                            } else if let
                                RevocationStatus::Revoked(rev) = ka.revocation_status()
                            {
                                t!("{:02X}{:02X}: key {} revoked: {:?}",
                                   sigid[0], sigid[1], ka.fingerprint(), rev);
                                VerificationErrorInternal::BadKey {
                                    ka,
                                    error: Error::InvalidKey(
                                        "signing key is revoked".into())
                                        .into(),
                                }
                            } else if ! ka.for_signing() {
                                t!("{:02X}{:02X}: key {} not signing capable",
                                   sigid[0], sigid[1], ka.fingerprint());
                                VerificationErrorInternal::BadKey {
                                    ka,
                                    error: Error::InvalidKey(
                                        "key is not signing capable".into())
                                        .into(),
                                }
                            } else if let Err(error) = sig.signature_alive(
                                self.time, self.clock_skew_tolerance)
                            {
                                t!("{:02X}{:02X}: Signature not alive: {}",
                                   sigid[0], sigid[1], error);
                                VerificationErrorInternal::BadSignature {
                                    ka,
                                    error,
                                }
                            } else if self.identity.as_ref().map(|identity| {
                                let (have_one, contains_identity) =
                                    sig.intended_recipients()
                                        .fold((false, false),
                                              |(_, contains_one), ir| {
                                                  (
                                                      true,
                                                      contains_one || identity == ir
                                                  )
                                              });
                                have_one && ! contains_identity
                            }).unwrap_or(false) {
                                // The signature contains intended
                                // recipients, but we are not one.
                                // Treat the signature as bad.
                                t!("{:02X}{:02X}: not an intended recipient",
                                   sigid[0], sigid[1]);
                                VerificationErrorInternal::BadSignature {
                                    ka,
                                    error: Error::BadSignature(
                                        "Not an intended recipient".into())
                                        .into(),
                                }
                            } else {
                                match sig.verify(ka.key()) {
                                    Ok(()) => {
                                        if let Err(error)
                                            = self.policy.signature(
                                                sig, Default::default())
                                        {
                                            t!("{:02X}{:02X}: signature rejected by policy: {}",
                                               sigid[0], sigid[1], error);
                                            VerificationErrorInternal::BadSignature {
                                                ka,
                                                error,
                                            }
                                        } else {
                                            t!("{:02X}{:02X}: good checksum using {}",
                                               sigid[0], sigid[1], ka.fingerprint());
                                            results.push_verification_result(
                                                Ok(GoodChecksum {
                                                    sig,
                                                    ka,
                                                }));
                                            // Continue to the next sig.
                                            continue 'sigs;
                                        }
                                    }
                                    Err(error) => {
                                        t!("{:02X}{:02X} using {}: error: {}",
                                           sigid[0], sigid[1], ka.fingerprint(), error);
                                        VerificationErrorInternal::BadSignature {
                                            ka,
                                            error,
                                        }
                                    }
                                }
                            }
                        }

                        let err = err.attach_sig(sig);
                        t!("{:02X}{:02X}: returning: {:?}", sigid[0], sigid[1], err);
                        results.push_verification_result(Err(err));
                    }
                }
            }
        }

        let r = self.helper.check(results);
        t!("-> {:?}", r);
        r
    }

    /// Like `io::Read::read()`, but returns our `Result`.
    fn read_helper(&mut self, buf: &mut [u8]) -> Result<usize> {
        tracer!(TRACE, "Decryptor::read_helper", TRACE_INDENT);
        t!("read(buf of {} bytes)", buf.len());

        if buf.is_empty() {
            return Ok(0);
        }

        if let Some(ref mut reserve) = self.reserve {
            // The message has been verified.  We can now drain the
            // reserve.
            t!("Message verified, draining reserve.");
            assert!(self.oppr.is_none());
            assert!(self.cursor <= reserve.len());
            let n = cmp::min(buf.len(), reserve.len() - self.cursor);
            buf[..n]
                .copy_from_slice(&reserve[self.cursor..n + self.cursor]);
            self.cursor += n;
            return Ok(n);
        }

        // Read the data from the Literal data packet.
        if let Some(PacketParserResult::Some(mut pp)) = self.oppr.take() {
            // Be careful to not read from the reserve.
            if self.cursor >= self.buffer_size {
                // Consume the active part of the buffer.
                t!("Consuming first part of the buffer.");
                pp.consume(self.buffer_size);
                self.cursor -= self.buffer_size;
            }

            // We request two times what our buffer size is, the first
            // part is the one we give out, the second part is the one
            // we hold back.
            let data_len = pp.data(2 * self.buffer_size)?.len();
            t!("Read {} bytes.", data_len);
            if data_len - self.cursor <= self.buffer_size {
                self.oppr = Some(PacketParserResult::Some(pp));
                self.finish_maybe()?;
                self.read_helper(buf)
            } else {
                let data = pp.data(2 * self.buffer_size - self.cursor)?;
                assert_eq!(data.len(), data_len);

                let n =
                    buf.len().min(data_len - self.buffer_size - self.cursor);
                buf[..n].copy_from_slice(&data[self.cursor..self.cursor + n]);
                self.cursor += n;
                self.oppr = Some(PacketParserResult::Some(pp));
                t!("Copied {} bytes from buffer, cursor is {}.", n, self.cursor);
                Ok(n)
            }
        } else {
            panic!("No ppr.");
        }
    }
}

impl<'a, H: VerificationHelper + DecryptionHelper> io::Read for Decryptor<'a, H>
{
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

#[cfg(test)]
mod test {
    use std::io::Read;
    use super::*;
    use std::convert::TryFrom;
    use crate::parse::Parse;
    use crate::policy::StandardPolicy as P;
    use crate::serialize::Serialize;
    use crate::{
        crypto::Password,
    };

    #[derive(PartialEq)]
    struct VHelper {
        good: usize,
        unknown: usize,
        bad: usize,
        error: usize,
        certs: Vec<Cert>,
        keys: Vec<Cert>,
        passwords: Vec<Password>,
        for_decryption: bool,
        error_out: bool,
    }

    impl std::fmt::Debug for VHelper {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.debug_struct("VHelper")
                .field("good", &self.good)
                .field("unknown", &self.unknown)
                .field("bad", &self.bad)
                .field("error", &self.error)
                .field("error_out", &self.error_out)
                .finish()
        }
    }

    impl Default for VHelper {
        fn default() -> Self {
            VHelper {
                good: 0,
                unknown: 0,
                bad: 0,
                error: 0,
                certs: Vec::default(),
                keys: Vec::default(),
                passwords: Default::default(),
                for_decryption: false,
                error_out: true,
            }
        }
    }

    impl VHelper {
        fn new(good: usize, unknown: usize, bad: usize, error: usize,
               certs: Vec<Cert>)
               -> Self {
            VHelper {
                good,
                unknown,
                bad,
                error,
                certs,
                keys: Default::default(),
                passwords: Default::default(),
                for_decryption: false,
                error_out: true,
            }
        }

        fn for_decryption(good: usize, unknown: usize, bad: usize, error: usize,
                          certs: Vec<Cert>,
                          keys: Vec<Cert>,
                          passwords: Vec<Password>)
               -> Self {
            VHelper {
                good,
                unknown,
                bad,
                error,
                certs,
                keys,
                passwords,
                for_decryption: true,
                error_out: true,
            }
        }
    }

    impl VerificationHelper for VHelper {
        fn get_certs(&mut self, _ids: &[crate::KeyHandle]) -> Result<Vec<Cert>> {
            Ok(self.certs.clone())
        }

        fn check(&mut self, structure: MessageStructure) -> Result<()> {
            use self::VerificationError::*;
            for layer in structure.iter() {
                match layer {
                    MessageLayer::SignatureGroup { ref results } =>
                        for result in results {
                            match result {
                                Ok(_) => self.good += 1,
                                Err(MissingKey { .. }) => self.unknown += 1,
                                Err(UnboundKey { .. }) => self.unknown += 1,
                                Err(MalformedSignature { .. }) => self.bad += 1,
                                Err(BadKey { .. }) => self.bad += 1,
                                Err(BadSignature { error, .. }) => {
                                    eprintln!("error: {}", error);
                                    self.bad += 1;
                                },
                            }
                        }
                    MessageLayer::Compression { .. } => (),
                    MessageLayer::Encryption { .. } => (),
                }
            }

            if ! self.error_out || (self.good > 0 && self.bad == 0)
                || (self.for_decryption && self.certs.is_empty())
            {
                Ok(())
            } else {
                Err(anyhow::anyhow!("Verification failed: {:?}", self))
            }
        }
    }

    impl DecryptionHelper for VHelper {
        fn decrypt<D>(&mut self, pkesks: &[PKESK], _skesks: &[SKESK],
                      sym_algo: Option<SymmetricAlgorithm>, mut decrypt: D)
                      -> Result<Option<Fingerprint>>
            where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
        {
            let p = P::new();
            if ! self.for_decryption {
                unreachable!("Shouldn't be called for verifications");
            }

            for pkesk in pkesks {
                for key in &self.keys {
                    for subkey in key.with_policy(&p, None)?.keys().secret()
                        .key_handle(pkesk.recipient())
                    {
                        if let Some((algo, sk)) =
                            subkey.key().clone().into_keypair().ok()
                            .and_then(|mut k| pkesk.decrypt(&mut k, sym_algo))
                        {
                            if decrypt(algo, &sk) {
                                return Ok(None);
                            }
                        }
                    }
                }
            }

            Err(Error::MissingSessionKey("Decryption failed".into()).into())
        }
    }

    #[test]
    fn verifier() -> Result<()> {
        let p = P::new();

        let certs = [
            "neal.pgp",
            "testy-new.pgp",
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"
        ].iter()
         .map(|f| Cert::from_bytes(crate::tests::key(f)).unwrap())
         .collect::<Vec<_>>();
        let tests = &[
            // Signed messages.
            (crate::tests::message("signed-1.gpg").to_vec(),
             crate::tests::manifesto().to_vec(),
             true,
             Some(crate::frozen_time()),
             VHelper::new(1, 0, 0, 0, certs.clone())),
            // The same, but with a marker packet.
            ({
                let pp = crate::PacketPile::from_bytes(
                    crate::tests::message("signed-1.gpg"))?;
                let mut buf = Vec::new();
                Packet::Marker(Default::default()).serialize(&mut buf)?;
                pp.serialize(&mut buf)?;
                buf
            },
             crate::tests::manifesto().to_vec(),
             true,
             Some(crate::frozen_time()),
             VHelper::new(1, 0, 0, 0, certs.clone())),
            (crate::tests::message("signed-1-sha256-testy.gpg").to_vec(),
             crate::tests::manifesto().to_vec(),
             true,
             Some(crate::frozen_time()),
             VHelper::new(0, 1, 0, 0, certs.clone())),
            (crate::tests::message("signed-1-notarized-by-ed25519.pgp")
             .to_vec(),
             crate::tests::manifesto().to_vec(),
             true,
             Some(crate::frozen_time()),
             VHelper::new(2, 0, 0, 0, certs.clone())),
            // Signed messages using the Cleartext Signature Framework.
            (crate::tests::message("a-cypherpunks-manifesto.txt.cleartext.sig")
             .to_vec(),
             {
                 // The transformation process trims trailing whitespace,
                 // and the manifesto has a trailing whitespace right at
                 // the end.
                 let mut manifesto = crate::tests::manifesto().to_vec();
                 let ws_at = manifesto.len() - 2;
                 let ws = manifesto.remove(ws_at);
                 assert_eq!(ws, b' ');
                 manifesto
             },
             false,
             None,
             VHelper::new(1, 0, 0, 0, certs.clone())),
            (crate::tests::message("a-problematic-poem.txt.cleartext.sig")
             .to_vec(),
             crate::tests::message("a-problematic-poem.txt").to_vec(),
             false,
             None,
             VHelper::new(1, 0, 0, 0, certs.clone())),
            // A key as example of an invalid message.
            (crate::tests::key("neal.pgp").to_vec(),
             crate::tests::manifesto().to_vec(),
             true,
             Some(crate::frozen_time()),
             VHelper::new(0, 0, 0, 1, certs.clone())),
            // A signed message where the signature type is text and a
            // crlf straddles two chunks.
            (crate::tests::message("crlf-straddles-chunks.txt.sig").to_vec(),
             crate::tests::message("crlf-straddles-chunks.txt").to_vec(),
             false,
             None,
             VHelper::new(1, 0, 0, 0, certs.clone())),
            // Like crlf-straddles-chunks, but the signature includes a
            // notation with a '\n'.  Make sure it is not converted to
            // a '\r\n'.
            (crate::tests::message("text-signature-notation-has-lf.txt.sig").to_vec(),
             crate::tests::message("text-signature-notation-has-lf.txt").to_vec(),
             false,
             None,
             VHelper::new(1, 0, 0, 0, certs.clone())),
        ];

        for (i, (signed, reference, test_decryptor, time, r))
            in tests.iter().enumerate()
        {
            eprintln!("{}...", i);

            // Test Verifier.
            let h = VHelper::new(0, 0, 0, 0, certs.clone());
            let mut v =
                match VerifierBuilder::from_bytes(&signed)?
                    .with_policy(&p, *time, h) {
                    Ok(v) => v,
                    Err(e) => if r.error > 0 || r.unknown > 0 {
                        // Expected error.  No point in trying to read
                        // something.
                        continue;
                    } else {
                        panic!("{}: {}", i, e);
                    },
                };
            assert!(v.message_processed());
            assert_eq!(v.helper_ref(), r);

            if v.helper_ref().error > 0 {
                // Expected error.  No point in trying to read
                // something.
                continue;
            }

            let mut content = Vec::new();
            v.read_to_end(&mut content).unwrap();
            assert_eq!(reference.len(), content.len());
            assert_eq!(&reference[..], &content[..]);

            if ! test_decryptor {
                continue;
            }

            // Test Decryptor.
            let h = VHelper::new(0, 0, 0, 0, certs.clone());
            let mut v = match DecryptorBuilder::from_bytes(&signed)?
                .with_policy(&p, *time, h) {
                    Ok(v) => v,
                    Err(e) => if r.error > 0 || r.unknown > 0 {
                        // Expected error.  No point in trying to read
                        // something.
                        continue;
                    } else {
                        panic!("{}: {}", i, e);
                    },
                };
            assert!(v.message_processed());
            assert_eq!(v.helper_ref(), r);

            if v.helper_ref().error > 0 {
                // Expected error.  No point in trying to read
                // something.
                continue;
            }

            let mut content = Vec::new();
            v.read_to_end(&mut content).unwrap();
            assert_eq!(reference.len(), content.len());
            assert_eq!(&reference[..], &content[..]);
        }
        Ok(())
    }

    #[test]
    fn decryptor() -> Result<()> {
        let p = P::new();
        for alg in &[
            "rsa", "elg", "cv25519", "cv25519.unclamped",
            "nistp256", "nistp384", "nistp521",
            "brainpoolP256r1", "brainpoolP384r1", "brainpoolP512r1",
            "secp256k1",
        ] {
            eprintln!("Test vector {:?}...", alg);
            let key = Cert::from_bytes(crate::tests::message(
                &format!("encrypted/{}.sec.pgp", alg)))?;
            if let Some(k) =
                key.with_policy(&p, None)?.keys().subkeys().supported().next()
            {
                use crate::crypto::mpi::PublicKey;
                match k.mpis() {
                    PublicKey::ECDH { curve, .. } if ! curve.is_supported() => {
                        eprintln!("Skipping {} because we don't support \
                                   the curve {}", alg, curve);
                        continue;
                    },
                    _ => (),
                }
            } else {
                eprintln!("Skipping {} because we don't support the algorithm",
                          alg);
                continue;
            }

            let h = VHelper::for_decryption(0, 0, 0, 0, Vec::new(),
                                            vec![key], Vec::new());
            let mut d = DecryptorBuilder::from_bytes(
                crate::tests::message(&format!("encrypted/{}.msg.pgp", alg)))?
                .with_policy(&p, None, h)?;
            assert!(d.message_processed());

            if d.helper_ref().error > 0 {
                // Expected error.  No point in trying to read
                // something.
                continue;
            }

            let mut content = Vec::new();
            d.read_to_end(&mut content).unwrap();
            if content[0] == b'H' {
                assert_eq!(&b"Hello World!\n"[..], &content[..]);
            } else {
                assert_eq!("дружба", &String::from_utf8_lossy(&content));
            }
            eprintln!("decrypted {:?} using {}",
                      String::from_utf8(content).unwrap(), alg);
        }

        Ok(())
    }

    /// Tests legacy two-pass signature scheme, corner cases.
    ///
    /// XXX: This test needs to be adapted once
    /// https://gitlab.com/sequoia-pgp/sequoia/-/issues/128 is
    /// implemented.
    #[test]
    fn verifier_legacy() -> Result<()> {
        let packets = crate::PacketPile::from_bytes(
            crate::tests::message("signed-1.gpg")
        )?
            .into_children()
            .collect::<Vec<_>>();

        fn check(msg: &str, buf: &[u8], expect_good: usize) -> Result<()> {
            eprintln!("{}...", msg);
            let p = P::new();

            let certs = [
                "neal.pgp",
            ]
                .iter()
                .map(|f| Cert::from_bytes(crate::tests::key(f)).unwrap())
                .collect::<Vec<_>>();

            let mut h = VHelper::new(0, 0, 0, 0, certs.clone());
            h.error_out = false;
            let mut v = VerifierBuilder::from_bytes(buf)?
                .with_policy(&p, crate::frozen_time(), h)?;
            assert!(v.message_processed());
            assert_eq!(v.helper_ref().good, expect_good);

            let mut content = Vec::new();
            v.read_to_end(&mut content).unwrap();
            let reference = crate::tests::manifesto();
            assert_eq!(reference.len(), content.len());
            assert_eq!(reference, &content[..]);
            Ok(())
        }

        // Bare legacy signed message: SIG Literal
        let mut o = Vec::new();
        packets[2].serialize(&mut o)?;
        packets[1].serialize(&mut o)?;
        check("bare", &o, 0 /* XXX: should be 1 once #128 is implemented.  */)?;

        // Legacy signed message, two signatures: SIG SIG Literal
        let mut o = Vec::new();
        packets[2].serialize(&mut o)?;
        packets[2].serialize(&mut o)?;
        packets[1].serialize(&mut o)?;
        check("double", &o, 0 /* XXX: should be 2 once #128 is implemented.  */)?;

        // Weird legacy signed message: OPS SIG Literal SIG
        let mut o = Vec::new();
        packets[0].serialize(&mut o)?;
        packets[2].serialize(&mut o)?;
        packets[1].serialize(&mut o)?;
        packets[2].serialize(&mut o)?;
        check("weird", &o, 0 /* XXX: should be 2 once #128 is implemented.  */)?;

        // Fubar legacy signed message: SIG OPS Literal SIG
        let mut o = Vec::new();
        packets[2].serialize(&mut o)?;
        packets[0].serialize(&mut o)?;
        packets[1].serialize(&mut o)?;
        packets[2].serialize(&mut o)?;
        check("fubar", &o, 1 /* XXX: should be 2 once #128 is implemented.  */)?;

        Ok(())
    }

    /// Tests the order of signatures given to
    /// VerificationHelper::check().
    #[test]
    fn verifier_levels() -> Result<()> {
        let p = P::new();

        struct VHelper(());
        impl VerificationHelper for VHelper {
            fn get_certs(&mut self, _ids: &[crate::KeyHandle])
                               -> Result<Vec<Cert>> {
                Ok(Vec::new())
            }

            fn check(&mut self, structure: MessageStructure) -> Result<()> {
                assert_eq!(structure.len(), 2);
                for (i, layer) in structure.into_iter().enumerate() {
                    match layer {
                        MessageLayer::SignatureGroup { results } => {
                            assert_eq!(results.len(), 1);
                            if let Err(VerificationError::MissingKey {
                                sig, ..
                            }) = &results[0] {
                                assert_eq!(
                                    &sig.issuer_fingerprints().next().unwrap()
                                        .to_hex(),
                                    match i {
                                        0 => "8E8C33FA4626337976D97978069C0C348DD82C19",
                                        1 => "C03FA6411B03AE12576461187223B56678E02528",
                                        _ => unreachable!(),
                                    }
                                );
                            } else {
                                unreachable!()
                            }
                        },
                        _ => unreachable!(),
                    }
                }
                Ok(())
            }
        }
        impl DecryptionHelper for VHelper {
            fn decrypt<D>(&mut self, _: &[PKESK], _: &[SKESK],
                          _: Option<SymmetricAlgorithm>, _: D)
                          -> Result<Option<Fingerprint>>
                where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
            {
                unreachable!();
            }
        }

        // Test verifier.
        let v = VerifierBuilder::from_bytes(
            crate::tests::message("signed-1-notarized-by-ed25519.pgp"))?
            .with_policy(&p, crate::frozen_time(), VHelper(()))?;
        assert!(v.message_processed());

        // Test decryptor.
        let v = DecryptorBuilder::from_bytes(
            crate::tests::message("signed-1-notarized-by-ed25519.pgp"))?
            .with_policy(&p, crate::frozen_time(), VHelper(()))?;
        assert!(v.message_processed());
        Ok(())
    }

    #[test]
    fn detached_verifier() -> Result<()> {
        lazy_static::lazy_static! {
            static ref ZEROS: Vec<u8> = vec![0; 100 * 1024 * 1024];
        }

        let p = P::new();

        struct Test<'a> {
            sig: Vec<u8>,
            content: &'a [u8],
            reference: time::SystemTime,
        }
        let tests = [
            Test {
                sig: crate::tests::message(
                    "a-cypherpunks-manifesto.txt.ed25519.sig").to_vec(),
                content: crate::tests::manifesto(),
                reference: crate::frozen_time(),
            },
            // The same, but with a marker packet.
            Test {
                sig: {
                    let sig = crate::PacketPile::from_bytes(
                        crate::tests::message(
                            "a-cypherpunks-manifesto.txt.ed25519.sig"))?;
                    let mut buf = Vec::new();
                    Packet::Marker(Default::default()).serialize(&mut buf)?;
                    sig.serialize(&mut buf)?;
                    buf
                },
                content: crate::tests::manifesto(),
                reference: crate::frozen_time(),
            },
            Test {
                sig: crate::tests::message(
                    "emmelie-dorothea-dina-samantha-awina-detached-signature-of-100MB-of-zeros.sig")
                    .to_vec(),
                content: &ZEROS[..],
                reference:
                crate::types::Timestamp::try_from(1572602018).unwrap().into(),
            },
        ];

        let certs = [
            "emmelie-dorothea-dina-samantha-awina-ed25519.pgp"
        ].iter()
            .map(|f| Cert::from_bytes(crate::tests::key(f)).unwrap())
            .collect::<Vec<_>>();

        for test in tests.iter() {
            let sig = &test.sig;
            let content = test.content;
            let reference = test.reference;

            let h = VHelper::new(0, 0, 0, 0, certs.clone());
            let mut v = DetachedVerifierBuilder::from_bytes(sig).unwrap()
                .with_policy(&p, reference, h).unwrap();
            v.verify_bytes(content).unwrap();

            let h = v.into_helper();
            assert_eq!(h.good, 1);
            assert_eq!(h.bad, 0);
        }
        Ok(())
    }

    #[test]
    fn test_streaming_verifier_bug_issue_682() -> Result<()> {
        let p = P::new();
        let sig = crate::tests::message("signature-with-broken-mpis.sig");

        let h = VHelper::new(0, 0, 0, 0, vec![]);
        let result = DetachedVerifierBuilder::from_bytes(sig)?
        .with_policy(&p, None, h);

        if let Err(e) = result {
            let error = e.downcast::<crate::Error>()?;
            assert!(matches!(error, Error::MalformedMessage(..)));
        } else {
            unreachable!("Should error out as the signature is broken.");
        }

        Ok(())
    }

    #[test]
    fn verify_long_message() -> Result<()> {
        use std::io::Write;
        use crate::serialize::stream::{LiteralWriter, Signer, Message};

        let p = &P::new();

        let (cert, _) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_signing_subkey()
            .generate().unwrap();

        // sign 3MiB message
        let mut buf = vec![];
        {
            let key = cert.keys().with_policy(p, None).for_signing().next().unwrap().key();
            let keypair =
                key.clone().parts_into_secret().unwrap()
                .into_keypair().unwrap();

            let m = Message::new(&mut buf);
            let signer = Signer::new(m, keypair).build().unwrap();
            let mut ls = LiteralWriter::new(signer).build().unwrap();

            ls.write_all(&mut vec![42u8; 3 * 1024 * 1024]).unwrap();
            ls.finalize().unwrap();
        }

        // Test Verifier.
        let h = VHelper::new(0, 0, 0, 0, vec![cert.clone()]);
        let mut v = VerifierBuilder::from_bytes(&buf)?
            .buffer_size(2 * 2usize.pow(20))
            .with_policy(p, None, h)?;

        assert!(!v.message_processed());
        assert!(v.helper_ref().good == 0);
        assert!(v.helper_ref().bad == 0);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);

        let mut message = Vec::new();

        v.read_to_end(&mut message).unwrap();

        assert!(v.message_processed());
        assert_eq!(3 * 1024 * 1024, message.len());
        assert!(message.iter().all(|&b| b == 42));
        assert!(v.helper_ref().good == 1);
        assert!(v.helper_ref().bad == 0);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);

        // Try the same, but this time we let .check() fail.
        let h = VHelper::new(0, 0, /* makes check() fail: */ 1, 0,
                             vec![cert.clone()]);
        let mut v = VerifierBuilder::from_bytes(&buf)?
            .buffer_size(2 * 2usize.pow(20))
            .with_policy(p, None, h)?;

        assert!(!v.message_processed());
        assert!(v.helper_ref().good == 0);
        assert!(v.helper_ref().bad == 1);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);

        let mut message = Vec::new();
        let r = v.read_to_end(&mut message);
        assert!(r.is_err());

        // Check that we only got a truncated message.
        assert!(v.message_processed());
        assert!(!message.is_empty());
        assert!(message.len() <= 1 * 1024 * 1024);
        assert!(message.iter().all(|&b| b == 42));
        assert!(v.helper_ref().good == 1);
        assert!(v.helper_ref().bad == 1);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);

        // Test Decryptor.
        let h = VHelper::new(0, 0, 0, 0, vec![cert.clone()]);
        let mut v = DecryptorBuilder::from_bytes(&buf)?
            .buffer_size(2 * 2usize.pow(20))
            .with_policy(p, None, h)?;

        assert!(!v.message_processed());
        assert!(v.helper_ref().good == 0);
        assert!(v.helper_ref().bad == 0);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);

        let mut message = Vec::new();

        v.read_to_end(&mut message).unwrap();

        assert!(v.message_processed());
        assert_eq!(3 * 1024 * 1024, message.len());
        assert!(message.iter().all(|&b| b == 42));
        assert!(v.helper_ref().good == 1);
        assert!(v.helper_ref().bad == 0);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);

        // Try the same, but this time we let .check() fail.
        let h = VHelper::new(0, 0, /* makes check() fail: */ 1, 0,
                             vec![cert.clone()]);
        let mut v = DecryptorBuilder::from_bytes(&buf)?
            .buffer_size(2 * 2usize.pow(20))
            .with_policy(p, None, h)?;

        assert!(!v.message_processed());
        assert!(v.helper_ref().good == 0);
        assert!(v.helper_ref().bad == 1);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);

        let mut message = Vec::new();
        let r = v.read_to_end(&mut message);
        assert!(r.is_err());

        // Check that we only got a truncated message.
        assert!(v.message_processed());
        assert!(!message.is_empty());
        assert!(message.len() <= 1 * 1024 * 1024);
        assert!(message.iter().all(|&b| b == 42));
        assert!(v.helper_ref().good == 1);
        assert!(v.helper_ref().bad == 1);
        assert!(v.helper_ref().unknown == 0);
        assert!(v.helper_ref().error == 0);
        Ok(())
    }

    /// Checks that tampering with the MDC yields a uniform error
    /// response.
    #[test]
    fn issue_693() -> Result<()> {
        struct H();
        impl VerificationHelper for H {
            fn get_certs(&mut self, _ids: &[crate::KeyHandle])
                         -> Result<Vec<Cert>> {
                Ok(Vec::new())
            }

            fn check(&mut self, _: MessageStructure)
                     -> Result<()> {
                Ok(())
            }
        }
        impl DecryptionHelper for H {
            fn decrypt<D>(&mut self, _: &[PKESK], s: &[SKESK],
                          _: Option<SymmetricAlgorithm>, mut decrypt: D)
                          -> Result<Option<Fingerprint>>
            where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
            {
                let (algo, sk) = s[0].decrypt(&"123".into()).unwrap();
                let r = decrypt(algo, &sk);
                assert!(r);
                Ok(None)
            }
        }

        fn check(m: &str) -> Result<()> {
            let doit = || -> Result<()> {
                let p = &P::new();
                let mut decryptor = DecryptorBuilder::from_bytes(m.as_bytes())?
                    .with_policy(p, None, H())?;
                let mut b = Vec::new();
                decryptor.read_to_end(&mut b)?;
                Ok(())
            };

            let e = doit().unwrap_err();
            match e.downcast::<io::Error>() {
                Ok(e) =>
                    assert_eq!(e.into_inner().unwrap().downcast().unwrap(),
                               Box::new(Error::ManipulatedMessage)),
                Err(e) =>
                    assert_eq!(e.downcast::<Error>().unwrap(),
                               Error::ManipulatedMessage),
            };
            Ok(())
        }

        // Bad hash.
        check("-----BEGIN PGP MESSAGE-----

wx4EBwMI7dKRUiOYGCUAWmzhiYGS8Pn/16QkyTous6vSOgFMcilte26C7kej
rKhvjj6uYNT+mt+L2Yg/FHFvpgVF3KfP0fb+9jZwgt4qpDkTMY7AWPTK6wXX
Jo8=
=LS8u
-----END PGP MESSAGE-----
")?;

        // Bad header.
        check("-----BEGIN PGP MESSAGE-----

wx4EBwMI7sPTdlgQwd8AogIcbF/hLVrYbvVbgj4EC6/SOgGNaCyffrR4Fuwl
Ft2w56/hB/gTaGEhCgDGXg8NiFGIURqF3eIwxxdKWghUutYmsGwqOZmdJ49a
9gE=
=DzKF
-----END PGP MESSAGE-----
")?;

        // Bad header matching other packet type.
        check("-----BEGIN PGP MESSAGE-----

wx4EBwMIhpEGBh3v0oMAYgGcj+4CG1mcWQwmyGIDRdvSOgFSHlL2GZ1ZKeXS
29kScqGg2U8N6ZF9vmj/9Sn7CFtO5PGXn2owQVsopeUSTofV3BNUBpxaBDCO
EK8=
=TgeJ
-----END PGP MESSAGE-----
")?;

        Ok(())
    }

    /// Tests whether messages using the cleartext signature framework
    /// with multiple signatures and signers are correctly handled.
    #[test]
    fn csf_multiple_signers() -> Result<()> {
        struct H(bool);
        impl VerificationHelper for H {
            fn get_certs(&mut self, _ids: &[crate::KeyHandle])
                         -> Result<Vec<Cert>> {
                crate::cert::CertParser::from_bytes(
                    crate::tests::key("InRelease.signers.pgp"))?
                    .collect()
            }

            fn check(&mut self, m: MessageStructure)
                     -> Result<()> {
                for (i, layer) in m.into_iter().enumerate() {
                    assert_eq!(i, 0);
                    if let MessageLayer::SignatureGroup { results } = layer {
                        assert_eq!(results.len(), 3);
                        for result in results {
                            assert!(result.is_ok());
                        }
                        self.0 = true;
                    } else {
                        panic!();
                    }
                }

                Ok(())
            }
        }

        let p = &P::new();
        let mut verifier = VerifierBuilder::from_bytes(
            crate::tests::message("InRelease"))?
            .with_policy(p, None, H(false))?;
        let mut b = Vec::new();
        verifier.read_to_end(&mut b)?;
        let h = verifier.into_helper();
        assert!(h.0);
        Ok(())
    }
}
