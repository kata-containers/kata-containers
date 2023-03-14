//! A mechanism to specify policy.
//!
//! A major goal of the Sequoia OpenPGP crate is to be policy free.
//! However, many mid-level operations build on low-level primitives.
//! For instance, finding a certificate's primary User ID means
//! examining each of its User IDs and their current self-signature.
//! Some algorithms are considered broken (e.g., MD5) and some are
//! considered weak (e.g. SHA-1).  When dealing with data from an
//! untrusted source, for instance, callers will often prefer to
//! ignore signatures that rely on these algorithms even though [RFC
//! 4880] says that "\[i\]mplementations MUST implement SHA-1."  When
//! trying to decrypt old archives, however, users probably don't want
//! to ignore keys using MD5, even though [RFC 4880] deprecates MD5.
//!
//! Rather than not provide this mid-level functionality, the `Policy`
//! trait allows callers to specify their preferred policy.  This can be
//! highly customized by providing a custom implementation of the
//! `Policy` trait, or it can be slightly refined by tweaking the
//! `StandardPolicy`'s parameters.
//!
//! When implementing the `Policy` trait, it is *essential* that the
//! functions are [pure].  That is, if the same `Policy` is used
//! to determine whether a given `Signature` is valid, it must always
//! return the same value.
//!
//! [RFC 4880]: https://tools.ietf.org/html/rfc4880#section-9.4
//! [pure]: https://en.wikipedia.org/wiki/Pure_function
use std::fmt;
use std::time::{SystemTime, Duration};
use std::u32;

use anyhow::Context;

use crate::{
    cert::prelude::*,
    Error,
    Packet,
    packet::{
        key,
        Signature,
        signature::subpacket::{
            SubpacketTag,
            SubpacketValue,
        },
        Tag,
    },
    Result,
    types,
    types::{
        AEADAlgorithm,
        HashAlgorithm,
        SignatureType,
        SymmetricAlgorithm,
        Timestamp,
    },
};

#[macro_use] mod cutofflist;
use cutofflist::{
    CutoffList,
    REJECT,
    ACCEPT,
    VersionedCutoffList,
};

/// A policy for cryptographic operations.
pub trait Policy : fmt::Debug + Send + Sync {
    /// Returns an error if the signature violates the policy.
    ///
    /// This function performs the last check before the library
    /// decides that a signature is valid.  That is, after the library
    /// has determined that the signature is well-formed, alive, not
    /// revoked, etc., it calls this function to allow you to
    /// implement any additional policy.  For instance, you may reject
    /// signatures that make use of cryptographically insecure
    /// algorithms like SHA-1.
    ///
    /// Note: Whereas it is generally better to reject suspicious
    /// signatures, one should be more liberal when considering
    /// revocations: if you reject a revocation certificate, it may
    /// inadvertently make something else valid!
    fn signature(&self, _sig: &Signature, _sec: HashAlgoSecurity) -> Result<()> {
        Err(anyhow::anyhow!("By default all signatures are rejected."))
    }

    /// Returns an error if the key violates the policy.
    ///
    /// This function performs one of the last checks before a
    /// `KeyAmalgamation` or a related data structures is turned into
    /// a `ValidKeyAmalgamation`, or similar.
    ///
    /// Internally, the library always does this before using a key.
    /// The sole exception is when creating a key using `CertBuilder`.
    /// In that case, the primary key is not validated before it is
    /// used to create any binding signatures.
    ///
    /// Thus, you can prevent keys that make use of insecure
    /// algorithms, don't have a sufficiently high security margin
    /// (e.g., 1024-bit RSA keys), are on a bad list, etc. from being
    /// used here.
    ///
    /// If you implement this function, make sure to consider the Key
    /// Derivation Function and Key Encapsulation parameters of ECDH
    /// keys, see [`PublicKey::ECDH`].
    ///
    /// [`PublicKey::ECDH`]: crate::crypto::mpi::PublicKey::ECDH
    fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
        -> Result<()>
    {
        Err(anyhow::anyhow!("By default all keys are rejected."))
    }

    /// Returns an error if the symmetric encryption algorithm
    /// violates the policy.
    ///
    /// This function performs the last check before an encryption
    /// container is decrypted by the streaming decryptor.
    ///
    /// With this function, you can prevent the use of insecure
    /// symmetric encryption algorithms.
    fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
        Err(anyhow::anyhow!("By default all symmetric algorithms are rejected."))
    }

    /// Returns an error if the AEAD mode violates the policy.
    ///
    /// This function performs the last check before an encryption
    /// container is decrypted by the streaming decryptor.
    ///
    /// With this function, you can prevent the use of insecure AEAD
    /// constructions.
    ///
    /// This feature is [experimental](super#experimental-features).
    fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
        Err(anyhow::anyhow!("By default all AEAD algorithms are rejected."))
    }

    /// Returns an error if the packet violates the policy.
    ///
    /// This function performs the last check before a packet is
    /// considered by the streaming verifier and decryptor.
    ///
    /// With this function, you can prevent the use of insecure
    /// encryption containers, notably the *Symmetrically Encrypted
    /// Data Packet*.
    fn packet(&self, _packet: &Packet) -> Result<()> {
        Err(anyhow::anyhow!("By default all packets are rejected."))
    }
}

/// Whether the signed data requires a hash algorithm with collision
/// resistance.
///
/// Since the context of a signature is not passed to
/// `Policy::signature`, it is not possible to determine from that
/// function whether the signature requires a hash algorithm with
/// collision resistance.  This enum indicates this.
///
/// In short, many self signatures only require second pre-image
/// resistance.  This can be used to extend the life of hash
/// algorithms whose collision resistance has been partially
/// compromised.  Be careful.  Read the background and the warning
/// before accepting the use of weak hash algorithms!
///
/// # Warning
///
/// Although distinguishing whether signed data requires collision
/// resistance can be used to permit the continued use of a hash
/// algorithm in certain situations, once attacks against a hash
/// algorithm are known, it is imperative to retire the use of the
/// hash algorithm as soon as it is feasible.  Cryptoanalytic attacks
/// improve quickly, as demonstrated by the attacks on SHA-1.
///
/// # Background
///
/// Cryptographic hash functions normally have three security
/// properties:
///
///   - Pre-image resistance,
///   - Second pre-image resistance, and
///   - Collision resistance.
///
/// A hash algorithm has pre-image resistance if given a hash `h`, it
/// is impractical for an attacker to find a message `m` such that `h
/// = hash(m)`.  In other words, a hash algorithm has pre-image
/// resistance if it is hard to invert.  A hash algorithm has second
/// pre-image resistance if it is impractical for an attacker to find
/// a second message with the same hash as the first.  That is, given
/// `m1`, it is hard for an attacker to find an `m2` such that
/// `hash(m1) = hash(m2)`.  And, a hash algorithm has collision
/// resistance if it is impractical for an attacker to find two
/// messages with the same hash.  That is, it is hard for an attacker
/// to find an `m1` and an `m2` such that `hash(m1) = hash(m2)`.
///
/// In the context of verifying an OpenPGP signature, we don't need a
/// hash algorithm with pre-image resistance.  Pre-image resistance is
/// only required when the message is a secret, e.g., a password.  We
/// always need a hash algorithm with second pre-image resistance,
/// because an attacker must not be able to repurpose an arbitrary
/// signature, i.e., create a collision with respect to a *known*
/// hash.  And, we need collision resistance when a signature is over
/// data that could have been influenced by an attacker: if an
/// attacker creates a pair of colliding messages and convinces the
/// user to sign one of them, then the attacker can copy the signature
/// to the other message.
///
/// Collision resistance implies second pre-image resistance, but not
/// vice versa.  If an attacker can find a second message with the
/// same hash as some known message, they can also create a collision
/// by choosing an arbitrary message and using their pre-image attack
/// to find a colliding message.  Thus, a context that requires
/// collision resistance also requires second pre-image resistance.
///
/// Because collision resistance is with respect to two arbitrary
/// messages, collision resistance is always susceptible to a
/// [birthday paradox].  This means that the security margin of a hash
/// algorithm's collision resistance is half of the security margin of
/// its second pre-image resistance.  And, in practice, the collision
/// resistance of industry standard hash algorithms has been
/// practically attacked multiple times.  In the context of SHA-1,
/// Wang et al. described how to find collisions in SHA-1 in their
/// 2005 paper [Finding Collisions in the Full SHA-1].  In 2017,
/// Stevens et al. published [The First Collision for Full SHA-1],
/// which demonstrates the first practical attack on SHA-1's collision
/// resistance, an identical-prefix collision attack.  This attack
/// only gives the attacker limited control over the content of the
/// collided messages, which limits its applicability.  However, in
/// 2020, Leurent and Peyrin published [SHA-1 is a Shambles], which
/// demonstrates a practical chosen-prefix collision attack.  This
/// attack gives the attacker complete control over the prefixes of
/// the collided messages.
///
///   [birthday paradox]: https://en.wikipedia.org/wiki/Birthday_attack#Digital_signature_susceptibility
///   [Finding Collisions in the Full SHA-1]: https://link.springer.com/chapter/10.1007/11535218_2
///   [The first collision for full SHA-1]: https://shattered.io/
///   [SHA-1 is a Shambles]: https://sha-mbles.github.io/
///
/// A chosen-prefix collision attack works as follows: an attacker
/// chooses two arbitrary message prefixes, and then searches for
/// so-called near collision blocks.  These near collision blocks
/// cause the internal state of the hashes to converge and eventually
/// result in a collision, i.e., an identical hash value.  The attack
/// described in the [SHA-1 is a Shambles] paper requires 8 to 10 near
/// collision blocks (512 to 640 bytes) to fully synchronize the
/// internal state.
///
/// SHA-1 is a [Merkle-Damgård hash function].  This means that the
/// hash function processes blocks one after the other, and the
/// internal state of the hash function at any given point only
/// depends on earlier blocks in the stream.  A consequence of this is
/// that it is possible to append a common suffix to the collided
/// messages without any additional computational effort.  That is, if
/// `hash(m1) = hash(m2)`, then it necessarily holds that `hash(m1 ||
/// suffix) = hash(m2 || suffix)`.  This is called a [length extension
/// attack].
///
///   [Merkle-Damgård hash function]: https://en.wikipedia.org/wiki/Merkle%E2%80%93Damg%C3%A5rd_construction
///   [length extension attack]: https://en.wikipedia.org/wiki/Length_extension_attack
///
/// Thus, the [SHA-1 is a Shambles] attack solves the following:
///
/// ```text
/// hash(m1 || collision blocks 1 || suffix) = hash(m2 || collision blocks 2 || suffix)
/// ```
///
/// Where `m1`, `m2`, and `suffix` are controlled by the attacker, and
/// only the collision blocks are controlled by the algorithm.
///
/// If an attacker can convince an OpenPGP user to sign a message of
/// their choosing (some `m1 || collision blocks 1 || suffix`), then
/// the attacker also has a valid signature from the victim for a
/// colliding message (some `m2 || collision blocks 2 || suffix`).
///
/// The OpenPGP format imposes some additional constraints on the
/// attacker.  Although the attacker may control the message, the
/// signature is also over a [signature packet], and a trailer.
/// Specifically, [the following is signed] when signing a document:
///
/// ```text
/// hash(document || sig packet || 0x04 || sig packet len)
/// ```
///
/// and the [following is signed] when signing a binding signature:
///
/// ```text
/// hash(public key || subkey || sig packet || 0x04 || sig packet len)
/// ```
///
///  [signature packet]: https://tools.ietf.org/html/rfc4880#section-5.2.3
///  [the following is signed]: https://tools.ietf.org/html/rfc4880#section-5.2.4
///
/// Since the signature packet is chosen by the victim's OpenPGP
/// implementation, the attacker may be able to predict it, but they
/// cannot store the collision blocks there.  Thus, the signature
/// packet is necessarily part of the common suffix, and the collision
/// blocks must occur earlier in the stream.
///
/// This restriction on the signature packet means that an attacker
/// cannot convince the victim to sign a document, and then transfer
/// that signature to a colliding binding signature.  These signatures
/// necessarily have different [signature packet]s: the value of the
/// [signature type] field is different.  And, as just described, for
/// this attack, the signature packets must be identical, because they
/// are part of the common suffix.  Finally, the trailer, which
/// contains the signature packet's length, prevents hiding a
/// signature in a signature.
///
///   [signature type]: https://tools.ietf.org/html/rfc4880#section-5.2.1
///
/// Given this, if we know for a given signature type that an attacker
/// cannot control any of the data that is signed, then that type of
/// signature does not need collision resistance; it is still
/// vulnerable to an attack on the hash's second pre-image resistance
/// (a collision with a specific message), but not one on its
/// collision resistance (a collision with any message).  This is the
/// case for binding signatures, and direct key signatures.  But, it
/// is not normally the case for documents (the attacker may be able
/// to control the content of the document), certifications (the
/// attacker may be able to control the the key packet, the User ID
/// packet, or the User Attribute packet), or certificate revocations
/// (the attacker may be able to control the key packet).
///
/// Certification signatures and revocations signatures can be further
/// divided into self signatures and third-party signatures.  If an
/// attacker can convince a victim into signing a third-party
/// signature, as was done in the [SHA-1 is a Shambles], they may be
/// able to transfer the signature to a colliding self signature.  If
/// we can show that an attacker can't collide a self signature, and a
/// third-party signature, then we may be able to show that self
/// signatures don't require collision resistance.  The same
/// consideration holds for revocations and third-party revocations.
///
/// We first consider revocations, which are more straightforward.
/// The attack is the following: an attacker creates a fake
/// certificate (A), and sets the victim as a designated revoker.
/// They then ask the victim to revoke their certificate (V).  The
/// attacker than transfers the signature to a colliding self
/// revocation, which causes the victim's certificate (V) to be
/// revoked.
///
/// A revocation is over a public key packet and a signature packet.
/// In this scenario, the attacker controls the fake certificate (A)
/// and thus the public key packet that the victim actually signs.
/// But the victim's public key packet is determined by their
/// certificate (V).  Thus, the attacker would have to insert the near
/// collision blocks in the signature packet, which, as we argued
/// before, is not possible.  Thus, it is safe to only use a hash with
/// pre-image resistance to protect a self-revocation.
///
/// We now turn to self signatures.  The attack is similar to the
/// [SHA-1 is a Shambles] attack.  An attacker creates a certificate
/// (A) and convinces the victim to sign it.  The attacker can then
/// transfer the third-party certification to a colliding self
/// signature for the victim's certificate (V).  If successful, this
/// attack allows the attacker to add a User ID or a User Attribute to
/// the victim's certificate (V).  This can confuse people who use the
/// victim's certificate.  For instance, if the attacker adds the
/// identity `alice@example.org` to the victim's certificate, and Bob
/// receives a message signed using the victim's certificate (V), he
/// may think that Alice signed the message instead of the victim.
/// Bob won't be tricked if he uses strong authentication, but many
/// OpenPGP users use weak authentication (e.g., TOFU) or don't
/// authenticate keys at all.
///
/// A certification is over a public key packet, a User ID or User
/// Attribute packet, and a signature packet.  The attacker controls
/// the fake certificate (A) and therefore the public key packet, and
/// the User ID or User Attribute packet that the victim signs.
/// However, to trick the victim, the User ID packet or User Attribute
/// packet needs to correspond to an identity that the attacker
/// appears to control.  Thus, if the near collision blocks are stored
/// in the User ID or User Attribute packet of A, they have to be
/// hidden to avoid making the victim suspicious.  This is
/// straightforward for User Attributes, which are currently images,
/// and have many places to hide this type of data.  However, User IDs
/// are are normally [UTF-8 encoded RFC 2822 mailbox]es, which makes
/// hiding half a kilobyte of binary data impractical.  The attacker
/// does not control the victim's public key (in V).  But, they do
/// control the malicious User ID or User Attribute that they want to
/// attack to the victim's certificate (V).  But again, the near
/// collision blocks have to be hidden in order to trick Bob, the
/// second victim.  Thus, the attack has two possibilities: they can
/// hide the near collision blocks in the fake public key (in A), and
/// the User ID or User Attribute (added to V); or, they can hide them
/// in the fake User IDs or User Attributes (in A and the one added to
/// V).
///
/// As evidenced by the [SHA-1 is a Shambles] attack, it is possible
/// to hide near collision blocks in User Attribute packets.  Thus,
/// this attack can be used to transfer a third-party certification
/// over a User Attribute to a self signature over a User Attribute.
/// As such, self signatures over User Attributes need collision
/// resistance.
///
/// The final case to consider is hiding the near collision blocks in
/// the User ID that the attacker wants to add to the victim's
/// certificate.  Again, it is possible to store the near collision
/// blocks there.  However, there are two mitigating factors.  First,
/// there is no place to hide the blocks.  As such, the user must be
/// convinced to ignore them.  Second, a User ID is structure: it
/// normally contains a [UTF-8 encoded RFC 2822 mailbox].  Thus, if we
/// only consider valid UTF-8 strings, and limit the maximum size, we
/// can dramatically increase the workfactor, which can extend the life
/// of a hash algorithm whose collision resistance has been weakened.
///
///   [UTF-8 encoded RFC 2822 mailbox]: https://tools.ietf.org/html/rfc4880#section-5.11
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HashAlgoSecurity {
    /// The signed data only requires second pre-image resistance.
    ///
    /// If a signature is over data that an attacker cannot influence,
    /// then the hash function does not need to provide collision
    /// resistance.  This is **only** the case for:
    ///
    ///   - Subkey binding signatures
    ///   - Primary key binding signatures
    ///   - Self revocations
    ///
    /// Due to the structure of User IDs (they are normally short,
    /// UTF-8 encoded RFC 2822 mailboxes), self signatures over short,
    /// reasonable User IDs (**not** User Attributes) also don't
    /// require strong collision resistance.  Thus, we also only
    /// require a signature with second pre-image resistance for:
    ///
    ///   - Self signatures over reasonable User IDs
    SecondPreImageResistance,
    /// The signed data requires collision resistance.
    ///
    /// If a signature is over data that an attacker can influence,
    /// then the hash function must provide collision resistance.
    /// This is the case for documents, third-party certifications,
    /// and third-party revocations.
    ///
    /// Note: collision resistance implies second pre-image
    /// resistance.  Thus, when evaluating whether a hash algorithm
    /// has collision resistance, we also check whether it has second
    /// pre-image resistance.
    CollisionResistance,
}

