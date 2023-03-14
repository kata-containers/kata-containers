//! PublicKey-Encrypted Session Key packets.
//!
//! The session key is needed to decrypt the actual ciphertext.  See
//! [Section 5.1 of RFC 4880] for details.
//!
//!   [Section 5.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.1

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::Error;
use crate::packet::key;
use crate::packet::Key;
use crate::KeyID;
use crate::crypto::Decryptor;
use crate::crypto::mpi::Ciphertext;
use crate::Packet;
use crate::PublicKeyAlgorithm;
use crate::Result;
use crate::SymmetricAlgorithm;
use crate::crypto::SessionKey;
use crate::packet;

/// Holds an asymmetrically encrypted session key.
///
/// The session key is needed to decrypt the actual ciphertext.  See
/// [Section 5.1 of RFC 4880] for details.
///
///   [Section 5.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.1
// IMPORTANT: If you add fields to this struct, you need to explicitly
// IMPORTANT: implement PartialEq, Eq, and Hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PKESK3 {
    /// CTB header fields.
    pub(crate) common: packet::Common,
    /// Key ID of the key this is encrypted to.
    recipient: KeyID,
    /// Public key algorithm used to encrypt the session key.
    pk_algo: PublicKeyAlgorithm,
    /// The encrypted session key.
    esk: Ciphertext,
}

assert_send_and_sync!(PKESK3);

impl PKESK3 {
    /// Creates a new PKESK3 packet.
    pub fn new(recipient: KeyID, pk_algo: PublicKeyAlgorithm,
               encrypted_session_key: Ciphertext)
               -> Result<PKESK3> {
        Ok(PKESK3 {
            common: Default::default(),
            recipient,
            pk_algo,
            esk: encrypted_session_key,
        })
    }

    /// Creates a new PKESK3 packet for the given recipent.
    ///
    /// The given symmetric algorithm must match the algorithm that is
    /// used to encrypt the payload.
    pub fn for_recipient<P, R>(algo: SymmetricAlgorithm,
                               session_key: &SessionKey,
                               recipient: &Key<P, R>)
        -> Result<PKESK3>
        where P: key::KeyParts,
              R: key::KeyRole,
    {
        // We need to prefix the cipher specifier to the session key,
        // and a two-octet checksum.
        let mut psk = Vec::with_capacity(1 + session_key.len() + 2);
        psk.push(algo.into());
        psk.extend_from_slice(session_key);

        // Compute the sum modulo 65536, i.e. as u16.
        let checksum = session_key
            .iter()
            .cloned()
            .map(u16::from)
            .fold(0u16, u16::wrapping_add);

        psk.extend_from_slice(&checksum.to_be_bytes());

        let psk: SessionKey = psk.into();
        let esk = recipient.encrypt(&psk)?;
        Ok(PKESK3{
            common: Default::default(),
            recipient: recipient.keyid(),
            pk_algo: recipient.pk_algo(),
            esk,
        })
    }

    /// Gets the recipient.
    pub fn recipient(&self) -> &KeyID {
        &self.recipient
    }

    /// Sets the recipient.
    pub fn set_recipient(&mut self, recipient: KeyID) -> KeyID {
        ::std::mem::replace(&mut self.recipient, recipient)
    }

    /// Gets the public key algorithm.
    pub fn pk_algo(&self) -> PublicKeyAlgorithm {
        self.pk_algo
    }

    /// Sets the public key algorithm.
    pub fn set_pk_algo(&mut self, algo: PublicKeyAlgorithm) -> PublicKeyAlgorithm {
        ::std::mem::replace(&mut self.pk_algo, algo)
    }

    /// Gets the encrypted session key.
    pub fn esk(&self) -> &Ciphertext {
        &self.esk
    }

    /// Sets the encrypted session key.
    pub fn set_esk(&mut self, esk: Ciphertext) -> Ciphertext {
        ::std::mem::replace(&mut self.esk, esk)
    }

    /// Decrypts the encrypted session key.
    ///
    /// If the symmetric algorithm used to encrypt the message is
    /// known in advance, it should be given as argument.  This allows
    /// us to reduce the side-channel leakage of the decryption
    /// operation for RSA.
    ///
    /// Returns the session key and symmetric algorithm used to
    /// encrypt the following payload.
    ///
    /// Returns `None` on errors.  This prevents leaking information
    /// to an attacker, which could lead to compromise of secret key
    /// material with certain algorithms (RSA).  See [Section 14 of
    /// RFC 4880].
    ///
    ///   [Section 14 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-14
    pub fn decrypt(&self, decryptor: &mut dyn Decryptor,
                   sym_algo_hint: Option<SymmetricAlgorithm>)
        -> Option<(SymmetricAlgorithm, SessionKey)>
    {
        self.decrypt_insecure(decryptor, sym_algo_hint).ok()
    }

