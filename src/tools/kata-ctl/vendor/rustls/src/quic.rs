/// This module contains optional APIs for implementing QUIC TLS.
use crate::cipher::{Iv, IvLen};
pub use crate::client::ClientQuicExt;
use crate::conn::CommonState;
use crate::error::Error;
use crate::msgs::enums::AlertDescription;
pub use crate::server::ServerQuicExt;
use crate::suites::BulkAlgorithm;
use crate::tls13::key_schedule::hkdf_expand;
use crate::tls13::{Tls13CipherSuite, TLS13_AES_128_GCM_SHA256_INTERNAL};

use ring::{aead, hkdf};

/// Secrets used to encrypt/decrypt traffic
#[derive(Clone, Debug)]
pub struct Secrets {
    /// Secret used to encrypt packets transmitted by the client
    client: hkdf::Prk,
    /// Secret used to encrypt packets transmitted by the server
    server: hkdf::Prk,
    /// Cipher suite used with these secrets
    suite: &'static Tls13CipherSuite,
    is_client: bool,
}

impl Secrets {
    pub(crate) fn new(
        client: hkdf::Prk,
        server: hkdf::Prk,
        suite: &'static Tls13CipherSuite,
        is_client: bool,
    ) -> Self {
        Self {
            client,
            server,
            suite,
            is_client,
        }
    }

    /// Derive the next set of packet keys
    pub fn next_packet_keys(&mut self) -> PacketKeySet {
        let keys = PacketKeySet::new(self);
        self.update();
        keys
    }

    fn update(&mut self) {
        let hkdf_alg = self.suite.hkdf_algorithm;
        self.client = hkdf_expand(&self.client, hkdf_alg, b"quic ku", &[]);
        self.server = hkdf_expand(&self.server, hkdf_alg, b"quic ku", &[]);
    }

    fn local_remote(&self) -> (&hkdf::Prk, &hkdf::Prk) {
        if self.is_client {
            (&self.client, &self.server)
        } else {
            (&self.server, &self.client)
        }
    }
}

/// Generic methods for QUIC sessions
pub trait QuicExt {
    /// Return the TLS-encoded transport parameters for the session's peer.
    ///
    /// While the transport parameters are technically available prior to the
    /// completion of the handshake, they cannot be fully trusted until the
    /// handshake completes, and reliance on them should be minimized.
    /// However, any tampering with the parameters will cause the handshake
    /// to fail.
    fn quic_transport_parameters(&self) -> Option<&[u8]>;

    /// Compute the keys for encrypting/decrypting 0-RTT packets, if available
    fn zero_rtt_keys(&self) -> Option<DirectionalKeys>;

    /// Consume unencrypted TLS handshake data.
    ///
    /// Handshake data obtained from separate encryption levels should be supplied in separate calls.
    fn read_hs(&mut self, plaintext: &[u8]) -> Result<(), Error>;

    /// Emit unencrypted TLS handshake data.
    ///
    /// When this returns `Some(_)`, the new keys must be used for future handshake data.
    fn write_hs(&mut self, buf: &mut Vec<u8>) -> Option<KeyChange>;

    /// Emit the TLS description code of a fatal alert, if one has arisen.
    ///
    /// Check after `read_hs` returns `Err(_)`.
    fn alert(&self) -> Option<AlertDescription>;
}

/// Keys used to communicate in a single direction
pub struct DirectionalKeys {
    /// Encrypts or decrypts a packet's headers
    pub header: HeaderProtectionKey,
    /// Encrypts or decrypts the payload of a packet
    pub packet: PacketKey,
}

impl DirectionalKeys {
    pub(crate) fn new(suite: &'static Tls13CipherSuite, secret: &hkdf::Prk) -> Self {
        Self {
            header: HeaderProtectionKey::new(suite, secret),
            packet: PacketKey::new(suite, secret),
        }
    }
}

/// A QUIC header protection key
pub struct HeaderProtectionKey(aead::quic::HeaderProtectionKey);

impl HeaderProtectionKey {
    fn new(suite: &'static Tls13CipherSuite, secret: &hkdf::Prk) -> Self {
        let alg = match suite.common.bulk {
            BulkAlgorithm::Aes128Gcm => &aead::quic::AES_128,
            BulkAlgorithm::Aes256Gcm => &aead::quic::AES_256,
            BulkAlgorithm::Chacha20Poly1305 => &aead::quic::CHACHA20,
        };

        Self(hkdf_expand(secret, alg, b"quic hp", &[]))
    }