impl Default for HashAlgoSecurity {
    /// The default is the most conservative policy.
    fn default() -> Self {
        HashAlgoSecurity::CollisionResistance
    }
}

/// The standard policy.
///
/// The standard policy stores when each algorithm in a family of
/// algorithms is no longer considered safe.  Attempts to use an
/// algorithm after its cutoff time should fail.
///
/// A `StandardPolicy` can be configured using Rust.  Sometimes it is
/// useful to configure it via a configuration file.  This can be done
/// using the [`sequoia-policy-config`] crate.
///
///   [`sequoia-policy-config`]: https://docs.rs/sequoia-policy-config/latest/sequoia_policy_config/
///
/// It is recommended to support using a configuration file when the
/// program should respect the system's crypto policy.  This is
/// required on Fedora, for instance.  See the [Fedora Crypto
/// Policies] project for more information.
///
///   [Fedora]: https://gitlab.com/redhat-crypto/fedora-crypto-policies
///
/// When validating a signature, we normally want to know whether the
/// algorithms used are safe *now*.  That is, we don't use the
/// signature's alleged creation time when considering whether an
/// algorithm is safe, because if an algorithm is discovered to be
/// compromised at time X, then an attacker could forge a message
/// after time X with a signature creation time that is prior to X,
/// which would be incorrectly accepted.
///
/// Occasionally, we know that a signature has not been tampered with
/// since some time in the past.  We might know this if the signature
/// was stored on some tamper-proof medium.  In those cases, it is
/// reasonable to use the time that the signature was saved, since an
/// attacker could not have taken advantage of any weaknesses found
/// after that time.
///
/// # Examples
///
/// A `StandardPolicy` object can be used to build specialized policies.
/// For example the following policy filters out Persona certifications mimicking
/// what GnuPG does when calculating the Web of Trust.
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// use std::io::{Cursor, Read};
/// use openpgp::Result;
/// use openpgp::packet::{Packet, Signature, key::PublicParts};
/// use openpgp::cert::prelude::*;
/// use openpgp::parse::Parse;
/// use openpgp::armor::{Reader, ReaderMode, Kind};
/// use openpgp::policy::{HashAlgoSecurity, Policy, StandardPolicy};
/// use openpgp::types::{
///    SymmetricAlgorithm,
///    AEADAlgorithm,
///    SignatureType
/// };
///
/// #[derive(Debug)]
/// struct RejectPersonaCertificationsPolicy<'a>(StandardPolicy<'a>);
///
/// impl Policy for RejectPersonaCertificationsPolicy<'_> {
///     fn key(&self, ka: &ValidErasedKeyAmalgamation<PublicParts>)
///            -> Result<()>
///     {
///         self.0.key(ka)
///     }
///
///     fn signature(&self, sig: &Signature, sec: HashAlgoSecurity) -> Result<()> {
///         if sig.typ() == SignatureType::PersonaCertification {
///             Err(anyhow::anyhow!("Persona certifications are ignored."))
///         } else {
///             self.0.signature(sig, sec)
///         }
///     }
///
///     fn symmetric_algorithm(&self, algo: SymmetricAlgorithm) -> Result<()> {
///         self.0.symmetric_algorithm(algo)
///     }
///
///     fn aead_algorithm(&self, algo: AEADAlgorithm) -> Result<()> {
///         self.0.aead_algorithm(algo)
///     }
///
///     fn packet(&self, packet: &Packet) -> Result<()> {
///         self.0.packet(packet)
///     }
/// }
///
/// impl RejectPersonaCertificationsPolicy<'_> {
///     fn new() -> Self {
///         Self(StandardPolicy::new())
///     }
/// }
///
/// # fn main() -> Result<()> {
/// // this key has one persona certification
/// let data = r#"
/// -----BEGIN PGP PUBLIC KEY BLOCK-----
///
/// mDMEX7JGrxYJKwYBBAHaRw8BAQdASKGcnowaZBDc2Z3rZZlWb6jEjne9sK76afbJ
/// trd5Uw+0BlRlc3QgMoiQBBMWCAA4FiEEyZ6oBYFia3z+ooCBqR9BqiGp8AQFAl+y
/// Rq8CGwMFCwkIBwIGFQoJCAsCBBYCAwECHgECF4AACgkQqR9BqiGp8ASfxwEAvEb0
/// bFr7ZgFZSDOITNptm+FEynib8mmLACsvHAmCjvIA+gOaSNyxMW6N59q7/j0sDjp1
/// aYNgpNFLbYBZpkXXVL0GiHUEERYIAB0WIQTE4QfdkkisIbWVOcHmlsuS3dbWEwUC
/// X7JG4gAKCRDmlsuS3dbWExEwAQCpqfiVMhjDwVFMsMpwd5r0N/8rAx8/nmgpCsK3
/// M9TUrAD7BhTYVPRbkJqTZYd9DlLtBcbF3yNPTHlB+F2sFjI+cgo=
/// =ZfYu
/// -----END PGP PUBLIC KEY BLOCK-----
/// "#;
///
/// let mut cursor = Cursor::new(&data);
/// let mut reader = Reader::from_reader(&mut cursor, ReaderMode::Tolerant(Some(Kind::PublicKey)));
///
/// let mut buf = Vec::new();
/// reader.read_to_end(&mut buf)?;
/// let cert = Cert::from_bytes(&buf)?;
///
/// let ref sp = StandardPolicy::new();
/// let u = cert.with_policy(sp, None)?.userids().nth(0).unwrap();
///
/// // Under the standard policy the persona certification is visible.
/// assert_eq!(u.certifications().count(), 1);
///
/// // Under our custom policy the persona certification is not available.
/// let ref p = RejectPersonaCertificationsPolicy::new();
/// assert_eq!(u.with_policy(p, None)?.certifications().count(), 0);
/// #
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct StandardPolicy<'a> {
    // The time.  If None, the current time is used.
    time: Option<Timestamp>,

    // Hash algorithms.
    collision_resistant_hash_algos:
        CollisionResistantHashCutoffList,
    second_pre_image_resistant_hash_algos:
        SecondPreImageResistantHashCutoffList,
    hash_revocation_tolerance: types::Duration,

    // Critical subpacket tags.
    critical_subpackets: SubpacketTagCutoffList,

    // Critical notation good-list.
    good_critical_notations: &'a [&'a str],

    // Packet types.
    packet_tags: PacketTagCutoffList,

    // Symmetric algorithms.
    symmetric_algos: SymmetricAlgorithmCutoffList,

    // AEAD algorithms.
    aead_algos: AEADAlgorithmCutoffList,

    // Asymmetric algorithms.
    asymmetric_algos: AsymmetricAlgorithmCutoffList,
}