    fn decrypt_insecure(&self, decryptor: &mut dyn Decryptor,
                        sym_algo_hint: Option<SymmetricAlgorithm>)
        -> Result<(SymmetricAlgorithm, SessionKey)>
    {
        let plaintext_len = if let Some(s) = sym_algo_hint {
            Some(1 /* cipher octet */ + s.key_size()? + 2 /* chksum */)
        } else {
            None
        };
        let plain = decryptor.decrypt(&self.esk, plaintext_len)?;
        let key_rgn = 1..plain.len().saturating_sub(2);
        let sym_algo: SymmetricAlgorithm = plain[0].into();
        let mut key: SessionKey = vec![0u8; sym_algo.key_size()?].into();

        if key_rgn.len() != sym_algo.key_size()? {
            return Err(Error::MalformedPacket(
                format!("session key has the wrong size (got: {}, expected: {})",
                        key_rgn.len(), sym_algo.key_size()?)).into())
        }

        key.copy_from_slice(&plain[key_rgn]);

        let our_checksum
            = key.iter().map(|&x| x as usize).sum::<usize>() & 0xffff;
        let their_checksum = (plain[plain.len() - 2] as usize) << 8
            | (plain[plain.len() - 1] as usize);

        if their_checksum == our_checksum {
            Ok((sym_algo, key))
        } else {
            Err(Error::MalformedPacket("key checksum wrong".to_string())
                .into())
        }
    }
}

impl From<PKESK3> for super::PKESK {
    fn from(p: PKESK3) -> Self {
        super::PKESK::V3(p)
    }
}

impl From<PKESK3> for Packet {
    fn from(p: PKESK3) -> Self {
        Packet::PKESK(p.into())
    }
}

#[cfg(test)]
impl Arbitrary for super::PKESK {
    fn arbitrary(g: &mut Gen) -> Self {
        PKESK3::arbitrary(g).into()
    }
}