    /// Adds QUIC Header Protection.
    ///
    /// `sample` must contain the sample of encrypted payload; see
    /// [Header Protection Sample].
    ///
    /// `first` must reference the first byte of the header, referred to as
    /// `packet[0]` in [Header Protection Application].
    ///
    /// `packet_number` must reference the Packet Number field; this is
    /// `packet[pn_offset:pn_offset+pn_length]` in [Header Protection Application].
    ///
    /// Returns an error without modifying anything if `sample` is not
    /// the correct length (see [Header Protection Sample] and [`Self::sample_len()`]),
    /// or `packet_number` is longer than allowed (see [Packet Number Encoding and Decoding]).
    ///
    /// Otherwise, `first` and `packet_number` will have the header protection added.
    ///
    /// [Header Protection Application]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.1
    /// [Header Protection Sample]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.2
    /// [Packet Number Encoding and Decoding]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.1
    #[inline]
    pub fn encrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), Error> {
        self.xor_in_place(sample, first, packet_number, false)
    }

    /// Removes QUIC Header Protection.
    ///
    /// `sample` must contain the sample of encrypted payload; see
    /// [Header Protection Sample].
    ///
    /// `first` must reference the first byte of the header, referred to as
    /// `packet[0]` in [Header Protection Application].
    ///
    /// `packet_number` must reference the Packet Number field; this is
    /// `packet[pn_offset:pn_offset+pn_length]` in [Header Protection Application].
    ///
    /// Returns an error without modifying anything if `sample` is not
    /// the correct length (see [Header Protection Sample] and [`Self::sample_len()`]),
    /// or `packet_number` is longer than allowed (see
    /// [Packet Number Encoding and Decoding]).
    ///
    /// Otherwise, `first` and `packet_number` will have the header protection removed.
    ///
    /// [Header Protection Application]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.1
    /// [Header Protection Sample]: https://datatracker.ietf.org/doc/html/rfc9001#section-5.4.2
    /// [Packet Number Encoding and Decoding]: https://datatracker.ietf.org/doc/html/rfc9000#section-17.1
    #[inline]
    pub fn decrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), Error> {
        self.xor_in_place(sample, first, packet_number, true)
    }

    fn xor_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
        masked: bool,
    ) -> Result<(), Error> {
        // This implements [Header Protection Application] almost verbatim.

        let mask = self
            .0
            .new_mask(sample)
            .map_err(|_| Error::General("sample of invalid length".into()))?;

        // The `unwrap()` will not panic because `new_mask` returns a
        // non-empty result.
        let (first_mask, pn_mask) = mask.split_first().unwrap();

        // It is OK for the `mask` to be longer than `packet_number`,
        // but a valid `packet_number` will never be longer than `mask`.
        if packet_number.len() > pn_mask.len() {
            return Err(Error::General("packet number too long".into()));
        }

        // Infallible from this point on. Before this point, `first` and
        // `packet_number` are unchanged.

        const LONG_HEADER_FORM: u8 = 0x80;
        let bits = match *first & LONG_HEADER_FORM == LONG_HEADER_FORM {
            true => 0x0f,  // Long header: 4 bits masked
            false => 0x1f, // Short header: 5 bits masked
        };

        let first_plain = match masked {
            // When unmasking, use the packet length bits after unmasking
            true => (*first ^ (first_mask & bits)),
            // When masking, use the packet length bits before masking
            false => *first,
        };
        let pn_len = (first_plain & 0x03) as usize + 1;

        *first ^= first_mask & bits;
        for (dst, m) in packet_number
            .iter_mut()
            .zip(pn_mask)
            .take(pn_len)
        {
            *dst ^= m;
        }

        Ok(())
    }

    /// Expected sample length for the key's algorithm
    #[inline]
    pub fn sample_len(&self) -> usize {
        self.0.algorithm().sample_len()
    }
}

/// Keys to encrypt or decrypt the payload of a packet
pub struct PacketKey {
    /// Encrypts or decrypts a packet's payload
    key: aead::LessSafeKey,
    /// Computes unique nonces for each packet
    iv: Iv,
    /// The cipher suite used for this packet key
    suite: &'static Tls13CipherSuite,
}

impl PacketKey {
    fn new(suite: &'static Tls13CipherSuite, secret: &hkdf::Prk) -> Self {
        Self {
            key: aead::LessSafeKey::new(hkdf_expand(
                secret,
                suite.common.aead_algorithm,
                b"quic key",
                &[],
            )),
            iv: hkdf_expand(secret, IvLen, b"quic iv", &[]),
            suite,
        }
    }