assert_send_and_sync!(StandardPolicy<'_>);

impl<'a> Default for StandardPolicy<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> From<&'a StandardPolicy<'a>> for Option<&'a dyn Policy> {
    fn from(p: &'a StandardPolicy<'a>) -> Self {
        Some(p as &dyn Policy)
    }
}

// Signatures that require a hash with collision Resistance and second
// Pre-image Resistance.  See the documentation for HashAlgoSecurity
// for more details.
a_cutoff_list!(CollisionResistantHashCutoffList, HashAlgorithm, 12,
               [
                   REJECT,                   // 0. Not assigned.
                   Some(Timestamp::Y1997M2), // 1. MD5
                   Some(Timestamp::Y2013M2), // 2. SHA-1
                   Some(Timestamp::Y2013M2), // 3. RIPE-MD/160
                   REJECT,                   // 4. Reserved.
                   REJECT,                   // 5. Reserved.
                   REJECT,                   // 6. Reserved.
                   REJECT,                   // 7. Reserved.
                   ACCEPT,                   // 8. SHA256
                   ACCEPT,                   // 9. SHA384
                   ACCEPT,                   // 10. SHA512
                   ACCEPT,                   // 11. SHA224
               ]);
// Signatures that *only* require a hash with Second Pre-image
// Resistance.  See the documentation for HashAlgoSecurity for more
// details.
a_cutoff_list!(SecondPreImageResistantHashCutoffList, HashAlgorithm, 12,
               [
                   REJECT,                   // 0. Not assigned.
                   Some(Timestamp::Y2004M2), // 1. MD5
                   Some(Timestamp::Y2023M2), // 2. SHA-1
                   Some(Timestamp::Y2013M2), // 3. RIPE-MD/160
                   REJECT,                   // 4. Reserved.
                   REJECT,                   // 5. Reserved.
                   REJECT,                   // 6. Reserved.
                   REJECT,                   // 7. Reserved.
                   ACCEPT,                   // 8. SHA256
                   ACCEPT,                   // 9. SHA384
                   ACCEPT,                   // 10. SHA512
                   ACCEPT,                   // 11. SHA224
               ]);

a_cutoff_list!(SubpacketTagCutoffList, SubpacketTag, 38,
               [
                   REJECT,                 // 0. Reserved.
                   REJECT,                 // 1. Reserved.
                   ACCEPT,                 // 2. SignatureCreationTime.
                   ACCEPT,                 // 3. SignatureExpirationTime.
                   ACCEPT,                 // 4. ExportableCertification.
                   ACCEPT,                 // 5. TrustSignature.
                   ACCEPT,                 // 6. RegularExpression.
                   // Note: Even though we don't explicitly honor the
                   // Revocable flag, we don't support signature
                   // revocations, hence it is safe to ACCEPT it.
                   ACCEPT,                 // 7. Revocable.
                   REJECT,                 // 8. Reserved.
                   ACCEPT,                 // 9. KeyExpirationTime.
                   REJECT,                 // 10. PlaceholderForBackwardCompatibility.
                   ACCEPT,                 // 11. PreferredSymmetricAlgorithms.
                   ACCEPT,                 // 12. RevocationKey.
                   REJECT,                 // 13. Reserved.
                   REJECT,                 // 14. Reserved.
                   REJECT,                 // 15. Reserved.
                   ACCEPT,                 // 16. Issuer.
                   REJECT,                 // 17. Reserved.
                   REJECT,                 // 18. Reserved.
                   REJECT,                 // 19. Reserved.
                   ACCEPT,                 // 20. NotationData.
                   ACCEPT,                 // 21. PreferredHashAlgorithms.
                   ACCEPT,                 // 22. PreferredCompressionAlgorithms.
                   ACCEPT,                 // 23. KeyServerPreferences.
                   ACCEPT,                 // 24. PreferredKeyServer.
                   ACCEPT,                 // 25. PrimaryUserID.
                   ACCEPT,                 // 26. PolicyURI.
                   ACCEPT,                 // 27. KeyFlags.
                   ACCEPT,                 // 28. SignersUserID.
                   ACCEPT,                 // 29. ReasonForRevocation.
                   ACCEPT,                 // 30. Features.
                   REJECT,                 // 31. SignatureTarget.
                   ACCEPT,                 // 32. EmbeddedSignature.
                   ACCEPT,                 // 33. IssuerFingerprint.
                   ACCEPT,                 // 34. PreferredAEADAlgorithms.
                   ACCEPT,                 // 35. IntendedRecipient.
                   REJECT,                 // 36. Reserved.
                   ACCEPT,                 // 37. AttestedCertifications.
               ]);

a_cutoff_list!(AsymmetricAlgorithmCutoffList, AsymmetricAlgorithm, 18,
               [
                   Some(Timestamp::Y2014M2), // 0. RSA1024.
                   ACCEPT,                   // 1. RSA2048.
                   ACCEPT,                   // 2. RSA3072.
                   ACCEPT,                   // 3. RSA4096.
                   Some(Timestamp::Y2014M2), // 4. ElGamal1024.
                   ACCEPT,                   // 5. ElGamal2048.
                   ACCEPT,                   // 6. ElGamal3072.
                   ACCEPT,                   // 7. ElGamal4096.
                   Some(Timestamp::Y2014M2), // 8. DSA1024.
                   ACCEPT,                   // 9. DSA2048.
                   ACCEPT,                   // 10. DSA3072.
                   ACCEPT,                   // 11. DSA4096.
                   ACCEPT,                   // 12. NistP256.
                   ACCEPT,                   // 13. NistP384.
                   ACCEPT,                   // 14. NistP521.
                   ACCEPT,                   // 15. BrainpoolP256.
                   ACCEPT,                   // 16. BrainpoolP512.
                   ACCEPT,                   // 17. Cv25519.
               ]);

a_cutoff_list!(SymmetricAlgorithmCutoffList, SymmetricAlgorithm, 14,
               [
                   REJECT,                   // 0. Unencrypted.
                   ACCEPT,                   // 1. IDEA.
                   Some(Timestamp::Y2017M2), // 2. TripleDES.
                   ACCEPT,                   // 3. CAST5.
                   ACCEPT,                   // 4. Blowfish.
                   REJECT,                   // 5. Reserved.
                   REJECT,                   // 6. Reserved.
                   ACCEPT,                   // 7. AES128.
                   ACCEPT,                   // 8. AES192.
                   ACCEPT,                   // 9. AES256.
                   ACCEPT,                   // 10. Twofish.
                   ACCEPT,                   // 11. Camellia128.
                   ACCEPT,                   // 12. Camellia192.
                   ACCEPT,                   // 13. Camellia256.
               ]);

a_cutoff_list!(AEADAlgorithmCutoffList, AEADAlgorithm, 3,
               [
                   REJECT,                 // 0. Reserved.
                   ACCEPT,                 // 1. EAX.
                   ACCEPT,                 // 2. OCB.
               ]);

a_versioned_cutoff_list!(PacketTagCutoffList, Tag, 21,
    [
        REJECT,                   // 0. Reserved.
        ACCEPT,                   // 1. PKESK.
        ACCEPT,                   // 2. Signature.
        ACCEPT,                   // 3. SKESK.
        ACCEPT,                   // 4. OnePassSig.
        ACCEPT,                   // 5. SecretKey.
        ACCEPT,                   // 6. PublicKey.
        ACCEPT,                   // 7. SecretSubkey.
        ACCEPT,                   // 8. CompressedData.
        Some(Timestamp::Y2004M2), // 9. SED.
        ACCEPT,                   // 10. Marker.
        ACCEPT,                   // 11. Literal.
        ACCEPT,                   // 12. Trust.
        ACCEPT,                   // 13. UserID.
        ACCEPT,                   // 14. PublicSubkey.
        REJECT,                   // 15. Not assigned.
        REJECT,                   // 16. Not assigned.
        ACCEPT,                   // 17. UserAttribute.
        ACCEPT,                   // 18. SEIP.
        ACCEPT,                   // 19. MDC.
        ACCEPT,                   // 20. AED.
    ],
    // The versioned list overrides the unversioned list.  So we only
    // need to tweak the above.
    //
    // Note: this list must be sorted and the tag and version must be unique!
    1,
    [
        (Tag::Signature, 3, Some(Timestamp::Y2007M2)),
    ]);

// We need to convert a `SystemTime` to a `Timestamp` in
// `StandardPolicy::reject_hash_at`.  Unfortunately, a `SystemTime`
// can represent a larger range of time than a `Timestamp` can.  Since
// the times passed to this function are cutoff points, and we only
// compare them to OpenPGP timestamps, any `SystemTime` that is prior
// to the Unix Epoch is equivalent to the Unix Epoch: it will reject
// all timestamps.  Similarly, any `SystemTime` that is later than the
// latest time representable by a `Timestamp` is equivalent to
// accepting all time stamps, which is equivalent to passing None.
fn system_time_cutoff_to_timestamp(t: SystemTime) -> Option<Timestamp> {
    let t = t
        .duration_since(SystemTime::UNIX_EPOCH)
        // An error can only occur if the SystemTime is less than the
        // reference time (SystemTime::UNIX_EPOCH).  Map that to
        // SystemTime::UNIX_EPOCH, as above.
        .unwrap_or_else(|_| Duration::new(0, 0));
    let t = t.as_secs();
    if t > u32::MAX as u64 {
        // Map to None, as above.
        None
    } else {
        Some((t as u32).into())
    }
}

impl<'a> StandardPolicy<'a> {
    /// Instantiates a new `StandardPolicy` with the default parameters.
    pub const fn new() -> Self {
        const EMPTY_LIST: &[&str] = &[];
        Self {
            time: None,
            collision_resistant_hash_algos:
                CollisionResistantHashCutoffList::Default(),
            second_pre_image_resistant_hash_algos:
                SecondPreImageResistantHashCutoffList::Default(),
            // There are 365.2425 days in a year.  Use a reasonable
            // approximation.
            hash_revocation_tolerance:
                types::Duration::seconds((7 * 365 + 2) * 24 * 60 * 60),
            critical_subpackets: SubpacketTagCutoffList::Default(),
            good_critical_notations: EMPTY_LIST,
            asymmetric_algos: AsymmetricAlgorithmCutoffList::Default(),
            symmetric_algos: SymmetricAlgorithmCutoffList::Default(),
            aead_algos: AEADAlgorithmCutoffList::Default(),
            packet_tags: PacketTagCutoffList::Default(),
        }
    }

    /// Instantiates a new `StandardPolicy` with parameters
    /// appropriate for `time`.
    ///
    /// `time` is a meta-parameter that selects a security profile
    /// that is appropriate for the given point in time.  When
    /// evaluating an object, the reference time should be set to the
    /// time that the object was stored to non-tamperable storage.
    /// Since most applications don't record when they received an
    /// object, they should conservatively use the current time.
    ///
    /// Note that the reference time is a security parameter and is
    /// different from the time that the object was allegedly created.
    /// Consider evaluating a signature whose `Signature Creation
    /// Time` subpacket indicates that it was created in 2007.  Since
    /// the subpacket is under the control of the sender, setting the
    /// reference time according to the subpacket means that the
    /// sender chooses the security profile.  If the sender were an
    /// attacker, she could have forged this to take advantage of
    /// security weaknesses found since 2007.  This is why the
    /// reference time must be set---at the earliest---to the time
    /// that the message was stored to non-tamperable storage.  When
    /// that is not available, the current time should be used.
    pub fn at<T>(time: T) -> Self
        where T: Into<SystemTime>,
    {
        let time = time.into();
        let mut p = Self::new();
        p.time = Some(system_time_cutoff_to_timestamp(time)
                          // Map "ACCEPT" to the end of time (None
                          // here means the current time).
                          .unwrap_or(Timestamp::MAX));
        p
    }

    /// Returns the policy's reference time.
    ///
    /// The current time is None.
    ///
    /// See [`StandardPolicy::at`] for details.
    ///
    /// [`StandardPolicy::at`]: StandardPolicy::at()
    pub fn time(&self) -> Option<SystemTime> {
        self.time.map(Into::into)
    }

    /// Always considers `h` to be secure.
    ///
    /// A cryptographic hash algorithm normally has three security
    /// properties:
    ///
    ///   - Pre-image resistance,
    ///   - Second pre-image resistance, and
    ///   - Collision resistance.
    ///
    /// A hash algorithm should only be unconditionally accepted if it
    /// has all three of these properties.  See the documentation for
    /// [`HashAlgoSecurity`] for more details.
    ///
    pub fn accept_hash(&mut self, h: HashAlgorithm) {
        self.collision_resistant_hash_algos.set(h, ACCEPT);
        self.second_pre_image_resistant_hash_algos.set(h, ACCEPT);
    }

    /// Considers `h` to be insecure in all security contexts.
    ///
    /// A cryptographic hash algorithm normally has three security
    /// properties:
    ///
    ///   - Pre-image resistance,
    ///   - Second pre-image resistance, and
    ///   - Collision resistance.
    ///
    /// This method causes the hash algorithm to be considered unsafe
    /// in all security contexts.
    ///
    /// See the documentation for [`HashAlgoSecurity`] for more
    /// details.
    ///
    ///
    /// To express a more nuanced policy, use
    /// [`StandardPolicy::reject_hash_at`] or
    /// [`StandardPolicy::reject_hash_property_at`].
    ///
    ///   [`StandardPolicy::reject_hash_at`]: StandardPolicy::reject_hash_at()
    ///   [`StandardPolicy::reject_hash_property_at`]: StandardPolicy::reject_hash_property_at()
    pub fn reject_hash(&mut self, h: HashAlgorithm) {
        self.collision_resistant_hash_algos.set(h, REJECT);
        self.second_pre_image_resistant_hash_algos.set(h, REJECT);
    }

    /// Considers all hash algorithms to be insecure.
    ///
    /// Causes all hash algorithms to be considered insecure in all
    /// security contexts.
    ///
    /// This is useful when using a good list to determine what
    /// algorithms are allowed.
    pub fn reject_all_hashes(&mut self) {
        self.collision_resistant_hash_algos.reject_all();
        self.second_pre_image_resistant_hash_algos.reject_all();
    }

    /// Considers `h` to be insecure in all security contexts starting
    /// at time `t`.
    ///
    /// A cryptographic hash algorithm normally has three security
    /// properties:
    ///
    ///   - Pre-image resistance,
    ///   - Second pre-image resistance, and
    ///   - Collision resistance.
    ///
    /// This method causes the hash algorithm to be considered unsafe
    /// in all security contexts starting at time `t`.
    ///
    /// See the documentation for [`HashAlgoSecurity`] for more
    /// details.
    ///
    ///
    /// To express a more nuanced policy, use
    /// [`StandardPolicy::reject_hash_property_at`].
    ///
    ///   [`StandardPolicy::reject_hash_property_at`]: StandardPolicy::reject_hash_property_at()
    pub fn reject_hash_at<T>(&mut self, h: HashAlgorithm, t: T)
        where T: Into<Option<SystemTime>>,
    {
        let t = t.into().and_then(system_time_cutoff_to_timestamp);
        self.collision_resistant_hash_algos.set(h, t);
        self.second_pre_image_resistant_hash_algos.set(h, t);
    }

    /// Considers `h` to be insecure starting at `t` for the specified
    /// security property.
    ///
    /// A hash algorithm is considered secure if it has all of the
    /// following security properties:
    ///
    ///   - Pre-image resistance,
    ///   - Second pre-image resistance, and
    ///   - Collision resistance.
    ///
    /// Some contexts only require a subset of these security
    /// properties.  Specifically, if an attacker is unable to
    /// influence the data that a user signs, then the hash algorithm
    /// only needs second pre-image resistance; it doesn't need
    /// collision resistance.  See the documentation for
    /// [`HashAlgoSecurity`] for more details.
    ///
    ///
    /// This method makes it possible to specify different policies
    /// depending on the security requirements.
    ///
    /// A cutoff of `None` means that there is no cutoff and the
    /// algorithm has no known vulnerabilities for the specified
    /// security policy.
    ///
    /// As a rule of thumb, collision resistance is easier to attack
    /// than second pre-image resistance.  And in practice there are
    /// practical attacks against several widely-used hash algorithms'
    /// collision resistance, but only theoretical attacks against
    /// their second pre-image resistance.  Nevertheless, once one
    /// property of a hash has been compromised, we want to deprecate
    /// its use as soon as it is feasible.  Unfortunately, because
    /// OpenPGP certificates are long-lived, this can take years.
    ///
    /// Given this, we start rejecting [MD5] in cases where collision
    /// resistance is required in 1997 and completely reject it
    /// starting in 2004:
    ///
    /// >  In 1996, Dobbertin announced a collision of the
    /// >  compression function of MD5 (Dobbertin, 1996). While this
    /// >  was not an attack on the full MD5 hash function, it was
    /// >  close enough for cryptographers to recommend switching to
    /// >  a replacement, such as SHA-1 or RIPEMD-160.
    /// >
    /// >  MD5CRK ended shortly after 17 August 2004, when collisions
    /// >  for the full MD5 were announced by Xiaoyun Wang, Dengguo
    /// >  Feng, Xuejia Lai, and Hongbo Yu. Their analytical attack
    /// >  was reported to take only one hour on an IBM p690 cluster.
    /// >
    /// > (Accessed Feb. 2020.)
    ///
    ///   [MD5]: https://en.wikipedia.org/wiki/MD5
    ///
    /// And we start rejecting [SHA-1] in cases where collision
    /// resistance is required in 2013, and completely reject it in
    /// 2023:
    ///
    /// > Since 2005 SHA-1 has not been considered secure against
    /// > well-funded opponents, as of 2010 many organizations have
    /// > recommended its replacement. NIST formally deprecated use
    /// > of SHA-1 in 2011 and disallowed its use for digital
    /// > signatures in 2013. As of 2020, attacks against SHA-1 are
    /// > as practical as against MD5; as such, it is recommended to
    /// > remove SHA-1 from products as soon as possible and use
    /// > instead SHA-256 or SHA-3. Replacing SHA-1 is urgent where
    /// > it's used for signatures.
    /// >
    /// > (Accessed Feb. 2020.)
    ///
    ///   [SHA-1]: https://en.wikipedia.org/wiki/SHA-1
    ///
    /// There are two main reasons why we have decided to accept SHA-1
    /// for so long.  First, as of the end of 2020, there are still a
    /// large number of [certificates that rely on SHA-1].  Second,
    /// Sequoia uses a variant of SHA-1 called [SHA1CD], which is able
    /// to detect and *mitigate* the known attacks on SHA-1's
    /// collision resistance.
    ///
    ///   [certificates that rely on SHA-1]: https://gitlab.com/sequoia-pgp/sequoia/-/issues/595
    ///   [SHA1CD]: https://github.com/cr-marcstevens/sha1collisiondetection
    ///
    /// Since RIPE-MD is structured similarly to SHA-1, we
    /// conservatively consider it to be broken as well.  But, because
    /// it is not widely used in the OpenPGP ecosystem, we don't make
    /// provisions for it.
    ///
    /// Note: if a context indicates that it requires collision
    /// resistance, then it requires both collision resistance and
    /// second pre-image resistance, and both policies must indicate
    /// that the hash algorithm can be safely used at the specified
    /// time.
    pub fn reject_hash_property_at<T>(&mut self, h: HashAlgorithm,
                                      sec: HashAlgoSecurity, t: T)
        where T: Into<Option<SystemTime>>,
    {
        let t = t.into().and_then(system_time_cutoff_to_timestamp);
        match sec {
            HashAlgoSecurity::CollisionResistance =>
                self.collision_resistant_hash_algos.set(h, t),
            HashAlgoSecurity::SecondPreImageResistance =>
                self.second_pre_image_resistant_hash_algos.set(h, t),
        }
    }

    /// Returns the cutoff time for the specified hash algorithm and
    /// security policy.
    pub fn hash_cutoff(&self, h: HashAlgorithm, sec: HashAlgoSecurity)
        -> Option<SystemTime>
    {
        match sec {
            HashAlgoSecurity::CollisionResistance =>
                self.collision_resistant_hash_algos.cutoff(h),
            HashAlgoSecurity::SecondPreImageResistance =>
                self.second_pre_image_resistant_hash_algos.cutoff(h),
        }.map(|t| t.into())
    }

    /// Sets the amount of time to continue to accept revocation
    /// certificates after a hash algorithm should be rejected.
    ///
    /// Using [`StandardPolicy::reject_hash_at`], it is possible to
    /// indicate when a hash algorithm's security has been
    /// compromised, and, as such, should no longer be accepted.
    ///
    ///   [`StandardPolicy::reject_hash_at`]: StandardPolicy::reject_hash_at()
    ///
    /// Applying this policy to revocation certificates can have some
    /// unfortunate side effects.  In particular, if a certificate has
    /// been revoked using a revocation certificate that relies on a
    /// broken hash algorithm, but the most recent self signature uses
    /// a strong acceptable hash algorithm, then rejecting the
    /// revocation certificate would mean considering the certificate
    /// to not be revoked!  This would be a catastrophe if the secret
    /// key material were compromised.
    ///
    /// Unfortunately, this happens in practice.  A common example
    /// appears to be a certificate that has been updated many times,
    /// and is then revoked using a revocation certificate that was
    /// generated when the certificate was generated.
    ///
    /// Since the consequences of allowing an invalid revocation
    /// certificate are significantly less severe (a denial of
    /// service) than ignoring a valid revocation certificate
    /// (compromised confidentiality, integrity, and authentication),
    /// this option makes it possible to accept revocations using weak
    /// hash algorithms longer than other types of signatures.
    ///
    /// By default, the standard policy accepts revocation
    /// certificates seven years after the hash they are using was
    /// initially compromised.
    pub fn hash_revocation_tolerance<D>(&mut self, d: D)
        where D: Into<types::Duration>
    {
        self.hash_revocation_tolerance = d.into();
    }

    /// Sets the amount of time to continue to accept revocation
    /// certificates after a hash algorithm should be rejected.
    ///
    /// See [`StandardPolicy::hash_revocation_tolerance`] for details.
    ///
    ///   [`StandardPolicy::hash_revocation_tolerance`]: StandardPolicy::hash_revocation_tolerance()
    pub fn get_hash_revocation_tolerance(&self) -> types::Duration {
        self.hash_revocation_tolerance
    }

    /// Always considers `s` to be secure.
    pub fn accept_critical_subpacket(&mut self, s: SubpacketTag) {
        self.critical_subpackets.set(s, ACCEPT);
    }

    /// Always considers `s` to be insecure.
    pub fn reject_critical_subpacket(&mut self, s: SubpacketTag) {
        self.critical_subpackets.set(s, REJECT);
    }

    /// Considers all critical subpackets to be insecure.
    ///
    /// This is useful when using a good list to determine what
    /// critical subpackets are allowed.
    pub fn reject_all_critical_subpackets(&mut self) {
        self.critical_subpackets.reject_all();
    }

    /// Considers `s` to be insecure starting at `cutoff`.
    ///
    /// A cutoff of `None` means that there is no cutoff and the
    /// subpacket has no known vulnerabilities.
    ///
    /// By default, we accept all critical subpackets that Sequoia
    /// understands and honors.
    pub fn reject_critical_subpacket_at<C>(&mut self, s: SubpacketTag,
                                       cutoff: C)
        where C: Into<Option<SystemTime>>,
    {
        self.critical_subpackets.set(
            s,
            cutoff.into().and_then(system_time_cutoff_to_timestamp));
    }

    /// Returns the cutoff times for the specified subpacket tag.
    pub fn critical_subpacket_cutoff(&self, s: SubpacketTag)
                                 -> Option<SystemTime> {
        self.critical_subpackets.cutoff(s).map(|t| t.into())
    }

    /// Sets the list of accepted critical notations.
    ///
    /// By default, we reject all critical notations.
    pub fn good_critical_notations(&mut self, good_list: &'a [&'a str]) {
        self.good_critical_notations = good_list;
    }

    /// Always considers `s` to be secure.
    pub fn accept_asymmetric_algo(&mut self, a: AsymmetricAlgorithm) {
        self.asymmetric_algos.set(a, ACCEPT);
    }

    /// Always considers `s` to be insecure.
    pub fn reject_asymmetric_algo(&mut self, a: AsymmetricAlgorithm) {
        self.asymmetric_algos.set(a, REJECT);
    }

    /// Considers all asymmetric algorithms to be insecure.
    ///
    /// This is useful when using a good list to determine what
    /// algorithms are allowed.
    pub fn reject_all_asymmetric_algos(&mut self) {
        self.asymmetric_algos.reject_all();
    }

    /// Considers `a` to be insecure starting at `cutoff`.
    ///
    /// A cutoff of `None` means that there is no cutoff and the
    /// algorithm has no known vulnerabilities.
    ///
    /// By default, we reject the use of asymmetric key sizes lower
    /// than 2048 bits starting in 2014 following [NIST Special
    /// Publication 800-131A].
    ///
    ///   [NIST Special Publication 800-131A]: https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-131Ar2.pdf
    pub fn reject_asymmetric_algo_at<C>(&mut self, a: AsymmetricAlgorithm,
                                       cutoff: C)
        where C: Into<Option<SystemTime>>,
    {
        self.asymmetric_algos.set(
            a,
            cutoff.into().and_then(system_time_cutoff_to_timestamp));
    }

    /// Returns the cutoff times for the specified hash algorithm.
    pub fn asymmetric_algo_cutoff(&self, a: AsymmetricAlgorithm)
                                 -> Option<SystemTime> {
        self.asymmetric_algos.cutoff(a).map(|t| t.into())
    }

    /// Always considers `s` to be secure.
    pub fn accept_symmetric_algo(&mut self, s: SymmetricAlgorithm) {
        self.symmetric_algos.set(s, ACCEPT);
    }

    /// Always considers `s` to be insecure.
    pub fn reject_symmetric_algo(&mut self, s: SymmetricAlgorithm) {
        self.symmetric_algos.set(s, REJECT);
    }

    /// Considers all symmetric algorithms to be insecure.
    ///
    /// This is useful when using a good list to determine what
    /// algorithms are allowed.
    pub fn reject_all_symmetric_algos(&mut self) {
        self.symmetric_algos.reject_all();
    }

    /// Considers `s` to be insecure starting at `cutoff`.
    ///
    /// A cutoff of `None` means that there is no cutoff and the
    /// algorithm has no known vulnerabilities.
    ///
    /// By default, we reject the use of TripleDES (3DES) starting in
    /// the year 2017.  While 3DES is still a ["MUST implement"]
    /// algorithm in RFC4880, released in 2007, there are plenty of
    /// other symmetric algorithms defined in RFC4880, and it says
    /// AES-128 SHOULD be implemented.  Support for other algorithms
    /// in OpenPGP implementations is [excellent].  We chose 2017 as
    /// the cutoff year because [NIST deprecated 3DES] that year.
    ///
    ///   ["MUST implement"]: https://tools.ietf.org/html/rfc4880#section-9.2
    ///   [excellent]: https://tests.sequoia-pgp.org/#Symmetric_Encryption_Algorithm_support
    ///   [NIST deprecated 3DES]: https://csrc.nist.gov/News/2017/Update-to-Current-Use-and-Deprecation-of-TDEA
    pub fn reject_symmetric_algo_at<C>(&mut self, s: SymmetricAlgorithm,
                                       cutoff: C)
        where C: Into<Option<SystemTime>>,
    {
        self.symmetric_algos.set(
            s,
            cutoff.into().and_then(system_time_cutoff_to_timestamp));
    }

    /// Returns the cutoff times for the specified hash algorithm.
    pub fn symmetric_algo_cutoff(&self, s: SymmetricAlgorithm)
                                 -> Option<SystemTime> {
        self.symmetric_algos.cutoff(s).map(|t| t.into())
    }

    /// Always considers `s` to be secure.
    ///
    /// This feature is [experimental](super#experimental-features).
    pub fn accept_aead_algo(&mut self, a: AEADAlgorithm) {
        self.aead_algos.set(a, ACCEPT);
    }

    /// Always considers `s` to be insecure.
    ///
    /// This feature is [experimental](super#experimental-features).
    pub fn reject_aead_algo(&mut self, a: AEADAlgorithm) {
        self.aead_algos.set(a, REJECT);
    }

    /// Considers all AEAD algorithms to be insecure.
    ///
    /// This is useful when using a good list to determine what
    /// algorithms are allowed.
    pub fn reject_all_aead_algos(&mut self) {
        self.aead_algos.reject_all();
    }

    /// Considers `a` to be insecure starting at `cutoff`.
    ///
    /// A cutoff of `None` means that there is no cutoff and the
    /// algorithm has no known vulnerabilities.
    ///
    /// By default, we accept all AEAD modes.
    ///
    /// This feature is [experimental](super#experimental-features).
    pub fn reject_aead_algo_at<C>(&mut self, a: AEADAlgorithm,
                                       cutoff: C)
        where C: Into<Option<SystemTime>>,
    {
        self.aead_algos.set(
            a,
            cutoff.into().and_then(system_time_cutoff_to_timestamp));
    }

    /// Returns the cutoff times for the specified hash algorithm.
    ///
    /// This feature is [experimental](super#experimental-features).
    pub fn aead_algo_cutoff(&self, a: AEADAlgorithm)
                                 -> Option<SystemTime> {
        self.aead_algos.cutoff(a).map(|t| t.into())
    }

    /// Always accept the specified version of the packet.
    ///
    /// If a packet does not have a version field, then its version is
    /// `0`.
    pub fn accept_packet_tag_version(&mut self, tag: Tag, version: u8) {
        self.packet_tags.set_versioned(tag, version, ACCEPT);
    }

    /// Always accept packets with the given tag independent of their
    /// version.
    ///
    /// If you previously set a cutoff for a specific version of a
    /// packet, this overrides that.
    pub fn accept_packet_tag(&mut self, tag: Tag) {
        self.packet_tags.set_unversioned(tag, ACCEPT);
    }

    /// Always reject the specified version of the packet.
    ///
    /// If a packet does not have a version field, then its version is
    /// `0`.
    pub fn reject_packet_tag_version(&mut self, tag: Tag, version: u8) {
        self.packet_tags.set_versioned(tag, version, REJECT);
    }

    /// Always reject packets with the given tag.
    pub fn reject_packet_tag(&mut self, tag: Tag) {
        self.packet_tags.set_unversioned(tag, REJECT);
    }

    /// Considers all packets to be insecure.
    ///
    /// This is useful when using a good list to determine what
    /// packets are allowed.
    pub fn reject_all_packet_tags(&mut self) {
        self.packet_tags.reject_all();
    }

    /// Start rejecting the specified version of packets with the
    /// given tag at `t`.
    ///
    /// A cutoff of `None` means that there is no cutoff and the
    /// packet has no known vulnerabilities.
    ///
    /// By default, we consider the *Symmetrically Encrypted Data
    /// Packet* (SED) insecure in messages created in the year 2004 or
    /// later.  The rationale here is that *Symmetrically Encrypted
    /// Integrity Protected Data Packet* (SEIP) can be downgraded to
    /// SED packets, enabling attacks exploiting the malleability of
    /// the CFB stream (see [EFAIL]).
    ///
    ///   [EFAIL]: https://en.wikipedia.org/wiki/EFAIL
    ///
    /// We chose 2004 as a cutoff-date because [Debian 3.0] (Woody),
    /// released on 2002-07-19, was the first release of Debian to
    /// ship a version of GnuPG that emitted SEIP packets by default.
    /// The first version that emitted SEIP packets was [GnuPG 1.0.3],
    /// released on 2000-09-18.  Mid 2002 plus a 18 months grace
    /// period of people still using older versions is 2004.
    ///
    ///   [Debian 3.0]: https://www.debian.org/News/2002/20020719
    ///   [GnuPG 1.0.3]: https://lists.gnupg.org/pipermail/gnupg-announce/2000q3/000075.html
    pub fn reject_packet_tag_version_at<C>(&mut self, tag: Tag, version: u8,
                                           cutoff: C)
        where C: Into<Option<SystemTime>>,
    {
        self.packet_tags.set_versioned(
            tag, version,
            cutoff.into().and_then(system_time_cutoff_to_timestamp));
    }

    /// Start rejecting packets with the given tag at `t`.
    ///
    /// See the documentation for
    /// [`StandardPolicy::reject_packet_tag_version_at`].
    pub fn reject_packet_tag_at<C>(&mut self, tag: Tag, cutoff: C)
        where C: Into<Option<SystemTime>>,
    {
        self.packet_tags.set_unversioned(
            tag,
            cutoff.into().and_then(system_time_cutoff_to_timestamp));
    }

    /// Returns the cutoff for the specified version of the specified
    /// packet tag.
    ///
    /// This first considers the versioned cutoff list.  If there is
    /// no entry in the versioned list, it fallsback to the
    /// unversioned cutoff list.  If there is also no entry there,
    /// then it falls back to the default.
    pub fn packet_tag_version_cutoff(&self, tag: Tag, version: u8)
        -> Option<SystemTime>
    {
        self.packet_tags.cutoff(tag, version).map(|t| t.into())
    }

    /// Returns the cutoff time for the specified packet tag.
    ///
    /// This function returns the maximum cutoff for all versions of
    /// the packet.  That is, if one version has a cutoff of `t1`, and
    /// another version has a cutoff of `t2`, this returns `max(t1,
    /// t2)`.  These semantics answer the question: "Up to which point
    /// can we use this packet?"
    #[deprecated(note = "Since 1.11.  Use `packet_tag_version_cutoff`.")]
    pub fn packet_tag_cutoff(&self, tag: Tag) -> Option<SystemTime> {
        // Versioned policy.
        self.packet_tags.versioned_cutoffs
            .iter()
            .filter_map(|(t, _v, cutoff)| {
                if t == &tag {
                    Some(cutoff)
                } else {
                    None
                }
            })
            // Unversioned policy or default, if nont.
            .chain(
                std::iter::once(
                    self.packet_tags.unversioned_cutoffs.get(
                        u8::from(tag) as usize)
                        .unwrap_or(&cutofflist::DEFAULT_POLICY)))
            // Prefer None.
            .max_by(|a, b| a.is_none().cmp(&b.is_none()).then(a.cmp(b)))
            .expect("have one")
            .map(Into::into)
    }
}