#[cfg(test)]
impl Arbitrary for PKESK3 {
    fn arbitrary(g: &mut Gen) -> Self {
        let (ciphertext, pk_algo) = loop {
            let ciphertext = Ciphertext::arbitrary(g);
            if let Some(pk_algo) = ciphertext.pk_algo() {
                break (ciphertext, pk_algo);
            }
        };

        PKESK3::new(KeyID::arbitrary(g), pk_algo, ciphertext).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cert;
    use crate::PacketPile;
    use crate::packet::*;
    use crate::parse::Parse;
    use crate::serialize::MarshalInto;
    use crate::types::Curve;

    quickcheck! {
        fn roundtrip(p: PKESK3) -> bool {
            let q = PKESK3::from_bytes(&p.to_vec().unwrap()).unwrap();
            assert_eq!(p, q);
            true
        }
    }

    #[test]
    fn decrypt_rsa() {
        if ! PublicKeyAlgorithm::RSAEncryptSign.is_supported() {
            eprintln!("Skipping test, algorithm is not supported.");
            return;
        }

        let cert = Cert::from_bytes(
            crate::tests::key("testy-private.pgp")).unwrap();
        let pile = PacketPile::from_bytes(
            crate::tests::message("encrypted-to-testy.gpg")).unwrap();
        let mut keypair =
            cert.subkeys().next().unwrap()
            .key().clone().parts_into_secret().unwrap().into_keypair().unwrap();

        let pkesk: &PKESK =
            pile.descendants().next().unwrap().downcast_ref().unwrap();

        let plain = pkesk.decrypt(&mut keypair, None).unwrap();
        let plain_ =
            pkesk.decrypt(&mut keypair, Some(SymmetricAlgorithm::AES256))
            .unwrap();
        assert_eq!(plain, plain_);

        eprintln!("plain: {:?}", plain);
    }

    #[test]
    fn decrypt_ecdh_cv25519() {
        if ! (PublicKeyAlgorithm::EdDSA.is_supported()
              && Curve::Ed25519.is_supported()
              && PublicKeyAlgorithm::ECDH.is_supported()
              && Curve::Cv25519.is_supported()) {
            eprintln!("Skipping test, algorithm is not supported.");
            return;
        }

        let cert = Cert::from_bytes(
            crate::tests::key("testy-new-private.pgp")).unwrap();
        let pile = PacketPile::from_bytes(
            crate::tests::message("encrypted-to-testy-new.pgp")).unwrap();
        let mut keypair =
            cert.subkeys().next().unwrap()
            .key().clone().parts_into_secret().unwrap().into_keypair().unwrap();

        let pkesk: &PKESK =
            pile.descendants().next().unwrap().downcast_ref().unwrap();

        let plain = pkesk.decrypt(&mut keypair, None).unwrap();
        let plain_ =
            pkesk.decrypt(&mut keypair, Some(SymmetricAlgorithm::AES256))
            .unwrap();
        assert_eq!(plain, plain_);

        eprintln!("plain: {:?}", plain);
    }

    #[test]
    fn decrypt_ecdh_nistp256() {
        if ! (PublicKeyAlgorithm::ECDSA.is_supported()
              && PublicKeyAlgorithm::ECDH.is_supported()
              && Curve::NistP256.is_supported()) {
            eprintln!("Skipping test, algorithm is not supported.");
            return;
        }

        let cert = Cert::from_bytes(
            crate::tests::key("testy-nistp256-private.pgp")).unwrap();
        let pile = PacketPile::from_bytes(
            crate::tests::message("encrypted-to-testy-nistp256.pgp")).unwrap();
        let mut keypair =
            cert.subkeys().next().unwrap()
            .key().clone().parts_into_secret().unwrap().into_keypair().unwrap();

        let pkesk: &PKESK =
            pile.descendants().next().unwrap().downcast_ref().unwrap();

        let plain = pkesk.decrypt(&mut keypair, None).unwrap();
        let plain_ =
            pkesk.decrypt(&mut keypair, Some(SymmetricAlgorithm::AES256))
            .unwrap();
        assert_eq!(plain, plain_);

        eprintln!("plain: {:?}", plain);
    }

    #[test]
    fn decrypt_ecdh_nistp384() {
        if ! (PublicKeyAlgorithm::ECDSA.is_supported()
              && PublicKeyAlgorithm::ECDH.is_supported()
              && Curve::NistP384.is_supported()) {
            eprintln!("Skipping test, algorithm is not supported.");
            return;
        }

        let cert = Cert::from_bytes(
            crate::tests::key("testy-nistp384-private.pgp")).unwrap();
        let pile = PacketPile::from_bytes(
            crate::tests::message("encrypted-to-testy-nistp384.pgp")).unwrap();
        let mut keypair =
            cert.subkeys().next().unwrap()
            .key().clone().parts_into_secret().unwrap().into_keypair().unwrap();

        let pkesk: &PKESK =
            pile.descendants().next().unwrap().downcast_ref().unwrap();

        let plain = pkesk.decrypt(&mut keypair, None).unwrap();
        let plain_ =
            pkesk.decrypt(&mut keypair, Some(SymmetricAlgorithm::AES256))
            .unwrap();
        assert_eq!(plain, plain_);

        eprintln!("plain: {:?}", plain);
    }

    #[test]
    fn decrypt_ecdh_nistp521() {
        if ! (PublicKeyAlgorithm::ECDSA.is_supported()
              && PublicKeyAlgorithm::ECDH.is_supported()
              && Curve::NistP521.is_supported()) {
            eprintln!("Skipping test, algorithm is not supported.");
            return;
        }

        let cert = Cert::from_bytes(
            crate::tests::key("testy-nistp521-private.pgp")).unwrap();
        let pile = PacketPile::from_bytes(
            crate::tests::message("encrypted-to-testy-nistp521.pgp")).unwrap();
        let mut keypair =
            cert.subkeys().next().unwrap()
            .key().clone().parts_into_secret().unwrap().into_keypair().unwrap();

        let pkesk: &PKESK =
            pile.descendants().next().unwrap().downcast_ref().unwrap();

        let plain = pkesk.decrypt(&mut keypair, None).unwrap();
        let plain_ =
            pkesk.decrypt(&mut keypair, Some(SymmetricAlgorithm::AES256))
            .unwrap();
        assert_eq!(plain, plain_);

        eprintln!("plain: {:?}", plain);
    }


    #[test]
    fn decrypt_with_short_cv25519_secret_key() {
        if ! (PublicKeyAlgorithm::ECDH.is_supported()
              && Curve::Cv25519.is_supported()) {
            eprintln!("Skipping test, algorithm is not supported.");
            return;
        }

        use super::PKESK3;
        use crate::crypto::SessionKey;
        use crate::{HashAlgorithm, SymmetricAlgorithm};
        use crate::packet::key::{Key4, UnspecifiedRole};

        // 20 byte sec key
        let mut secret_key = [
            0x0,0x0,
            0x0,0x0,0x0,0x0,0x0,0x0,0x0,0x0,0x0,0x0,
            0x1,0x2,0x2,0x2,0x2,0x2,0x2,0x2,0x2,0x2,
            0x1,0x2,0x2,0x2,0x2,0x2,0x2,0x2,0x0,0x0
        ];
        // Ensure that the key is at least somewhat valid, according to the
        // generation procedure specified in "Responsibilities of the user":
        // https://cr.yp.to/ecdh/curve25519-20060209.pdf#page=5
        // Only perform the bit-twiddling on the last byte. This is done so that
        // we can still have somewhat defined multiplication while still testing
        // the "short" key logic.
        // secret_key[0] &= 0xf8;
        secret_key[31] &= 0x7f;
        secret_key[31] |= 0x40;

        let key: Key<_, UnspecifiedRole> = Key4::import_secret_cv25519(
            &secret_key,
            HashAlgorithm::SHA256,
            SymmetricAlgorithm::AES256,
            None,
        ).unwrap().into();

        let sess_key = SessionKey::new(32);
        let pkesk = PKESK3::for_recipient(SymmetricAlgorithm::AES256, &sess_key,
                                          &key).unwrap();
        let mut keypair = key.into_keypair().unwrap();
        pkesk.decrypt(&mut keypair, None).unwrap();
    }

    /// Insufficient validation of RSA ciphertexts crash Nettle.
    ///
    /// See CVE-2021-3580.
    #[test]
    fn cve_2021_3580_ciphertext_too_long() -> Result<()> {
        if ! PublicKeyAlgorithm::RSAEncryptSign.is_supported() {
            eprintln!("Skipping test, algorithm is not supported.");
            return Ok(());
        }

        // Get (any) 2k RSA key.
        let cert = Cert::from_bytes(
            crate::tests::key("testy-private.pgp"))?;
        let mut keypair = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;

        let pile = PacketPile::from_bytes(b"-----BEGIN PGP ARMORED FILE-----

wcDNAwAAAAAAAAAAAQwGI5SkpcRMjkiOKx332kxv+2Xh4y1QTefPilKOPOlHYFa0
rnnLaQVEACKJNQ38YuCFUvtpK4IN2grjlj71IP24+KDp3ZuVWnVTS6JcyE10Y9iq
uGvKdS0C17XCze2LD4ouVOrUZHGXpeDT47w6DsHb/0UE85h56wpk2CzO1XFQzHxX
HR2DDLqqeFVzTv0peYiQfLHl7kWXijTNEqmYhFCzxuICXzuClAAJM+fVIRfcm2tm
2R4AxOQGv9DlWfZwbkpKfj/uuo0CAe21n4NT+NzdVgPlff/hna3yGgPe1B+vjq4e
jfxHg+pvo/HTLkV+c2HAGbM1bCb/5TedGd1nAMSAIOu/J/WQp/l3HtEv63HaVPZJ
JInJ6L/KyPwjm/ieZx5EWOLJgFRWGCrBGnb8T81lkFey7uZR5Xiq+9KoUhHQFw8N
joc0YUVyhUBVFf4B0zVZRUfqZyJtJ07Sl5xppI12U1HQCTjn7Fp8BHMPKuBotYzv
1Q4f00k6Txctw+LDRM17/w==
=VtwB
-----END PGP ARMORED FILE-----
")?;
        let pkesk: &PKESK =
            pile.descendants().next().unwrap().downcast_ref().unwrap();
        // Boom goes the assertion.
        let _ = pkesk.decrypt(&mut keypair, None);

        Ok(())
    }

    /// Insufficient validation of RSA ciphertexts crash Nettle.
    ///
    /// See CVE-2021-3580.
    #[test]
    fn cve_2021_3580_zero_ciphertext() -> Result<()> {
        if ! PublicKeyAlgorithm::RSAEncryptSign.is_supported() {
            eprintln!("Skipping test, algorithm is not supported.");
            return Ok(());
        }

        // Get (any) 2k RSA key.
        let cert = Cert::from_bytes(
            crate::tests::key("testy-private.pgp"))?;
        let mut keypair = cert.primary_key().key().clone()
            .parts_into_secret()?.into_keypair()?;

        let pile = PacketPile::from_bytes(b"-----BEGIN PGP ARMORED FILE-----

wQwDAAAAAAAAAAABAAA=
=H/1T
-----END PGP ARMORED FILE-----
")?;
        let pkesk: &PKESK =
            pile.descendants().next().unwrap().downcast_ref().unwrap();
        // Boom goes the memory safety.
        let _ = pkesk.decrypt(&mut keypair, None);

        Ok(())
    }
}