    /// Encrypt a QUIC packet
    ///
    /// Takes a `packet_number`, used to derive the nonce; the packet `header`, which is used as
    /// the additional authenticated data; and the `payload`. The authentication tag is returned if
    /// encryption succeeds.
    ///
    /// Fails iff the payload is longer than allowed by the cipher suite's AEAD algorithm.
    pub fn encrypt_in_place(
        &self,
        packet_number: u64,
        header: &[u8],
        payload: &mut [u8],
    ) -> Result<Tag, Error> {
        let aad = aead::Aad::from(header);
        let nonce = nonce_for(packet_number, &self.iv);
        let tag = self
            .key
            .seal_in_place_separate_tag(nonce, aad, payload)
            .map_err(|_| Error::EncryptError)?;
        Ok(Tag(tag))
    }

    /// Decrypt a QUIC packet
    ///
    /// Takes the packet `header`, which is used as the additional authenticated data, and the
    /// `payload`, which includes the authentication tag.
    ///
    /// If the return value is `Ok`, the decrypted payload can be found in `payload`, up to the
    /// length found in the return value.
    pub fn decrypt_in_place<'a>(
        &self,
        packet_number: u64,
        header: &[u8],
        payload: &'a mut [u8],
    ) -> Result<&'a [u8], Error> {
        let payload_len = payload.len();
        let aad = aead::Aad::from(header);
        let nonce = nonce_for(packet_number, &self.iv);
        self.key
            .open_in_place(nonce, aad, payload)
            .map_err(|_| Error::DecryptError)?;

        let plain_len = payload_len - self.key.algorithm().tag_len();
        Ok(&payload[..plain_len])
    }

    /// Number of times the packet key can be used without sacrificing confidentiality
    ///
    /// See <https://www.rfc-editor.org/rfc/rfc9001.html#name-confidentiality-limit>.
    #[inline]
    pub fn confidentiality_limit(&self) -> u64 {
        self.suite.confidentiality_limit
    }

    /// Number of times the packet key can be used without sacrificing integrity
    ///
    /// See <https://www.rfc-editor.org/rfc/rfc9001.html#name-integrity-limit>.
    #[inline]
    pub fn integrity_limit(&self) -> u64 {
        self.suite.integrity_limit
    }

    /// Tag length for the underlying AEAD algorithm
    #[inline]
    pub fn tag_len(&self) -> usize {
        self.key.algorithm().tag_len()
    }
}

/// AEAD tag, must be appended to encrypted cipher text
pub struct Tag(aead::Tag);

impl AsRef<[u8]> for Tag {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// Packet protection keys for bidirectional 1-RTT communication
pub struct PacketKeySet {
    /// Encrypts outgoing packets
    pub local: PacketKey,
    /// Decrypts incoming packets
    pub remote: PacketKey,
}

impl PacketKeySet {
    fn new(secrets: &Secrets) -> Self {
        let (local, remote) = secrets.local_remote();
        Self {
            local: PacketKey::new(secrets.suite, local),
            remote: PacketKey::new(secrets.suite, remote),
        }
    }
}

/// Complete set of keys used to communicate with the peer
pub struct Keys {
    /// Encrypts outgoing packets
    pub local: DirectionalKeys,
    /// Decrypts incoming packets
    pub remote: DirectionalKeys,
}

impl Keys {
    /// Construct keys for use with initial packets
    pub fn initial(version: Version, client_dst_connection_id: &[u8], is_client: bool) -> Self {
        const CLIENT_LABEL: &[u8] = b"client in";
        const SERVER_LABEL: &[u8] = b"server in";
        let salt = version.initial_salt();
        let hs_secret = hkdf::Salt::new(hkdf::HKDF_SHA256, salt).extract(client_dst_connection_id);

        let secrets = Secrets {
            client: hkdf_expand(&hs_secret, hkdf::HKDF_SHA256, CLIENT_LABEL, &[]),
            server: hkdf_expand(&hs_secret, hkdf::HKDF_SHA256, SERVER_LABEL, &[]),
            suite: TLS13_AES_128_GCM_SHA256_INTERNAL,
            is_client,
        };
        Self::new(&secrets)
    }