impl<'a> Policy for StandardPolicy<'a> {
    fn signature(&self, sig: &Signature, sec: HashAlgoSecurity) -> Result<()> {
        let time = self.time.unwrap_or_else(Timestamp::now);

        let rev = matches!(sig.typ(), SignatureType::KeyRevocation
                | SignatureType::SubkeyRevocation
                | SignatureType::CertificationRevocation);

        // Note: collision resistance requires 2nd pre-image resistance.
        if sec == HashAlgoSecurity::CollisionResistance {
            if rev {
                self
                    .collision_resistant_hash_algos
                    .check(sig.hash_algo(), time,
                           Some(self.hash_revocation_tolerance))
                    .context(format!(
                        "Policy rejected revocation signature ({}) requiring \
                         collision resistance", sig.typ()))?
            } else {
                self
                    .collision_resistant_hash_algos
                    .check(sig.hash_algo(), time, None)
                    .context(format!(
                        "Policy rejected non-revocation signature ({}) requiring \
                         collision resistance", sig.typ()))?
            }
        }

        if rev {
            self
                .second_pre_image_resistant_hash_algos
                .check(sig.hash_algo(), time,
                       Some(self.hash_revocation_tolerance))
                .context(format!(
                    "Policy rejected revocation signature ({}) requiring \
                     second pre-image resistance", sig.typ()))?
        } else {
            self
                .second_pre_image_resistant_hash_algos
                .check(sig.hash_algo(), time, None)
                .context(format!(
                    "Policy rejected non-revocation signature ({}) requiring \
                     second pre-image resistance", sig.typ()))?
        }

        for csp in sig.hashed_area().iter().filter(|sp| sp.critical()) {
            self.critical_subpackets.check(csp.tag(), time, None)
                .context("Policy rejected critical signature subpacket")?;
            if let SubpacketValue::NotationData(n) = csp.value() {
                if ! self.good_critical_notations.contains(&n.name()) {
                    return Err(anyhow::Error::from(
                        Error::PolicyViolation(
                            format!("Critical notation {:?}",
                                    n.name()), None))
                               .context("Policy rejected critical notation"));
                }
            }
        }

        Ok(())
    }