    fn new(secrets: &Secrets) -> Self {
        let (local, remote) = secrets.local_remote();
        Self {
            local: DirectionalKeys::new(secrets.suite, local),
            remote: DirectionalKeys::new(secrets.suite, remote),
        }
    }
}

pub(crate) fn write_hs(this: &mut CommonState, buf: &mut Vec<u8>) -> Option<KeyChange> {
    while let Some((_, msg)) = this.quic.hs_queue.pop_front() {
        buf.extend_from_slice(&msg);
        if let Some(&(true, _)) = this.quic.hs_queue.front() {
            if this.quic.hs_secrets.is_some() {
                // Allow the caller to switch keys before proceeding.
                break;
            }
        }
    }

    if let Some(secrets) = this.quic.hs_secrets.take() {
        return Some(KeyChange::Handshake {
            keys: Keys::new(&secrets),
        });
    }

    if let Some(mut secrets) = this.quic.traffic_secrets.take() {
        if !this.quic.returned_traffic_keys {
            this.quic.returned_traffic_keys = true;
            let keys = Keys::new(&secrets);
            secrets.update();
            return Some(KeyChange::OneRtt {
                keys,
                next: secrets,
            });
        }
    }

    None
}

/// Key material for use in QUIC packet spaces
///
/// QUIC uses 4 different sets of keys (and progressive key updates for long-running connections):
///
/// * Initial: these can be created from [`Keys::initial()`]
/// * 0-RTT keys: can be retrieved from [`QuicExt::zero_rtt_keys()`]
/// * Handshake: these are returned from [`QuicExt::write_hs()`] after `ClientHello` and
///   `ServerHello` messages have been exchanged
/// * 1-RTT keys: these are returned from [`QuicExt::write_hs()`] after the handshake is done
///
/// Once the 1-RTT keys have been exchanged, either side may initiate a key update. Progressive
/// update keys can be obtained from the [`Secrets`] returned in [`KeyChange::OneRtt`]. Note that
/// only packet keys are updated by key updates; header protection keys remain the same.
#[allow(clippy::large_enum_variant)]
pub enum KeyChange {
    /// Keys for the handshake space
    Handshake {
        /// Header and packet keys for the handshake space
        keys: Keys,
    },
    /// Keys for 1-RTT data
    OneRtt {
        /// Header and packet keys for 1-RTT data
        keys: Keys,
        /// Secrets to derive updated keys from
        next: Secrets,
    },
}

/// Compute the nonce to use for encrypting or decrypting `packet_number`
fn nonce_for(packet_number: u64, iv: &Iv) -> ring::aead::Nonce {
    let mut out = [0; aead::NONCE_LEN];
    out[4..].copy_from_slice(&packet_number.to_be_bytes());
    for (out, inp) in out.iter_mut().zip(iv.0.iter()) {
        *out ^= inp;
    }
    aead::Nonce::assume_unique_for_key(out)
}

/// QUIC protocol version
///
/// Governs version-specific behavior in the TLS layer
#[non_exhaustive]
#[derive(Clone, Copy)]
pub enum Version {
    /// Draft versions 29, 30, 31 and 32
    V1Draft,
    /// First stable RFC
    V1,
}

impl Version {
    fn initial_salt(self) -> &'static [u8; 20] {
        match self {
            Self::V1Draft => &[
                // https://datatracker.ietf.org/doc/html/draft-ietf-quic-tls-32#section-5.2
                0xaf, 0xbf, 0xec, 0x28, 0x99, 0x93, 0xd2, 0x4c, 0x9e, 0x97, 0x86, 0xf1, 0x9c, 0x61,
                0x11, 0xe0, 0x43, 0x90, 0xa8, 0x99,
            ],
            Self::V1 => &[
                // https://www.rfc-editor.org/rfc/rfc9001.html#name-initial-secrets
                0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8,
                0x0c, 0xad, 0xcc, 0xbb, 0x7f, 0x0a,
            ],
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn short_packet_header_protection() {
        // https://www.rfc-editor.org/rfc/rfc9001.html#name-chacha20-poly1305-short-hea

        const PN: u64 = 654360564;
        const SECRET: &[u8] = &[
            0x9a, 0xc3, 0x12, 0xa7, 0xf8, 0x77, 0x46, 0x8e, 0xbe, 0x69, 0x42, 0x27, 0x48, 0xad,
            0x00, 0xa1, 0x54, 0x43, 0xf1, 0x82, 0x03, 0xa0, 0x7d, 0x60, 0x60, 0xf6, 0x88, 0xf3,
            0x0f, 0x21, 0x63, 0x2b,
        ];

        let secret = hkdf::Prk::new_less_safe(hkdf::HKDF_SHA256, SECRET);
        use crate::tls13::TLS13_CHACHA20_POLY1305_SHA256_INTERNAL;
        let hpk = HeaderProtectionKey::new(TLS13_CHACHA20_POLY1305_SHA256_INTERNAL, &secret);
        let packet = PacketKey::new(TLS13_CHACHA20_POLY1305_SHA256_INTERNAL, &secret);

        const PLAIN: &[u8] = &[0x42, 0x00, 0xbf, 0xf4, 0x01];

        let mut buf = PLAIN.to_vec();
        let (header, payload) = buf.split_at_mut(4);
        let tag = packet
            .encrypt_in_place(PN, &*header, payload)
            .unwrap();
        buf.extend(tag.as_ref());

        let pn_offset = 1;
        let (header, sample) = buf.split_at_mut(pn_offset + 4);
        let (first, rest) = header.split_at_mut(1);
        let sample = &sample[..hpk.sample_len()];
        hpk.encrypt_in_place(sample, &mut first[0], dbg!(rest))
            .unwrap();

        const PROTECTED: &[u8] = &[
            0x4c, 0xfe, 0x41, 0x89, 0x65, 0x5e, 0x5c, 0xd5, 0x5c, 0x41, 0xf6, 0x90, 0x80, 0x57,
            0x5d, 0x79, 0x99, 0xc2, 0x5a, 0x5b, 0xfb,
        ];

        assert_eq!(&buf, PROTECTED);

        let (header, sample) = buf.split_at_mut(pn_offset + 4);
        let (first, rest) = header.split_at_mut(1);
        let sample = &sample[..hpk.sample_len()];
        hpk.decrypt_in_place(sample, &mut first[0], rest)
            .unwrap();

        let (header, payload_tag) = buf.split_at_mut(4);
        let plain = packet
            .decrypt_in_place(PN, &*header, payload_tag)
            .unwrap();

        assert_eq!(plain, &PLAIN[4..]);
    }

    #[test]
    fn key_update_test_vector() {
        fn equal_prk(x: &hkdf::Prk, y: &hkdf::Prk) -> bool {
            let mut x_data = [0; 16];
            let mut y_data = [0; 16];
            let x_okm = x
                .expand(&[b"info"], &aead::quic::AES_128)
                .unwrap();
            x_okm.fill(&mut x_data[..]).unwrap();
            let y_okm = y
                .expand(&[b"info"], &aead::quic::AES_128)
                .unwrap();
            y_okm.fill(&mut y_data[..]).unwrap();
            x_data == y_data
        }

        let mut secrets = Secrets {
            // Constant dummy values for reproducibility
            client: hkdf::Prk::new_less_safe(
                hkdf::HKDF_SHA256,
                &[
                    0xb8, 0x76, 0x77, 0x08, 0xf8, 0x77, 0x23, 0x58, 0xa6, 0xea, 0x9f, 0xc4, 0x3e,
                    0x4a, 0xdd, 0x2c, 0x96, 0x1b, 0x3f, 0x52, 0x87, 0xa6, 0xd1, 0x46, 0x7e, 0xe0,
                    0xae, 0xab, 0x33, 0x72, 0x4d, 0xbf,
                ],
            ),
            server: hkdf::Prk::new_less_safe(
                hkdf::HKDF_SHA256,
                &[
                    0x42, 0xdc, 0x97, 0x21, 0x40, 0xe0, 0xf2, 0xe3, 0x98, 0x45, 0xb7, 0x67, 0x61,
                    0x34, 0x39, 0xdc, 0x67, 0x58, 0xca, 0x43, 0x25, 0x9b, 0x87, 0x85, 0x06, 0x82,
                    0x4e, 0xb1, 0xe4, 0x38, 0xd8, 0x55,
                ],
            ),
            suite: TLS13_AES_128_GCM_SHA256_INTERNAL,
            is_client: true,
        };
        secrets.update();

        assert!(equal_prk(
            &secrets.client,
            &hkdf::Prk::new_less_safe(
                hkdf::HKDF_SHA256,
                &[
                    0x42, 0xca, 0xc8, 0xc9, 0x1c, 0xd5, 0xeb, 0x40, 0x68, 0x2e, 0x43, 0x2e, 0xdf,
                    0x2d, 0x2b, 0xe9, 0xf4, 0x1a, 0x52, 0xca, 0x6b, 0x22, 0xd8, 0xe6, 0xcd, 0xb1,
                    0xe8, 0xac, 0xa9, 0x6, 0x1f, 0xce
                ]
            )
        ));
        assert!(equal_prk(
            &secrets.server,
            &hkdf::Prk::new_less_safe(
                hkdf::HKDF_SHA256,
                &[
                    0xeb, 0x7f, 0x5e, 0x2a, 0x12, 0x3f, 0x40, 0x7d, 0xb4, 0x99, 0xe3, 0x61, 0xca,
                    0xe5, 0x90, 0xd4, 0xd9, 0x92, 0xe1, 0x4b, 0x7a, 0xce, 0x3, 0xc2, 0x44, 0xe0,
                    0x42, 0x21, 0x15, 0xb6, 0xd3, 0x8a
                ]
            )
        ));
    }
}