    fn key(&self, ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
        -> Result<()>
    {
        use self::AsymmetricAlgorithm::{*, Unknown};
        use crate::types::PublicKeyAlgorithm::*;
        use crate::crypto::mpi::PublicKey;

        #[allow(deprecated)]
        let a = match (ka.pk_algo(), ka.mpis().bits()) {
            // RSA.
            (RSAEncryptSign, Some(b))
                | (RSAEncrypt, Some(b))
                | (RSASign, Some(b)) if b < 2048 => RSA1024,
            (RSAEncryptSign, Some(b))
                | (RSAEncrypt, Some(b))
                | (RSASign, Some(b)) if b < 3072 => RSA2048,
            (RSAEncryptSign, Some(b))
                | (RSAEncrypt, Some(b))
                | (RSASign, Some(b)) if b < 4096 => RSA3072,
            (RSAEncryptSign, Some(_))
                | (RSAEncrypt, Some(_))
                | (RSASign, Some(_)) => RSA4096,
            (RSAEncryptSign, None)
                | (RSAEncrypt, None)
                | (RSASign, None) => unreachable!(),

            // ElGamal.
            (ElGamalEncryptSign, Some(b))
                | (ElGamalEncrypt, Some(b)) if b < 2048 => ElGamal1024,
            (ElGamalEncryptSign, Some(b))
                | (ElGamalEncrypt, Some(b)) if b < 3072 => ElGamal2048,
            (ElGamalEncryptSign, Some(b))
                | (ElGamalEncrypt, Some(b)) if b < 4096 => ElGamal3072,
            (ElGamalEncryptSign, Some(_))
                | (ElGamalEncrypt, Some(_)) => ElGamal4096,
            (ElGamalEncryptSign, None)
                | (ElGamalEncrypt, None) => unreachable!(),

            // DSA.
            (DSA, Some(b)) if b < 2048 => DSA1024,
            (DSA, Some(b)) if b < 3072 => DSA2048,
            (DSA, Some(b)) if b < 4096 => DSA3072,
            (DSA, Some(_)) => DSA4096,
            (DSA, None) => unreachable!(),

            // ECC.
            (ECDH, _) | (ECDSA, _) | (EdDSA, _) => {
                let curve = match ka.mpis() {
                    PublicKey::EdDSA { curve, .. } => curve,
                    PublicKey::ECDSA { curve, .. } => curve,
                    PublicKey::ECDH { curve, .. } => curve,
                    _ => unreachable!(),
                };
                use crate::types::Curve;
                match curve {
                    Curve::NistP256 => NistP256,
                    Curve::NistP384 => NistP384,
                    Curve::NistP521 => NistP521,
                    Curve::BrainpoolP256 => BrainpoolP256,
                    Curve::BrainpoolP512 => BrainpoolP512,
                    Curve::Ed25519 => Cv25519,
                    Curve::Cv25519 => Cv25519,
                    Curve::Unknown(_) => Unknown,
                }
            },

            _ => Unknown,
        };

        let time = self.time.unwrap_or_else(Timestamp::now);
        self.asymmetric_algos.check(a, time, None)
            .context("Policy rejected asymmetric algorithm")?;

        // Check ECDH KDF and KEK parameters.
        if let PublicKey::ECDH { hash, sym, .. } = ka.mpis() {
            self.symmetric_algorithm(*sym)
                .context("Policy rejected ECDH \
                          key encapsulation algorithm")?;

            // RFC6637 says:
            //
            // > Refer to Section 13 for the details regarding the
            // > choice of the KEK algorithm, which SHOULD be one of
            // > three AES algorithms.
            //
            // Furthermore, GnuPG rejects anything other than AES.
            // I checked the SKS dump, and there are no keys out
            // there that use a different KEK algorithm.
            match sym {
                SymmetricAlgorithm::AES128
                    | SymmetricAlgorithm::AES192
                    | SymmetricAlgorithm::AES256
                    => (), // Good.
                _ =>
                    return Err(anyhow::Error::from(
                        Error::PolicyViolation(sym.to_string(), None))
                               .context("Policy rejected ECDH \
                                         key encapsulation algorithm")),
            }

            // For use in a KDF the hash algorithm does not
            // necessarily be collision resistant, but this is the
            // weakest property that we otherwise care for, so
            // (somewhat arbitrarily) use this.
            self
                .collision_resistant_hash_algos
                .check(*hash, time, None)
                .context("Policy rejected ECDH \
                          key derivation hash function")?;
        }

        Ok(())
    }

    fn packet(&self, packet: &Packet) -> Result<()> {
        let time = self.time.unwrap_or_else(Timestamp::now);
        self.packet_tags
            .check(
                packet.tag(),
                packet.version().unwrap_or(0),
                time, None)
            .context("Policy rejected packet type")
    }

    fn symmetric_algorithm(&self, algo: SymmetricAlgorithm) -> Result<()> {
        let time = self.time.unwrap_or_else(Timestamp::now);
        self.symmetric_algos.check(algo, time, None)
            .context("Policy rejected symmetric encryption algorithm")
    }

    fn aead_algorithm(&self, algo: AEADAlgorithm) -> Result<()> {
        let time = self.time.unwrap_or_else(Timestamp::now);
        self.aead_algos.check(algo, time, None)
            .context("Policy rejected authenticated encryption algorithm")
    }
}

/// Asymmetric encryption algorithms.
///
/// This type is for refining the [`StandardPolicy`] with respect to
/// asymmetric algorithms.  In contrast to [`PublicKeyAlgorithm`], it
/// does not concern itself with the use (encryption or signing), and
/// it does include key sizes (if applicable) and elliptic curves.
///
///   [`PublicKeyAlgorithm`]: crate::types::PublicKeyAlgorithm
///
/// Key sizes put into are buckets, rounding down to the nearest
/// bucket.  For example, a 3253-bit RSA key is categorized as
/// `RSA3072`.
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum AsymmetricAlgorithm {
    /// RSA with key sizes up to 2048-1 bit.
    RSA1024,
    /// RSA with key sizes up to 3072-1 bit.
    RSA2048,
    /// RSA with key sizes up to 4096-1 bit.
    RSA3072,
    /// RSA with key sizes larger or equal to 4096 bit.
    RSA4096,
    /// ElGamal with key sizes up to 2048-1 bit.
    ElGamal1024,
    /// ElGamal with key sizes up to 3072-1 bit.
    ElGamal2048,
    /// ElGamal with key sizes up to 4096-1 bit.
    ElGamal3072,
    /// ElGamal with key sizes larger or equal to 4096 bit.
    ElGamal4096,
    /// DSA with key sizes up to 2048-1 bit.
    DSA1024,
    /// DSA with key sizes up to 3072-1 bit.
    DSA2048,
    /// DSA with key sizes up to 4096-1 bit.
    DSA3072,
    /// DSA with key sizes larger or equal to 4096 bit.
    DSA4096,
    /// NIST curve P-256.
    NistP256,
    /// NIST curve P-384.
    NistP384,
    /// NIST curve P-521.
    NistP521,
    /// brainpoolP256r1.
    BrainpoolP256,
    /// brainpoolP512r1.
    BrainpoolP512,
    /// D.J. Bernstein's Curve25519.
    Cv25519,
    /// Unknown algorithm.
    Unknown,
}
assert_send_and_sync!(AsymmetricAlgorithm);

const ASYMMETRIC_ALGORITHM_VARIANTS: [AsymmetricAlgorithm; 18] = [
    AsymmetricAlgorithm::RSA1024,
    AsymmetricAlgorithm::RSA2048,
    AsymmetricAlgorithm::RSA3072,
    AsymmetricAlgorithm::RSA4096,
    AsymmetricAlgorithm::ElGamal1024,
    AsymmetricAlgorithm::ElGamal2048,
    AsymmetricAlgorithm::ElGamal3072,
    AsymmetricAlgorithm::ElGamal4096,
    AsymmetricAlgorithm::DSA1024,
    AsymmetricAlgorithm::DSA2048,
    AsymmetricAlgorithm::DSA3072,
    AsymmetricAlgorithm::DSA4096,
    AsymmetricAlgorithm::NistP256,
    AsymmetricAlgorithm::NistP384,
    AsymmetricAlgorithm::NistP521,
    AsymmetricAlgorithm::BrainpoolP256,
    AsymmetricAlgorithm::BrainpoolP512,
    AsymmetricAlgorithm::Cv25519,
];

impl AsymmetricAlgorithm {
    /// Returns an iterator over all valid variants.
    ///
    /// Returns an iterator over all known variants.  This does not
    /// include the [`AsymmetricAlgorithm::Unknown`] variant.
    pub fn variants() -> impl Iterator<Item=AsymmetricAlgorithm> {
        ASYMMETRIC_ALGORITHM_VARIANTS.iter().cloned()
    }
}

impl std::fmt::Display for AsymmetricAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<AsymmetricAlgorithm> for u8 {
    fn from(a: AsymmetricAlgorithm) -> Self {
        use self::AsymmetricAlgorithm::*;
        match a {
            RSA1024 => 0,
            RSA2048 => 1,
            RSA3072 => 2,
            RSA4096 => 3,
            ElGamal1024 => 4,
            ElGamal2048 => 5,
            ElGamal3072 => 6,
            ElGamal4096 => 7,
            DSA1024 => 8,
            DSA2048 => 9,
            DSA3072 => 10,
            DSA4096 => 11,
            NistP256 => 12,
            NistP384 => 13,
            NistP521 => 14,
            BrainpoolP256 => 15,
            BrainpoolP512 => 16,
            Cv25519 => 17,
            Unknown => 255,
        }
    }
}

/// The Null Policy.
///
/// Danger, here be dragons.
///
/// This policy imposes no additional policy, i.e., accepts
/// everything.  This includes the MD5 hash algorithm, and SED
/// packets.
///
/// The Null policy has a limited set of valid use cases, e.g., packet statistics.
/// For other purposes, it is more advisable to use the [`StandardPolicy`] and
/// adjust it by selectively allowing items considered insecure by default, e.g.,
/// via [`StandardPolicy::accept_hash`] function. If this is still too inflexible
/// consider creating a specialized policy based on the [`StandardPolicy`] as
/// [the example for `StandardPolicy`] illustrates.
///
///   [`StandardPolicy::accept_hash`]: StandardPolicy::accept_hash()
///   [the example for `StandardPolicy`]: StandardPolicy#examples
#[derive(Debug)]
pub struct NullPolicy {
}

assert_send_and_sync!(NullPolicy);

impl NullPolicy {
    /// Instantiates a new `NullPolicy`.
    pub const fn new() -> Self {
        NullPolicy {}
    }
}

impl Policy for NullPolicy {
    fn signature(&self, _sig: &Signature, _sec: HashAlgoSecurity) -> Result<()> {
        Ok(())
    }

    fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
        -> Result<()>
    {
        Ok(())
    }

    fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
        Ok(())
    }

    fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
        Ok(())
    }

    fn packet(&self, _packet: &Packet) -> Result<()> {
        Ok(())
    }

}

#[cfg(test)]
mod test {
    use std::io::Read;
    use std::time::Duration;

    use super::*;
    use crate::Error;
    use crate::Fingerprint;
    use crate::crypto::SessionKey;
    use crate::packet::key::Key4;
    use crate::packet::signature;
    use crate::packet::{PKESK, SKESK};
    use crate::parse::Parse;
    use crate::parse::stream::DecryptionHelper;
    use crate::parse::stream::DecryptorBuilder;
    use crate::parse::stream::DetachedVerifierBuilder;
    use crate::parse::stream::MessageLayer;
    use crate::parse::stream::MessageStructure;
    use crate::parse::stream::VerificationHelper;
    use crate::parse::stream::VerifierBuilder;
    use crate::policy::StandardPolicy as P;
    use crate::types::Curve;
    use crate::types::KeyFlags;
    use crate::types::SymmetricAlgorithm;

    // Test that the constructor is const.
    const _A_STANDARD_POLICY: StandardPolicy = StandardPolicy::new();

    #[test]
    fn binding_signature() {
        let p = &P::new();

        // A primary and two subkeys.
        let (cert, _) = CertBuilder::new()
            .add_signing_subkey()
            .add_transport_encryption_subkey()
            .generate().unwrap();

        assert_eq!(cert.keys().with_policy(p, None).count(), 3);

        // Reject all direct key signatures.
        #[derive(Debug)]
        struct NoDirectKeySigs;
        impl Policy for NoDirectKeySigs {
            fn signature(&self, sig: &Signature, _sec: HashAlgoSecurity)
                -> Result<()>
            {
                use crate::types::SignatureType::*;

                match sig.typ() {
                    DirectKey => Err(anyhow::anyhow!("direct key!")),
                    _ => Ok(()),
                }
            }

            fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                -> Result<()>
            {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }

        let p = &NoDirectKeySigs {};
        assert_eq!(cert.keys().with_policy(p, None).count(), 0);

        // Reject all subkey signatures.
        #[derive(Debug)]
        struct NoSubkeySigs;
        impl Policy for NoSubkeySigs {
            fn signature(&self, sig: &Signature, _sec: HashAlgoSecurity)
                -> Result<()>
            {
                use crate::types::SignatureType::*;

                match sig.typ() {
                    SubkeyBinding => Err(anyhow::anyhow!("subkey signature!")),
                    _ => Ok(()),
                }
            }

            fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                -> Result<()>
            {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }

        let p = &NoSubkeySigs {};
        assert_eq!(cert.keys().with_policy(p, None).count(), 1);
    }

    #[test]
    fn revocation() -> Result<()> {
        use crate::cert::prelude::*;
        use crate::types::SignatureType;
        use crate::types::ReasonForRevocation;

        let p = &P::new();

        // A primary and two subkeys.
        let (cert, _) = CertBuilder::new()
            .add_userid("Alice")
            .add_signing_subkey()
            .add_transport_encryption_subkey()
            .generate()?;

        // Make sure we have all keys and all user ids.
        assert_eq!(cert.keys().with_policy(p, None).count(), 3);
        assert_eq!(cert.userids().with_policy(p, None).count(), 1);

        // Reject all user id signatures.
        #[derive(Debug)]
        struct NoPositiveCertifications;
        impl Policy for NoPositiveCertifications {
            fn signature(&self, sig: &Signature, _sec: HashAlgoSecurity)
                -> Result<()>
            {
                use crate::types::SignatureType::*;
                match sig.typ() {
                    PositiveCertification =>
                        Err(anyhow::anyhow!("positive certification!")),
                    _ => Ok(()),
                }
            }

            fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                -> Result<()>
            {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }
        let p = &NoPositiveCertifications {};
        assert_eq!(cert.userids().with_policy(p, None).count(), 0);


        // Revoke it.
        let mut keypair = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        let ca = cert.userids().next().unwrap();

        // Generate the revocation for the first and only UserID.
        let revocation =
            UserIDRevocationBuilder::new()
            .set_reason_for_revocation(
                ReasonForRevocation::KeyRetired,
                b"Left example.org.")?
            .build(&mut keypair, &cert, ca.userid(), None)?;
        assert_eq!(revocation.typ(), SignatureType::CertificationRevocation);

        // Now merge the revocation signature into the Cert.
        let cert = cert.insert_packets(revocation.clone())?;

        // Check that it is revoked.
        assert_eq!(cert.userids().with_policy(p, None).revoked(false).count(), 0);

        // Reject all user id signatures.
        #[derive(Debug)]
        struct NoCertificationRevocation;
        impl Policy for NoCertificationRevocation {
            fn signature(&self, sig: &Signature, _sec: HashAlgoSecurity)
                -> Result<()>
            {
                use crate::types::SignatureType::*;
                match sig.typ() {
                    CertificationRevocation =>
                        Err(anyhow::anyhow!("certification certification!")),
                    _ => Ok(()),
                }
            }

            fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                -> Result<()>
            {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }
        let p = &NoCertificationRevocation {};

        // Check that the user id is no longer revoked.
        assert_eq!(cert.userids().with_policy(p, None).revoked(false).count(), 1);


        // Generate the revocation for the first subkey.
        let subkey = cert.keys().subkeys().next().unwrap();
        let revocation =
            SubkeyRevocationBuilder::new()
                .set_reason_for_revocation(
                    ReasonForRevocation::KeyRetired,
                    b"Smells funny.").unwrap()
                .build(&mut keypair, &cert, subkey.key(), None)?;
        assert_eq!(revocation.typ(), SignatureType::SubkeyRevocation);

        // Now merge the revocation signature into the Cert.
        assert_eq!(cert.keys().with_policy(p, None).revoked(false).count(), 3);
        let cert = cert.insert_packets(revocation.clone())?;
        assert_eq!(cert.keys().with_policy(p, None).revoked(false).count(), 2);

        // Reject all subkey revocations.
        #[derive(Debug)]
        struct NoSubkeyRevocation;
        impl Policy for NoSubkeyRevocation {
            fn signature(&self, sig: &Signature, _sec: HashAlgoSecurity)
                -> Result<()>
            {
                use crate::types::SignatureType::*;
                match sig.typ() {
                    SubkeyRevocation =>
                        Err(anyhow::anyhow!("subkey revocation!")),
                    _ => Ok(()),
                }
            }

            fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                -> Result<()>
            {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }
        let p = &NoSubkeyRevocation {};

        // Check that the key is no longer revoked.
        assert_eq!(cert.keys().with_policy(p, None).revoked(false).count(), 3);

        Ok(())
    }


    #[test]
    fn binary_signature() -> Result<()> {
        #[derive(PartialEq, Debug)]
        struct VHelper {
            good: usize,
            errors: usize,
            keys: Vec<Cert>,
        }

        impl VHelper {
            fn new(keys: Vec<Cert>) -> Self {
                VHelper {
                    good: 0,
                    errors: 0,
                    keys,
                }
            }
        }

        impl VerificationHelper for VHelper {
            fn get_certs(&mut self, _ids: &[crate::KeyHandle])
                -> Result<Vec<Cert>>
            {
                Ok(self.keys.clone())
            }

            fn check(&mut self, structure: MessageStructure) -> Result<()>
            {
                for layer in structure {
                    match layer {
                        MessageLayer::SignatureGroup { ref results } =>
                            for result in results {
                                eprintln!("result: {:?}", result);
                                match result {
                                    Ok(_) => self.good += 1,
                                    Err(_) => self.errors += 1,
                                }
                            }
                        MessageLayer::Compression { .. } => (),
                        _ => unreachable!(),
                    }
                }

                Ok(())
            }
        }

        impl DecryptionHelper for VHelper {
            fn decrypt<D>(&mut self, _: &[PKESK], _: &[SKESK],
                          _: Option<SymmetricAlgorithm>,_: D)
                          -> Result<Option<Fingerprint>>
                where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
            {
                unreachable!();
            }
        }

        // Reject all data (binary) signatures.
        #[derive(Debug)]
        struct NoBinarySigantures;
        impl Policy for NoBinarySigantures {
            fn signature(&self, sig: &Signature, _sec: HashAlgoSecurity)
                -> Result<()>
            {
                use crate::types::SignatureType::*;
                eprintln!("{:?}", sig.typ());
                match sig.typ() {
                    Binary =>
                        Err(anyhow::anyhow!("binary!")),
                    _ => Ok(()),
                }
            }

            fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                -> Result<()>
            {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }
        let no_binary_signatures = &NoBinarySigantures {};

        // Reject all subkey signatures.
        #[derive(Debug)]
        struct NoSubkeySigs;
        impl Policy for NoSubkeySigs {
            fn signature(&self, sig: &Signature, _sec: HashAlgoSecurity)
                -> Result<()>
            {
                use crate::types::SignatureType::*;

                match sig.typ() {
                    SubkeyBinding => Err(anyhow::anyhow!("subkey signature!")),
                    _ => Ok(()),
                }
            }

            fn key(&self, _ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                -> Result<()>
            {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }
        let no_subkey_signatures = &NoSubkeySigs {};

        let standard = &P::new();

        let keys = [
            "neal.pgp",
        ].iter()
            .map(|f| Cert::from_bytes(crate::tests::key(f)).unwrap())
            .collect::<Vec<_>>();
        let data = "messages/signed-1.gpg";

        let reference = crate::tests::manifesto();



        // Test Verifier.

        // Standard policy => ok.
        let h = VHelper::new(keys.clone());
        let mut v = VerifierBuilder::from_bytes(crate::tests::file(data))?
            .with_policy(standard, crate::frozen_time(), h)?;
        assert!(v.message_processed());
        assert_eq!(v.helper_ref().good, 1);
        assert_eq!(v.helper_ref().errors, 0);

        let mut content = Vec::new();
        v.read_to_end(&mut content).unwrap();
        assert_eq!(reference.len(), content.len());
        assert_eq!(reference, &content[..]);


        // Kill the subkey.
        let h = VHelper::new(keys.clone());
        let mut v = VerifierBuilder::from_bytes(crate::tests::file(data))?
            .with_policy(no_subkey_signatures, crate::frozen_time(), h)?;
        assert!(v.message_processed());
        assert_eq!(v.helper_ref().good, 0);
        assert_eq!(v.helper_ref().errors, 1);

        let mut content = Vec::new();
        v.read_to_end(&mut content).unwrap();
        assert_eq!(reference.len(), content.len());
        assert_eq!(reference, &content[..]);


        // Kill the data signature.
        let h = VHelper::new(keys.clone());
        let mut v = VerifierBuilder::from_bytes(crate::tests::file(data))?
            .with_policy(no_binary_signatures, crate::frozen_time(), h)?;
        assert!(v.message_processed());
        assert_eq!(v.helper_ref().good, 0);
        assert_eq!(v.helper_ref().errors, 1);

        let mut content = Vec::new();
        v.read_to_end(&mut content).unwrap();
        assert_eq!(reference.len(), content.len());
        assert_eq!(reference, &content[..]);



        // Test Decryptor.

        // Standard policy.
        let h = VHelper::new(keys.clone());
        let mut v = DecryptorBuilder::from_bytes(crate::tests::file(data))?
            .with_policy(standard, crate::frozen_time(), h)?;
        assert!(v.message_processed());
        assert_eq!(v.helper_ref().good, 1);
        assert_eq!(v.helper_ref().errors, 0);

        let mut content = Vec::new();
        v.read_to_end(&mut content).unwrap();
        assert_eq!(reference.len(), content.len());
        assert_eq!(reference, &content[..]);


        // Kill the subkey.
        let h = VHelper::new(keys.clone());
        let mut v = DecryptorBuilder::from_bytes(crate::tests::file(data))?
            .with_policy(no_subkey_signatures, crate::frozen_time(), h)?;
        assert!(v.message_processed());
        assert_eq!(v.helper_ref().good, 0);
        assert_eq!(v.helper_ref().errors, 1);

        let mut content = Vec::new();
        v.read_to_end(&mut content).unwrap();
        assert_eq!(reference.len(), content.len());
        assert_eq!(reference, &content[..]);


        // Kill the data signature.
        let h = VHelper::new(keys.clone());
        let mut v = DecryptorBuilder::from_bytes(crate::tests::file(data))?
            .with_policy(no_binary_signatures, crate::frozen_time(), h)?;
        assert!(v.message_processed());
        assert_eq!(v.helper_ref().good, 0);
        assert_eq!(v.helper_ref().errors, 1);

        let mut content = Vec::new();
        v.read_to_end(&mut content).unwrap();
        assert_eq!(reference.len(), content.len());
        assert_eq!(reference, &content[..]);
        Ok(())
    }

    #[test]
    fn hash_algo() -> Result<()> {
        use crate::types::RevocationStatus;
        use crate::types::ReasonForRevocation;

        const SECS_IN_YEAR : u64 = 365 * 24 * 60 * 60;

        // A `const fn` is only guaranteed to be evaluated at compile
        // time if the result is assigned to a `const` variable.  Make
        // sure that works.
        const DEFAULT : StandardPolicy = StandardPolicy::new();

        let (cert, _) = CertBuilder::new()
            .add_userid("Alice")
            .generate()?;

        let algo = cert.primary_key()
            .binding_signature(&DEFAULT, None).unwrap().hash_algo();

        eprintln!("{:?}", algo);

        // Create a revoked version.
        let mut keypair = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;
        let rev = cert.revoke(
            &mut keypair,
            ReasonForRevocation::KeyCompromised,
            b"It was the maid :/")?;
        let cert_revoked = cert.clone().insert_packets(rev)?;

        match cert_revoked.revocation_status(&DEFAULT, None) {
            RevocationStatus::Revoked(sigs) => {
                assert_eq!(sigs.len(), 1);
                assert_eq!(sigs[0].hash_algo(), algo);
            }
            _ => panic!("not revoked"),
        }


        // Reject the hash algorithm unconditionally.
        let mut reject : StandardPolicy = StandardPolicy::new();
        reject.reject_hash(algo);
        assert!(cert.primary_key()
                    .binding_signature(&reject, None).is_err());
        assert_match!(RevocationStatus::NotAsFarAsWeKnow
                      = cert_revoked.revocation_status(&reject, None));

        // Reject the hash algorithm next year.
        let mut reject : StandardPolicy = StandardPolicy::new();
        reject.reject_hash_at(
            algo,
            crate::now().checked_add(Duration::from_secs(SECS_IN_YEAR)));
        reject.hash_revocation_tolerance(0);
        cert.primary_key().binding_signature(&reject, None)?;
        assert_match!(RevocationStatus::Revoked(_)
                      = cert_revoked.revocation_status(&reject, None));

        // Reject the hash algorithm last year.
        let mut reject : StandardPolicy = StandardPolicy::new();
        reject.reject_hash_at(
            algo,
            crate::now().checked_sub(Duration::from_secs(SECS_IN_YEAR)));
        reject.hash_revocation_tolerance(0);
        assert!(cert.primary_key()
                    .binding_signature(&reject, None).is_err());
        assert_match!(RevocationStatus::NotAsFarAsWeKnow
                      = cert_revoked.revocation_status(&reject, None));

        // Reject the hash algorithm for normal signatures last year,
        // and revocations next year.
        let mut reject : StandardPolicy = StandardPolicy::new();
        reject.reject_hash_at(
            algo,
            crate::now().checked_sub(Duration::from_secs(SECS_IN_YEAR)));
        reject.hash_revocation_tolerance(2 * SECS_IN_YEAR as u32);
        assert!(cert.primary_key()
                    .binding_signature(&reject, None).is_err());
        assert_match!(RevocationStatus::Revoked(_)
                      = cert_revoked.revocation_status(&reject, None));

        // Accept algo, but reject the algos with id - 1 and id + 1.
        let mut reject : StandardPolicy = StandardPolicy::new();
        let algo_u8 : u8 = algo.into();
        assert!(algo_u8 != 0u8);
        reject.reject_hash_at(
            (algo_u8 - 1).into(),
            crate::now().checked_sub(Duration::from_secs(SECS_IN_YEAR)));
        reject.reject_hash_at(
            (algo_u8 + 1).into(),
            crate::now().checked_sub(Duration::from_secs(SECS_IN_YEAR)));
        reject.hash_revocation_tolerance(0);
        cert.primary_key().binding_signature(&reject, None)?;
        assert_match!(RevocationStatus::Revoked(_)
                      = cert_revoked.revocation_status(&reject, None));

        // Reject the hash algorithm since before the Unix epoch.
        // Since the earliest representable time using a Timestamp is
        // the Unix epoch, this is equivalent to rejecting everything.
        let mut reject : StandardPolicy = StandardPolicy::new();
        reject.reject_hash_at(
            algo,
            crate::now().checked_sub(Duration::from_secs(SECS_IN_YEAR)));
        reject.hash_revocation_tolerance(0);
        assert!(cert.primary_key()
                    .binding_signature(&reject, None).is_err());
        assert_match!(RevocationStatus::NotAsFarAsWeKnow
                      = cert_revoked.revocation_status(&reject, None));

        // Reject the hash algorithm after the end of time that is
        // representable by a Timestamp (2106).  This should accept
        // everything.
        let mut reject : StandardPolicy = StandardPolicy::new();
        reject.reject_hash_at(
            algo,
            SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(500 * SECS_IN_YEAR)));
        reject.hash_revocation_tolerance(0);
        cert.primary_key().binding_signature(&reject, None)?;
        assert_match!(RevocationStatus::Revoked(_)
                      = cert_revoked.revocation_status(&reject, None));

        Ok(())
    }

    #[test]
    fn key_verify_self_signature() -> Result<()> {
        let p = &P::new();

        #[derive(Debug)]
        struct NoRsa;
        impl Policy for NoRsa {
            fn key(&self, ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                   -> Result<()>
            {
                use crate::types::PublicKeyAlgorithm::*;

                eprintln!("algo: {}", ka.key().pk_algo());
                if ka.key().pk_algo() == RSAEncryptSign {
                    Err(anyhow::anyhow!("RSA!"))
                } else {
                    Ok(())
                }
            }

            fn signature(&self, _sig: &Signature, _sec: HashAlgoSecurity) -> Result<()> {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }
        let norsa = &NoRsa {};

        // Generate a certificate with an RSA primary and two RSA
        // subkeys.
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::RSA2k)
            .add_signing_subkey()
            .add_signing_subkey()
            .generate()?;
        assert_eq!(cert.keys().with_policy(p, None).count(), 3);
        assert_eq!(cert.keys().with_policy(norsa, None).count(), 0);
        assert!(cert.primary_key().with_policy(p, None).is_ok());
        assert!(cert.primary_key().with_policy(norsa, None).is_err());

        // Generate a certificate with an ECC primary, an ECC subkey,
        // and an RSA subkey.
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_signing_subkey()
            .generate()?;

        let pk = cert.primary_key().key().parts_as_secret()?;
        let subkey: key::SecretSubkey
            = Key4::generate_rsa(2048)?.into();
        let binding = signature::SignatureBuilder::new(SignatureType::SubkeyBinding)
            .set_key_flags(KeyFlags::empty().set_transport_encryption())?
            .sign_subkey_binding(&mut pk.clone().into_keypair()?,
                                 pk.parts_as_public(), &subkey)?;

        let cert = cert.insert_packets(
            vec![ Packet::from(subkey), binding.into() ])?;

        assert_eq!(cert.keys().with_policy(p, None).count(), 3);
        assert_eq!(cert.keys().with_policy(norsa, None).count(), 2);
        assert!(cert.primary_key().with_policy(p, None).is_ok());
        assert!(cert.primary_key().with_policy(norsa, None).is_ok());

        // Generate a certificate with an RSA primary, an RSA subkey,
        // and an ECC subkey.
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::RSA2k)
            .add_signing_subkey()
            .generate()?;

        let pk = cert.primary_key().key().parts_as_secret()?;
        let subkey: key::SecretSubkey
            = key::Key4::generate_ecc(true, Curve::Ed25519)?.into();
        let binding = signature::SignatureBuilder::new(SignatureType::SubkeyBinding)
            .set_key_flags(KeyFlags::empty().set_transport_encryption())?
            .sign_subkey_binding(&mut pk.clone().into_keypair()?,
                                 pk.parts_as_public(), &subkey)?;

        let cert = cert.insert_packets(
            vec![ Packet::from(subkey), binding.into() ])?;

        assert_eq!(cert.keys().with_policy(p, None).count(), 3);
        assert_eq!(cert.keys().with_policy(norsa, None).count(), 0);
        assert!(cert.primary_key().with_policy(p, None).is_ok());
        assert!(cert.primary_key().with_policy(norsa, None).is_err());

        // Generate a certificate with an ECC primary and two ECC
        // subkeys.
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_signing_subkey()
            .add_signing_subkey()
            .generate()?;
        assert_eq!(cert.keys().with_policy(p, None).count(), 3);
        assert_eq!(cert.keys().with_policy(norsa, None).count(), 3);
        assert!(cert.primary_key().with_policy(p, None).is_ok());
        assert!(cert.primary_key().with_policy(norsa, None).is_ok());

        Ok(())
    }

    #[test]
    fn key_verify_binary_signature() -> Result<()> {
        use crate::packet::signature;
        use crate::serialize::Serialize;
        use crate::Packet;
        use crate::types::KeyFlags;

        let p = &P::new();

        #[derive(Debug)]
        struct NoRsa;
        impl Policy for NoRsa {
            fn key(&self, ka: &ValidErasedKeyAmalgamation<key::PublicParts>)
                   -> Result<()>
            {
                use crate::types::PublicKeyAlgorithm::*;

                eprintln!("algo: {} is {}",
                          ka.fingerprint(), ka.key().pk_algo());
                if ka.key().pk_algo() == RSAEncryptSign {
                    Err(anyhow::anyhow!("RSA!"))
                } else {
                    Ok(())
                }
            }

            fn signature(&self, _sig: &Signature, _sec: HashAlgoSecurity) -> Result<()> {
                Ok(())
            }

            fn symmetric_algorithm(&self, _algo: SymmetricAlgorithm) -> Result<()> {
                Ok(())
            }

            fn aead_algorithm(&self, _algo: AEADAlgorithm) -> Result<()> {
                Ok(())
            }

            fn packet(&self, _packet: &Packet) -> Result<()> {
                Ok(())
            }
        }
        let norsa = &NoRsa {};

        #[derive(PartialEq, Debug)]
        struct VHelper {
            good: usize,
            errors: usize,
            keys: Vec<Cert>,
        }

        impl VHelper {
            fn new(keys: Vec<Cert>) -> Self {
                VHelper {
                    good: 0,
                    errors: 0,
                    keys,
                }
            }
        }

        impl VerificationHelper for VHelper {
            fn get_certs(&mut self, _ids: &[crate::KeyHandle])
                -> Result<Vec<Cert>>
            {
                Ok(self.keys.clone())
            }

            fn check(&mut self, structure: MessageStructure) -> Result<()>
            {
                for layer in structure {
                    match layer {
                        MessageLayer::SignatureGroup { ref results } =>
                            for result in results {
                                match result {
                                    Ok(_) => self.good += 1,
                                    Err(_) => self.errors += 1,
                                }
                            }
                        MessageLayer::Compression { .. } => (),
                        _ => unreachable!(),
                    }
                }

                Ok(())
            }
        }

        impl DecryptionHelper for VHelper {
            fn decrypt<D>(&mut self, _: &[PKESK], _: &[SKESK],
                          _: Option<SymmetricAlgorithm>,_: D)
                          -> Result<Option<Fingerprint>>
                where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
            {
                unreachable!();
            }
        }

        // Sign msg using cert's first subkey, return the signature.
        fn sign_and_verify(p: &dyn Policy, cert: &Cert, good: bool) {
            eprintln!("Expect verification to be {}",
                      if good { "good" } else { "bad" });
            for (i, k) in cert.keys().enumerate() {
                eprintln!("  {}. {}", i, k.fingerprint());
            }

            let msg = b"Hello, World";

            // We always use the first subkey.
            let key = cert.keys().nth(1).unwrap().key();
            let mut keypair = key.clone()
                .parts_into_secret().unwrap()
                .into_keypair().unwrap();

            // Create a signature.
            let mut sig =
                signature::SignatureBuilder::new(SignatureType::Binary)
                .sign_message(&mut keypair, msg).unwrap();

            // Make sure the signature is ok.
            sig.verify_message(key, msg).unwrap();

            // Turn it into a detached signature.
            let sig = {
                let mut v = Vec::new();
                let sig : Packet = sig.into();
                sig.serialize(&mut v).unwrap();
                v
            };

            let h = VHelper::new(vec![ cert.clone() ]);
            let mut v = DetachedVerifierBuilder::from_bytes(&sig).unwrap()
                .with_policy(p, None, h).unwrap();
            v.verify_bytes(msg).unwrap();
            assert_eq!(v.helper_ref().good, if good { 1 } else { 0 });
            assert_eq!(v.helper_ref().errors, if good { 0 } else { 1 });
        }


        // A certificate with an ECC primary and an ECC signing
        // subkey.
        eprintln!("Trying ECC primary, ECC sub:");
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_subkey(KeyFlags::empty().set_signing(), None,
                        None)
            .generate()?;

        assert_eq!(cert.keys().with_policy(p, None).count(), 2);
        assert_eq!(cert.keys().with_policy(norsa, None).count(), 2);
        assert!(cert.primary_key().with_policy(p, None).is_ok());
        assert!(cert.primary_key().with_policy(norsa, None).is_ok());

        sign_and_verify(p, &cert, true);
        sign_and_verify(norsa, &cert, true);

        // A certificate with an RSA primary and an RCC signing
        // subkey.
        eprintln!("Trying RSA primary, ECC sub:");
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::RSA2k)
            .add_subkey(KeyFlags::empty().set_signing(), None,
                        CipherSuite::Cv25519)
            .generate()?;

        assert_eq!(cert.keys().with_policy(p, None).count(), 2);
        assert_eq!(cert.keys().with_policy(norsa, None).count(), 0);
        assert!(cert.primary_key().with_policy(p, None).is_ok());
        assert!(cert.primary_key().with_policy(norsa, None).is_err());

        sign_and_verify(p, &cert, true);
        sign_and_verify(norsa, &cert, false);

        // A certificate with an ECC primary and an RSA signing
        // subkey.
        eprintln!("Trying ECC primary, RSA sub:");
        let (cert,_) = CertBuilder::new()
            .set_cipher_suite(CipherSuite::Cv25519)
            .add_subkey(KeyFlags::empty().set_signing(), None,
                        CipherSuite::RSA2k)
            .generate()?;

        assert_eq!(cert.keys().with_policy(p, None).count(), 2);
        assert_eq!(cert.keys().with_policy(norsa, None).count(), 1);
        assert!(cert.primary_key().with_policy(p, None).is_ok());
        assert!(cert.primary_key().with_policy(norsa, None).is_ok());

        sign_and_verify(p, &cert, true);
        sign_and_verify(norsa, &cert, false);

        Ok(())
    }

    #[test]
    fn reject_seip_packet() -> Result<()> {
        #[derive(PartialEq, Debug)]
        struct Helper {}
        impl VerificationHelper for Helper {
            fn get_certs(&mut self, _: &[crate::KeyHandle])
                -> Result<Vec<Cert>> {
                unreachable!()
            }

            fn check(&mut self, _: MessageStructure) -> Result<()> {
                unreachable!()
            }
        }

        impl DecryptionHelper for Helper {
            fn decrypt<D>(&mut self, _: &[PKESK], _: &[SKESK],
                          _: Option<SymmetricAlgorithm>, _: D)
                          -> Result<Option<Fingerprint>>
                where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool {
                Ok(None)
            }
        }

        let p = &P::new();
        let r = DecryptorBuilder::from_bytes(crate::tests::message(
                "encrypted-to-testy.gpg"))?
            .with_policy(p, crate::frozen_time(), Helper {});
        match r {
            Ok(_) => panic!(),
            Err(e) => assert_match!(Error::MissingSessionKey(_)
                                    = e.downcast().unwrap()),
        }

        // Reject the SEIP packet.
        let p = &mut P::new();
        p.reject_packet_tag(Tag::SEIP);
        let r = DecryptorBuilder::from_bytes(crate::tests::message(
                "encrypted-to-testy.gpg"))?
            .with_policy(p, crate::frozen_time(), Helper {});
        match r {
            Ok(_) => panic!(),
            Err(e) => assert_match!(Error::PolicyViolation(_, _)
                                    = e.downcast().unwrap()),
        }
        Ok(())
    }

    #[test]
    fn reject_cipher() -> Result<()> {
        struct Helper {}
        impl VerificationHelper for Helper {
            fn get_certs(&mut self, _: &[crate::KeyHandle])
                -> Result<Vec<Cert>> {
                Ok(Default::default())
            }

            fn check(&mut self, _: MessageStructure) -> Result<()> {
                Ok(())
            }
        }

        impl DecryptionHelper for Helper {
            fn decrypt<D>(&mut self, pkesks: &[PKESK], _: &[SKESK],
                          algo: Option<SymmetricAlgorithm>, mut decrypt: D)
                          -> Result<Option<Fingerprint>>
                where D: FnMut(SymmetricAlgorithm, &SessionKey) -> bool
            {
                let p = &P::new();
                let mut pair = Cert::from_bytes(
                    crate::tests::key("testy-private.pgp"))?
                    .keys().with_policy(p, None)
                    .for_transport_encryption().secret().next().unwrap()
                    .key().clone().into_keypair()?;
                pkesks[0].decrypt(&mut pair, algo)
                    .map(|(algo, session_key)| decrypt(algo, &session_key));
                Ok(None)
            }
        }

        let p = &P::new();
        DecryptorBuilder::from_bytes(crate::tests::message(
                "encrypted-to-testy-no-compression.gpg"))?
            .with_policy(p, crate::frozen_time(), Helper {})?;

        // Reject the AES256.
        let p = &mut P::new();
        p.reject_symmetric_algo(SymmetricAlgorithm::AES256);
        let r = DecryptorBuilder::from_bytes(crate::tests::message(
                "encrypted-to-testy-no-compression.gpg"))?
            .with_policy(p, crate::frozen_time(), Helper {});
        match r {
            Ok(_) => panic!(),
            Err(e) => assert_match!(Error::PolicyViolation(_, _)
                                    = e.downcast().unwrap()),
        }
        Ok(())
    }

    #[test]
    fn reject_asymmetric_algos() -> Result<()> {
        let cert = Cert::from_bytes(crate::tests::key("neal.pgp"))?;
        let p = &mut P::new();
        let t = crate::frozen_time();

        assert_eq!(cert.with_policy(p, t).unwrap().keys().count(), 4);
        p.reject_asymmetric_algo(AsymmetricAlgorithm::RSA1024);
        assert_eq!(cert.with_policy(p, t).unwrap().keys().count(), 4);
        p.reject_asymmetric_algo(AsymmetricAlgorithm::RSA2048);
        assert_eq!(cert.with_policy(p, t).unwrap().keys().count(), 1);
        Ok(())
    }

    #[test]
    fn reject_all_hashes() -> Result<()> {
        let mut p = StandardPolicy::new();

        let set_variants = [
            HashAlgorithm::MD5,
            HashAlgorithm::Unknown(234),
        ];
        let check_variants = [
            HashAlgorithm::SHA512,
            HashAlgorithm::Unknown(239),
        ];

        // Accept a few hashes explicitly.
        for v in set_variants.iter().cloned() {
            p.accept_hash(v);
            assert_eq!(
                p.hash_cutoff(
                    v,
                    HashAlgoSecurity::SecondPreImageResistance),
                ACCEPT.map(Into::into));
            assert_eq!(
                p.hash_cutoff(
                    v,
                    HashAlgoSecurity::CollisionResistance),
                ACCEPT.map(Into::into));
        }

        // Reject all hashes.
        p.reject_all_hashes();

        for v in set_variants.iter().chain(check_variants.iter()).cloned() {
            assert_eq!(
                p.hash_cutoff(
                    v,
                    HashAlgoSecurity::SecondPreImageResistance),
                REJECT.map(Into::into));
            assert_eq!(
                p.hash_cutoff(
                    v,
                    HashAlgoSecurity::CollisionResistance),
                REJECT.map(Into::into));
        }

        Ok(())
    }

    macro_rules! reject_all_check {
        ($reject_all:ident, $accept_one:ident, $cutoff:ident,
         $set_variants:expr, $check_variants:expr) => {
            #[test]
            fn $reject_all() -> Result<()> {
                let mut p = StandardPolicy::new();

                // Accept a few hashes explicitly.
                for v in $set_variants.iter().cloned() {
                    p.$accept_one(v);
                    assert_eq!(p.$cutoff(v), ACCEPT.map(Into::into));
                }

                // Reject all hashes.
                p.$reject_all();

                for v in $set_variants.iter()
                    .chain($check_variants.iter()).cloned()
                {
                    assert_eq!(
                        p.$cutoff(v),
                        REJECT.map(Into::into));
                }
                Ok(())
            }
        }
    }

    reject_all_check!(reject_all_critical_subpackets,
                      accept_critical_subpacket,
                      critical_subpacket_cutoff,
                      &[ SubpacketTag::TrustSignature,
                         SubpacketTag::Unknown(252) ],
                      &[ SubpacketTag::Unknown(253),
                         SubpacketTag::SignatureCreationTime ]);

    reject_all_check!(reject_all_asymmetric_algos,
                      accept_asymmetric_algo,
                      asymmetric_algo_cutoff,
                      &[ AsymmetricAlgorithm::RSA3072,
                         AsymmetricAlgorithm::Cv25519 ],
                      &[ AsymmetricAlgorithm::Unknown,
                         AsymmetricAlgorithm::NistP256 ]);

    reject_all_check!(reject_all_symmetric_algos,
                      accept_symmetric_algo,
                      symmetric_algo_cutoff,
                      &[ SymmetricAlgorithm::Unencrypted,
                         SymmetricAlgorithm::Unknown(252) ],
                      &[ SymmetricAlgorithm::AES256,
                         SymmetricAlgorithm::Unknown(230) ]);

    reject_all_check!(reject_all_aead_algos,
                      accept_aead_algo,
                      aead_algo_cutoff,
                      &[ AEADAlgorithm::OCB ],
                      &[ AEADAlgorithm::EAX ]);

    #[test]
    fn reject_all_packets() -> Result<()> {
        let mut p = StandardPolicy::new();

        let set_variants = [
            (Tag::SEIP, 4),
            (Tag::Unknown(252), 17),
        ];
        let check_variants = [
            (Tag::Signature, 4),
            (Tag::Unknown(230), 9),
        ];

        // Accept a few packets explicitly.
        for (t, v) in set_variants.iter().cloned() {
            p.accept_packet_tag_version(t, v);
            assert_eq!(
                p.packet_tag_version_cutoff(t, v),
                ACCEPT.map(Into::into));
        }

        // Reject all hashes.
        p.reject_all_packet_tags();

        for (t, v) in set_variants.iter().chain(check_variants.iter()).cloned() {
            assert_eq!(
                p.packet_tag_version_cutoff(t, v),
                REJECT.map(Into::into));
        }

        Ok(())
    }

    #[test]
    fn packet_versions() -> Result<()> {
        // Accept the version of a packet.  Optionally make sure a
        // different version is not accepted.
        fn accept_and_check(p: &mut StandardPolicy,
                            tag: Tag,
                            accept_versions: &[u8],
                            good_versions: &[u8],
                            bad_versions: &[u8]) {
            for v in accept_versions {
                p.accept_packet_tag_version(tag, *v);
                assert_eq!(
                    p.packet_tag_version_cutoff(tag, *v),
                    ACCEPT.map(Into::into));
            }

            for v in good_versions.iter() {
                assert_eq!(
                    p.packet_tag_version_cutoff(tag, *v),
                    ACCEPT.map(Into::into));
            }
            for v in bad_versions.iter() {
                assert_eq!(
                    p.packet_tag_version_cutoff(tag, *v),
                    REJECT.map(Into::into));
            }
        }

        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();

        let mut all_versions = (0..=u8::MAX).collect::<Vec<_>>();
        all_versions.shuffle(&mut rng);
        let all_versions = &all_versions[..];
        let mut not_v5 = all_versions.iter()
            .filter(|&&v| v != 5)
            .cloned()
            .collect::<Vec<_>>();
        not_v5.shuffle(&mut rng);
        let not_v5 = &not_v5[..];

        let p = &mut StandardPolicy::new();
        p.reject_all_packet_tags();

        // First only use the versioned interfaces.
        accept_and_check(p, Tag::Signature, &[3], &[], &[4, 5]);
        accept_and_check(p, Tag::Signature, &[4], &[3], &[5]);

        // Only use an unversioned policy.
        accept_and_check(p, Tag::SEIP,
                         &[], // set to accept
                         &[], // good
                         all_versions, // bad
        );
        p.accept_packet_tag(Tag::SEIP);
        accept_and_check(p, Tag::SEIP,
                         &[], // set to accept
                         all_versions, // good
                         &[], // bad
        );

        // Set an unversioned policy and then a versioned policy.
        accept_and_check(p, Tag::PKESK,
                         &[], // set to accept
                         &[], // good
                         all_versions, // bad
        );
        p.accept_packet_tag(Tag::PKESK);
        accept_and_check(p, Tag::PKESK,
                         &[], // set to accept
                         &(0..u8::MAX).collect::<Vec<_>>()[..], // good
                         &[], // bad
        );
        p.reject_packet_tag_version(Tag::PKESK, 5);
        accept_and_check(p, Tag::PKESK,
                         &[], // set to accept
                         not_v5, // good
                         &[5], // bad
        );

        // Set a versioned policy and then an unversioned policy.
        // Make sure that the versioned policy is cleared by the
        // unversioned policy.
        accept_and_check(p, Tag::SKESK,
                         &[], // set to accept
                         &[], // good
                         all_versions, // bad
        );
        p.accept_packet_tag_version(Tag::SKESK, 5);
        accept_and_check(p, Tag::SKESK,
                         &[], // set to accept
                         &[5], // good
                         not_v5, // bad
        );
        p.reject_packet_tag(Tag::SKESK);
        // All versions should be bad now...
        accept_and_check(p, Tag::SKESK,
                         &[], // set to accept
                         &[], // good
                         all_versions, // bad
        );

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn packet_tag_cutoff() {
        // The semantics of packet_tag_cutoff are: max of all
        // versioned cutoffs and the unversioned cutoff.

        let p = &mut StandardPolicy::new();
        p.reject_all_packet_tags();

        assert_eq!(p.packet_tag_cutoff(Tag::Signature),
                   REJECT.map(Into::into));

        p.reject_packet_tag_version_at(Tag::Signature, 5,
                                       Timestamp::Y2007M2);
        assert_eq!(p.packet_tag_cutoff(Tag::Signature),
                   Some(Timestamp::Y2007M2.into()));

        p.reject_packet_tag_version_at(Tag::Signature, 3,
                                       Timestamp::Y2005M2);
        assert_eq!(p.packet_tag_cutoff(Tag::Signature),
                   Some(Timestamp::Y2007M2.into()));

        p.reject_packet_tag_version_at(Tag::Signature, 6,
                                       ACCEPT.map(Into::into));
        assert_eq!(p.packet_tag_cutoff(Tag::Signature),
                   ACCEPT.map(Into::into));

        p.reject_packet_tag_version_at(Tag::Signature, 6,
                                       Timestamp::Y2005M2);
        assert_eq!(p.packet_tag_cutoff(Tag::Signature),
                   Some(Timestamp::Y2007M2.into()));
    }
}
