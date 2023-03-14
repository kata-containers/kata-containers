use num_bigint::traits::ModInverse;
use num_bigint::Sign::Plus;
use num_bigint::{BigInt, BigUint};
use num_traits::{FromPrimitive, One};
use rand::{rngs::ThreadRng, Rng};
#[cfg(feature = "serde")]
use serde_crate::{Deserialize, Serialize};
use std::ops::Deref;
use zeroize::Zeroize;

use crate::algorithms::generate_multi_prime_key;
use crate::errors::{Error, Result};

use crate::padding::PaddingScheme;
use crate::raw::{DecryptionPrimitive, EncryptionPrimitive};
use crate::{oaep, pkcs1v15, pss};

lazy_static! {
    static ref MIN_PUB_EXPONENT: BigUint = BigUint::from_u64(2).unwrap();
    static ref MAX_PUB_EXPONENT: BigUint = BigUint::from_u64(1 << (31 - 1)).unwrap();
}

pub trait PublicKeyParts {
    /// Returns the modulus of the key.
    fn n(&self) -> &BigUint;

    /// Returns the public exponent of the key.
    fn e(&self) -> &BigUint;

    /// Returns the modulus size in bytes. Raw signatures and ciphertexts for
    /// or by this public key will have the same size.
    fn size(&self) -> usize {
        (self.n().bits() + 7) / 8
    }
}

pub trait PrivateKey: DecryptionPrimitive + PublicKeyParts {}

/// Represents the public part of an RSA key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct RSAPublicKey {
    n: BigUint,
    e: BigUint,
}

/// Represents a whole RSA key, public and private parts.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct RSAPrivateKey {
    /// Public components of the private key.
    pubkey_components: RSAPublicKey,
    /// Private exponent
    d: BigUint,
    /// Prime factors of N, contains >= 2 elements.
    primes: Vec<BigUint>,
    /// precomputed values to speed up private operations
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) precomputed: Option<PrecomputedValues>,
}

impl PartialEq for RSAPrivateKey {
    #[inline]
    fn eq(&self, other: &RSAPrivateKey) -> bool {
        self.pubkey_components == other.pubkey_components
            && self.d == other.d
            && self.primes == other.primes
    }
}

impl Eq for RSAPrivateKey {}

impl Zeroize for RSAPrivateKey {
    fn zeroize(&mut self) {
        self.d.zeroize();
        for prime in self.primes.iter_mut() {
            prime.zeroize();
        }
        self.primes.clear();
        if self.precomputed.is_some() {
            self.precomputed.take().unwrap().zeroize();
        }
    }
}

impl Drop for RSAPrivateKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl Deref for RSAPrivateKey {
    type Target = RSAPublicKey;
    fn deref(&self) -> &RSAPublicKey {
        &self.pubkey_components
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PrecomputedValues {
    /// D mod (P-1)
    pub(crate) dp: BigUint,
    /// D mod (Q-1)
    pub(crate) dq: BigUint,
    /// Q^-1 mod P
    pub(crate) qinv: BigInt,

    /// CRTValues is used for the 3rd and subsequent primes. Due to a
    /// historical accident, the CRT for the first two primes is handled
    /// differently in PKCS#1 and interoperability is sufficiently
    /// important that we mirror this.
    pub(crate) crt_values: Vec<CRTValue>,
}

impl Zeroize for PrecomputedValues {
    fn zeroize(&mut self) {
        self.dp.zeroize();
        self.dq.zeroize();
        self.qinv.zeroize();
        for val in self.crt_values.iter_mut() {
            val.zeroize();
        }
        self.crt_values.clear();
    }
}

impl Drop for PrecomputedValues {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Contains the precomputed Chinese remainder theorem values.
#[derive(Debug, Clone, Zeroize)]
pub(crate) struct CRTValue {
    /// D mod (prime - 1)
    pub(crate) exp: BigInt,
    /// R·Coeff ≡ 1 mod Prime.
    pub(crate) coeff: BigInt,
    /// product of primes prior to this (inc p and q)
    pub(crate) r: BigInt,
}

impl From<RSAPrivateKey> for RSAPublicKey {
    fn from(private_key: RSAPrivateKey) -> Self {
        (&private_key).into()
    }
}

impl From<&RSAPrivateKey> for RSAPublicKey {
    fn from(private_key: &RSAPrivateKey) -> Self {
        let n = private_key.n.clone();
        let e = private_key.e.clone();

        RSAPublicKey { n, e }
    }
}

/// Generic trait for operations on a public key.
pub trait PublicKey: EncryptionPrimitive + PublicKeyParts {
    /// Encrypt the given message.
    fn encrypt<R: Rng>(&self, rng: &mut R, padding: PaddingScheme, msg: &[u8]) -> Result<Vec<u8>>;

    /// Verify a signed message.
    /// `hashed`must be the result of hashing the input using the hashing function
    /// passed in through `hash`.
    /// If the message is valid `Ok(())` is returned, otherwiese an `Err` indicating failure.
    fn verify(&self, padding: PaddingScheme, hashed: &[u8], sig: &[u8]) -> Result<()>;
}

impl PublicKeyParts for RSAPublicKey {
    fn n(&self) -> &BigUint {
        &self.n
    }

    fn e(&self) -> &BigUint {
        &self.e
    }
}

impl PublicKey for RSAPublicKey {
    fn encrypt<R: Rng>(&self, rng: &mut R, padding: PaddingScheme, msg: &[u8]) -> Result<Vec<u8>> {
        match padding {
            PaddingScheme::PKCS1v15Encrypt => pkcs1v15::encrypt(rng, self, msg),
            PaddingScheme::OAEP { mut digest, label } => {
                oaep::encrypt(rng, self, msg, &mut *digest, label)
            }
            _ => Err(Error::InvalidPaddingScheme),
        }
    }

    fn verify(&self, padding: PaddingScheme, hashed: &[u8], sig: &[u8]) -> Result<()> {
        match padding {
            PaddingScheme::PKCS1v15Sign { ref hash } => {
                pkcs1v15::verify(self, hash.as_ref(), hashed, sig)
            }
            PaddingScheme::PSS { mut digest, .. } => pss::verify(self, hashed, sig, &mut *digest),
            _ => Err(Error::InvalidPaddingScheme),
        }
    }
}

impl RSAPublicKey {
    /// Create a new key from its components.
    pub fn new(n: BigUint, e: BigUint) -> Result<Self> {
        let k = RSAPublicKey { n, e };
        check_public(&k)?;

        Ok(k)
    }

    /// Parse a `PKCS1` encoded RSA Public Key.
    ///
    /// The `der` data is expected to be the `base64` decoded content
    /// following a `-----BEGIN RSA PUBLIC KEY-----` header.
    ///
    /// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
    ///
    /// # Example
    ///
    /// ```
    /// use rsa::RSAPublicKey;
    ///
    /// # // openssl rsa -pubin -in tiny_key.pub.pem -RSAPublicKey_out
    /// let file_content = r#"
    /// -----BEGIN RSA PUBLIC KEY-----
    /// MEgCQQCuWe45NapeQ6rkb5T5hYMgQwr3T0NE9rItqDwFJjcNmf6m9kq/wRAaFqWo
    /// BX/BpdSuD0YqSUrnQ5ejf1XW9gmJAgMBAAE=
    /// -----END RSA PUBLIC KEY-----
    /// "#;
    ///
    /// let der_encoded = file_content
    ///     .lines()
    ///     .filter(|line| !line.starts_with("-"))
    ///     .fold(String::new(), |mut data, line| {
    ///         data.push_str(&line);
    ///         data
    ///     });
    /// let der_bytes = base64::decode(&der_encoded).expect("failed to decode base64 content");
    /// let public_key = RSAPublicKey::from_pkcs1(&der_bytes).expect("failed to parse key");
    /// ```
    pub fn from_pkcs1(der: &[u8]) -> Result<RSAPublicKey> {
        crate::parse::parse_public_key_pkcs1(der)
    }

    /// Parse a `PKCS8` encoded RSA Public Key.
    ///
    /// The `der` data is expected to be the `base64` decoded content
    /// following a `-----BEGIN PUBLIC KEY-----` header.
    ///
    /// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
    ///
    /// # Example
    ///
    /// ```
    /// use rsa::RSAPublicKey;
    ///
    /// # // openssl rsa -in tiny_key.pem -outform PEM -pubout -out tiny_key.pub.pem
    /// let file_content = r#"
    /// -----BEGIN PUBLIC KEY-----
    /// MFwwDQYJKoZIhvcNAQEBBQADSwAwSAJBAK5Z7jk1ql5DquRvlPmFgyBDCvdPQ0T2
    /// si2oPAUmNw2Z/qb2Sr/BEBoWpagFf8Gl1K4PRipJSudDl6N/Vdb2CYkCAwEAAQ==
    /// -----END PUBLIC KEY-----
    /// "#;
    ///
    /// let der_encoded = file_content
    ///     .lines()
    ///     .filter(|line| !line.starts_with("-"))
    ///     .fold(String::new(), |mut data, line| {
    ///         data.push_str(&line);
    ///         data
    ///     });
    /// let der_bytes = base64::decode(&der_encoded).expect("failed to decode base64 content");
    /// let public_key = RSAPublicKey::from_pkcs8(&der_bytes).expect("failed to parse key");
    /// ```
    pub fn from_pkcs8(der: &[u8]) -> Result<RSAPublicKey> {
        crate::parse::parse_public_key_pkcs8(der)
    }
}

impl<'a> PublicKeyParts for &'a RSAPublicKey {
    /// Returns the modulus of the key.
    fn n(&self) -> &BigUint {
        &self.n
    }

    /// Returns the public exponent of the key.
    fn e(&self) -> &BigUint {
        &self.e
    }
}

impl<'a> PublicKey for &'a RSAPublicKey {
    fn encrypt<R: Rng>(&self, rng: &mut R, padding: PaddingScheme, msg: &[u8]) -> Result<Vec<u8>> {
        (*self).encrypt(rng, padding, msg)
    }

    fn verify(&self, padding: PaddingScheme, hashed: &[u8], sig: &[u8]) -> Result<()> {
        (*self).verify(padding, hashed, sig)
    }
}

impl PublicKeyParts for RSAPrivateKey {
    fn n(&self) -> &BigUint {
        &self.n
    }

    fn e(&self) -> &BigUint {
        &self.e
    }
}

impl PrivateKey for RSAPrivateKey {}

impl<'a> PublicKeyParts for &'a RSAPrivateKey {
    fn n(&self) -> &BigUint {
        &self.n
    }

    fn e(&self) -> &BigUint {
        &self.e
    }
}

impl<'a> PrivateKey for &'a RSAPrivateKey {}

impl RSAPrivateKey {
    /// Generate a new RSA key pair of the given bit size using the passed in `rng`.
    pub fn new<R: Rng>(rng: &mut R, bit_size: usize) -> Result<RSAPrivateKey> {
        generate_multi_prime_key(rng, 2, bit_size)
    }

    /// Constructs an RSA key pair from the individual components.
    pub fn from_components(
        n: BigUint,
        e: BigUint,
        d: BigUint,
        primes: Vec<BigUint>,
    ) -> RSAPrivateKey {
        let mut k = RSAPrivateKey {
            pubkey_components: RSAPublicKey { n, e },
            d,
            primes,
            precomputed: None,
        };

        // precompute when possible, ignore error otherwise.
        let _ = k.precompute();

        k
    }

    /// Parse a `PKCS1` encoded RSA Private Key.
    ///
    /// The `der` data is expected to be the `base64` decoded content
    /// following a `-----BEGIN RSA PRIVATE KEY-----` header.
    ///
    /// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
    ///
    /// # Example
    ///
    /// ```
    /// use rsa::RSAPrivateKey;
    ///
    /// # // openssl genrsa -out tiny_key.pem 512
    /// let file_content = r#"
    /// -----BEGIN RSA PRIVATE KEY-----
    /// MIIBOwIBAAJBAK5Z7jk1ql5DquRvlPmFgyBDCvdPQ0T2si2oPAUmNw2Z/qb2Sr/B
    /// EBoWpagFf8Gl1K4PRipJSudDl6N/Vdb2CYkCAwEAAQJBAI3vWCfqsE8c9zoQPE8F
    /// icHx0jOSq0ixLExO8M2gVqESq3SJpWbEbvPPbRb1sIqZHe5wV3Xmj09zvUzfdeB7
    /// C6ECIQDjoB/kp7QlRiNhgudhQPct8XUf6Cgp7hBxL2K9Q9UzawIhAMQVvtH1TUOd
    /// aSWiqrFx7w+54o58fIpkecI5Kl0TaWfbAiBrnye1Kn2IKhNMZWIUn2y+8izYeyGS
    /// QZbQjQD4T3wcJQIgKGgWv2teNZ29ai0AIbrJuaLjhdsvStFzqctf6Hg0k1sCIQCj
    /// JdwDGF7Kanex70KAacmOlw3vfx6XWT+2PH6Qh8tLug==
    /// -----END RSA PRIVATE KEY-----
    /// "#;
    ///
    /// let der_encoded = file_content
    ///     .lines()
    ///     .filter(|line| !line.starts_with("-"))
    ///     .fold(String::new(), |mut data, line| {
    ///         data.push_str(&line);
    ///         data
    ///     });
    /// let der_bytes = base64::decode(&der_encoded).expect("failed to decode base64 content");
    /// let private_key = RSAPrivateKey::from_pkcs1(&der_bytes).expect("failed to parse key");
    /// ```
    pub fn from_pkcs1(der: &[u8]) -> Result<RSAPrivateKey> {
        crate::parse::parse_private_key_pkcs1(der)
    }

    /// Parse a `PKCS8` encoded RSA Private Key.
    ///
    /// The `der` data is expected to be the `base64` decoded content
    /// following a `-----BEGIN PRIVATE KEY-----` header.
    ///
    /// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
    ///
    /// # Example
    ///
    /// ```
    /// use rsa::RSAPrivateKey;
    ///
    /// # // openssl pkcs8 -topk8 -inform PEM -outform PEM -in tiny_key.pem -out tiny_key.pkcs8.pem -nocrypt
    /// let file_content = r#"
    /// -----BEGIN PRIVATE KEY-----
    /// MIIBVQIBADANBgkqhkiG9w0BAQEFAASCAT8wggE7AgEAAkEArlnuOTWqXkOq5G+U
    /// +YWDIEMK909DRPayLag8BSY3DZn+pvZKv8EQGhalqAV/waXUrg9GKklK50OXo39V
    /// 1vYJiQIDAQABAkEAje9YJ+qwTxz3OhA8TwWJwfHSM5KrSLEsTE7wzaBWoRKrdIml
    /// ZsRu889tFvWwipkd7nBXdeaPT3O9TN914HsLoQIhAOOgH+SntCVGI2GC52FA9y3x
    /// dR/oKCnuEHEvYr1D1TNrAiEAxBW+0fVNQ51pJaKqsXHvD7nijnx8imR5wjkqXRNp
    /// Z9sCIGufJ7UqfYgqE0xlYhSfbL7yLNh7IZJBltCNAPhPfBwlAiAoaBa/a141nb1q
    /// LQAhusm5ouOF2y9K0XOpy1/oeDSTWwIhAKMl3AMYXspqd7HvQoBpyY6XDe9/HpdZ
    /// P7Y8fpCHy0u6
    /// -----END PRIVATE KEY-----
    /// "#;
    ///
    /// let der_encoded = file_content
    ///     .lines()
    ///     .filter(|line| !line.starts_with("-"))
    ///     .fold(String::new(), |mut data, line| {
    ///         data.push_str(&line);
    ///         data
    ///     });
    /// let der_bytes = base64::decode(&der_encoded).expect("failed to decode base64 content");
    /// let private_key = RSAPrivateKey::from_pkcs8(&der_bytes).expect("failed to parse key");
    /// ```
    pub fn from_pkcs8(der: &[u8]) -> Result<RSAPrivateKey> {
        crate::parse::parse_private_key_pkcs8(der)
    }

    /// Get the public key from the private key, cloning `n` and `e`.
    ///
    /// Generally this is not needed since `RSAPrivateKey` implements the `PublicKey` trait,
    /// but it can occationally be useful to discard the private information entirely.
    pub fn to_public_key(&self) -> RSAPublicKey {
        // Safe to unwrap since n and e are already verified.
        RSAPublicKey::new(self.n().clone(), self.e().clone()).unwrap()
    }

    /// Performs some calculations to speed up private key operations.
    pub fn precompute(&mut self) -> Result<()> {
        if self.precomputed.is_some() {
            return Ok(());
        }

        let dp = &self.d % (&self.primes[0] - BigUint::one());
        let dq = &self.d % (&self.primes[1] - BigUint::one());
        let qinv = self.primes[1]
            .clone()
            .mod_inverse(&self.primes[0])
            .ok_or(Error::InvalidPrime)?;

        let mut r: BigUint = &self.primes[0] * &self.primes[1];
        let crt_values: Vec<CRTValue> = {
            let mut values = Vec::with_capacity(self.primes.len() - 2);
            for prime in &self.primes[2..] {
                let res = CRTValue {
                    exp: BigInt::from_biguint(Plus, &self.d % (prime - BigUint::one())),
                    r: BigInt::from_biguint(Plus, r.clone()),
                    coeff: BigInt::from_biguint(
                        Plus,
                        r.clone()
                            .mod_inverse(prime)
                            .ok_or(Error::InvalidCoefficient)?
                            .to_biguint()
                            .unwrap(),
                    ),
                };
                r *= prime;

                values.push(res);
            }
            values
        };

        self.precomputed = Some(PrecomputedValues {
            dp,
            dq,
            qinv,
            crt_values,
        });

        Ok(())
    }

    /// Returns the private exponent of the key.
    pub fn d(&self) -> &BigUint {
        &self.d
    }

    /// Returns the prime factors.
    pub fn primes(&self) -> &[BigUint] {
        &self.primes
    }

    /// Performs basic sanity checks on the key.
    /// Returns `Ok(())` if everything is good, otherwise an approriate error.
    pub fn validate(&self) -> Result<()> {
        check_public(self)?;

        // Check that Πprimes == n.
        let mut m = BigUint::one();
        for prime in &self.primes {
            // Any primes ≤ 1 will cause divide-by-zero panics later.
            if *prime < BigUint::one() {
                return Err(Error::InvalidPrime);
            }
            m *= prime;
        }
        if m != self.n {
            return Err(Error::InvalidModulus);
        }

        // Check that de ≡ 1 mod p-1, for each prime.
        // This implies that e is coprime to each p-1 as e has a multiplicative
        // inverse. Therefore e is coprime to lcm(p-1,q-1,r-1,...) =
        // exponent(ℤ/nℤ). It also implies that a^de ≡ a mod p as a^(p-1) ≡ 1
        // mod p. Thus a^de ≡ a mod n for all a coprime to n, as required.
        let mut de = self.e.clone();
        de *= self.d.clone();
        for prime in &self.primes {
            let congruence: BigUint = &de % (prime - BigUint::one());
            if !congruence.is_one() {
                return Err(Error::InvalidExponent);
            }
        }

        Ok(())
    }

    /// Decrypt the given message.
    pub fn decrypt(&self, padding: PaddingScheme, ciphertext: &[u8]) -> Result<Vec<u8>> {
        match padding {
            // need to pass any Rng as the type arg, so the type checker is happy, it is not actually used for anything
            PaddingScheme::PKCS1v15Encrypt => {
                pkcs1v15::decrypt::<ThreadRng, _>(None, self, ciphertext)
            }
            PaddingScheme::OAEP { mut digest, label } => {
                oaep::decrypt::<ThreadRng, _>(None, self, ciphertext, &mut *digest, label)
            }
            _ => Err(Error::InvalidPaddingScheme),
        }
    }

    /// Decrypt the given message.
    ///
    /// Uses `rng` to blind the decryption process.
    pub fn decrypt_blinded<R: Rng>(
        &self,
        rng: &mut R,
        padding: PaddingScheme,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>> {
        match padding {
            PaddingScheme::PKCS1v15Encrypt => pkcs1v15::decrypt(Some(rng), self, ciphertext),
            PaddingScheme::OAEP { mut digest, label } => {
                oaep::decrypt(Some(rng), self, ciphertext, &mut *digest, label)
            }
            _ => Err(Error::InvalidPaddingScheme),
        }
    }

    /// Sign the given digest.
    pub fn sign(&self, padding: PaddingScheme, digest_in: &[u8]) -> Result<Vec<u8>> {
        match padding {
            PaddingScheme::PKCS1v15Sign { ref hash } => {
                pkcs1v15::sign::<ThreadRng, _>(None, self, hash.as_ref(), digest_in)
            }
            PaddingScheme::PSS {
                mut salt_rng,
                mut digest,
                salt_len,
            } => pss::sign::<_, ThreadRng, _>(
                &mut *salt_rng,
                None,
                self,
                digest_in,
                salt_len,
                &mut *digest,
            ),
            _ => Err(Error::InvalidPaddingScheme),
        }
    }

    /// Sign the given digest.
    ///
    /// Use `rng` for blinding.
    pub fn sign_blinded<R: Rng>(
        &self,
        rng: &mut R,
        padding: PaddingScheme,
        digest_in: &[u8],
    ) -> Result<Vec<u8>> {
        match padding {
            PaddingScheme::PKCS1v15Sign { ref hash } => {
                pkcs1v15::sign(Some(rng), self, hash.as_ref(), digest_in)
            }
            PaddingScheme::PSS {
                mut salt_rng,
                mut digest,
                salt_len,
            } => pss::sign::<_, R, _>(
                &mut *salt_rng,
                Some(rng),
                self,
                digest_in,
                salt_len,
                &mut *digest,
            ),
            _ => Err(Error::InvalidPaddingScheme),
        }
    }
}

/// Check that the public key is well formed and has an exponent within acceptable bounds.
#[inline]
pub fn check_public(public_key: &impl PublicKeyParts) -> Result<()> {
    if public_key.e() < &*MIN_PUB_EXPONENT {
        return Err(Error::PublicExponentTooSmall);
    }

    if public_key.e() > &*MAX_PUB_EXPONENT {
        return Err(Error::PublicExponentTooLarge);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internals;

    use digest::{Digest, DynDigest};
    use num_traits::{FromPrimitive, ToPrimitive};
    use rand::{distributions::Alphanumeric, rngs::ThreadRng, thread_rng};
    use sha1::Sha1;
    use sha2::{Sha224, Sha256, Sha384, Sha512};
    use sha3::{Sha3_256, Sha3_384, Sha3_512};

    #[test]
    fn test_from_into() {
        let private_key = RSAPrivateKey {
            pubkey_components: RSAPublicKey {
                n: BigUint::from_u64(100).unwrap(),
                e: BigUint::from_u64(200).unwrap(),
            },
            d: BigUint::from_u64(123).unwrap(),
            primes: vec![],
            precomputed: None,
        };
        let public_key: RSAPublicKey = private_key.into();

        assert_eq!(public_key.n().to_u64(), Some(100));
        assert_eq!(public_key.e().to_u64(), Some(200));
    }

    fn test_key_basics(private_key: &RSAPrivateKey) {
        private_key.validate().expect("invalid private key");

        assert!(
            private_key.d() < private_key.n(),
            "private exponent too large"
        );

        let pub_key: RSAPublicKey = private_key.clone().into();
        let m = BigUint::from_u64(42).expect("invalid 42");
        let c = internals::encrypt(&pub_key, &m);
        let m2 = internals::decrypt::<ThreadRng>(None, &private_key, &c)
            .expect("unable to decrypt without blinding");
        assert_eq!(m, m2);
        let mut rng = thread_rng();
        let m3 = internals::decrypt(Some(&mut rng), &private_key, &c)
            .expect("unable to decrypt with blinding");
        assert_eq!(m, m3);
    }

    macro_rules! key_generation {
        ($name:ident, $multi:expr, $size:expr) => {
            #[test]
            fn $name() {
                let mut rng = thread_rng();

                for _ in 0..10 {
                    let private_key = if $multi == 2 {
                        RSAPrivateKey::new(&mut rng, $size).expect("failed to generate key")
                    } else {
                        generate_multi_prime_key(&mut rng, $multi, $size).unwrap()
                    };
                    assert_eq!(private_key.n().bits(), $size);

                    test_key_basics(&private_key);
                }
            }
        };
    }

    key_generation!(key_generation_128, 2, 128);
    key_generation!(key_generation_1024, 2, 1024);

    key_generation!(key_generation_multi_3_256, 3, 256);

    key_generation!(key_generation_multi_4_64, 4, 64);

    key_generation!(key_generation_multi_5_64, 5, 64);
    key_generation!(key_generation_multi_8_576, 8, 576);
    key_generation!(key_generation_multi_16_1024, 16, 1024);

    #[test]
    fn test_impossible_keys() {
        // make sure not infinite loops are hit here.
        let mut rng = thread_rng();
        for i in 0..32 {
            let _ = RSAPrivateKey::new(&mut rng, i).is_err();
            let _ = generate_multi_prime_key(&mut rng, 3, i);
            let _ = generate_multi_prime_key(&mut rng, 4, i);
            let _ = generate_multi_prime_key(&mut rng, 5, i);
        }
    }

    #[test]
    fn test_negative_decryption_value() {
        let private_key = RSAPrivateKey::from_components(
            BigUint::from_bytes_le(&vec![
                99, 192, 208, 179, 0, 220, 7, 29, 49, 151, 75, 107, 75, 73, 200, 180,
            ]),
            BigUint::from_bytes_le(&vec![1, 0, 1]),
            BigUint::from_bytes_le(&vec![
                81, 163, 254, 144, 171, 159, 144, 42, 244, 133, 51, 249, 28, 12, 63, 65,
            ]),
            vec![
                BigUint::from_bytes_le(&vec![105, 101, 60, 173, 19, 153, 3, 192]),
                BigUint::from_bytes_le(&vec![235, 65, 160, 134, 32, 136, 6, 241]),
            ],
        );

        for _ in 0..1000 {
            test_key_basics(&private_key);
        }
    }

    #[test]
    #[cfg(feature = "serde1")]
    fn test_serde() {
        use rand::SeedableRng;
        use rand_xorshift::XorShiftRng;
        use serde_test::{assert_tokens, Token};

        let mut rng = XorShiftRng::from_seed([1; 16]);
        let priv_key = RSAPrivateKey::new(&mut rng, 64).expect("failed to generate key");

        let priv_tokens = [
            Token::Struct {
                name: "RSAPrivateKey",
                len: 3,
            },
            Token::Str("pubkey_components"),
            Token::Struct {
                name: "RSAPublicKey",
                len: 2,
            },
            Token::Str("n"),
            Token::Seq { len: Some(2) },
            Token::U32(1296829443),
            Token::U32(2444363981),
            Token::SeqEnd,
            Token::Str("e"),
            Token::Seq { len: Some(1) },
            Token::U32(65537),
            Token::SeqEnd,
            Token::StructEnd,
            Token::Str("d"),
            Token::Seq { len: Some(2) },
            Token::U32(298985985),
            Token::U32(2349628418),
            Token::SeqEnd,
            Token::Str("primes"),
            Token::Seq { len: Some(2) },
            Token::Seq { len: Some(1) },
            Token::U32(3238068481),
            Token::SeqEnd,
            Token::Seq { len: Some(1) },
            Token::U32(3242199299),
            Token::SeqEnd,
            Token::SeqEnd,
            Token::StructEnd,
        ];
        assert_tokens(&priv_key, &priv_tokens);

        let priv_tokens = [
            Token::Struct {
                name: "RSAPublicKey",
                len: 2,
            },
            Token::Str("n"),
            Token::Seq { len: Some(2) },
            Token::U32(1296829443),
            Token::U32(2444363981),
            Token::SeqEnd,
            Token::Str("e"),
            Token::Seq { len: Some(1) },
            Token::U32(65537),
            Token::SeqEnd,
            Token::StructEnd,
        ];
        assert_tokens(&RSAPublicKey::from(priv_key), &priv_tokens);
    }

    #[test]
    fn invalid_coeff_private_key_regression() {
        let n = base64::decode("wC8GyQvTCZOK+iiBR5fGQCmzRCTWX9TQ3aRG5gGFk0wB6EFoLMAyEEqeG3gS8xhAm2rSWYx9kKufvNat3iWlbSRVqkcbpVAYlj2vTrpqDpJl+6u+zxFYoUEBevlJJkAhl8EuCccOA30fVpcfRvXPTtvRd3yFT9E9EwZljtgSI02w7gZwg7VIxaGeajh5Euz6ZVQZ+qNRKgXrRC7gPRqVyI6Dt0Jc+Su5KBGNn0QcPDzOahWha1ieaeMkFisZ9mdpsJoZ4tw5eicLaUomKzALHXQVt+/rcZSrCd6/7uUo11B/CYBM4UfSpwXaL88J9AE6A5++no9hmJzaF2LLp+Qwx4yY3j9TDutxSAjsraxxJOGZ3XyA9nG++Ybt3cxZ5fP7ROjxCfROBmVv5dYn0O9OBIqYeCH6QraNpZMadlLNIhyMv8Y+P3r5l/PaK4VJaEi5pPosnEPawp0W0yZDzmjk2z1LthaRx0aZVrAjlH0Rb/6goLUQ9qu1xsDtQVVpN4A89ZUmtTWORnnJr0+595eHHxssd2gpzqf4bPjNITdAEuOCCtpvyi4ls23zwuzryUYjcUOEnsXNQ+DrZpLKxdtsD/qNV/j1hfeyBoPllC3cV+6bcGOFcVGbjYqb+Kw1b0+jL69RSKQqgmS+qYqr8c48nDRxyq3QXhR8qtzUwBFSLVk=").unwrap();
        let e = base64::decode("AQAB").unwrap();
        let d = base64::decode("qQazSQ+FRN7nVK1bRsROMRB8AmsDwLVEHivlz1V3Td2Dr+oW3YUMgxedhztML1IdQJPq/ad6qErJ6yRFNySVIjDaxzBTOEoB1eHa1btOnBJWb8rVvvjaorixvJ6Tn3i4EuhsvVy9DoR1k4rGj3qSIiFjUVvLRDAbLyhpGgEfsr0Z577yJmTC5E8JLRMOKX8Tmxsk3jPVpsgd65Hu1s8S/ZmabwuHCf9SkdMeY/1bd/9i7BqqJeeDLE4B5x1xcC3z3scqDUTzqGO+vZPhjgprPDRlBamVwgenhr7KwCn8iaLamFinRVwOAag8BeBqOJj7lURiOsKQa9FIX1kdFUS1QMQxgtPycLjkbvCJjriqT7zWKsmJ7l8YLs6Wmm9/+QJRwNCEVdMTXKfCP1cJjudaiskEQThfUldtgu8gUDNYbQ/Filb2eKfiX4h1TiMxZqUZHVZyb9nShbQoXJ3vj/MGVF0QM8TxhXM8r2Lv9gDYU5t9nQlUMLhs0jVjai48jHABbFNyH3sEcOmJOIwJrCXw1dzG7AotwyaEVUHOmL04TffmwCFfnyrLjbFgnyOeoyIIBYjcY7QFRm/9nupXMTH5hZ2qrHfCJIp0KK4tNBdQqmnHapFl5l6Le1s4qBS5bEIzjitobLvAFm9abPlDGfxmY6mlrMK4+nytwF9Ct7wc1AE=").unwrap();
        let primes = vec![
            base64::decode("9kQWEAzsbzOcdPa+s5wFfw4XDd7bB1q9foZ31b1+TNjGNxbSBCFlDF1q98vwpV6nM8bWDh/wtbNoETSQDgpEnYOQ26LWEw6YY1+q1Q2GGEFceYUf+Myk8/vTc8TN6Zw0bKZBWy10Qo8h7xk4JpzuI7NcxvjJYTkS9aErFxi3vVH0aiZC0tmfaCqr8a2rJxyVwqreRpOjwAWrotMsf2wGsF4ofx5ScoFy5GB5fJkkdOrW1LyTvZAUCX3cstPr19+TNC5zZOk7WzZatnCkN5H5WzalWtZuu0oVL205KPOa3R8V2yv5e6fm0v5fTmqSuvjmaMJLXCN4QJkmIzojO99ckQ==").unwrap(),
            base64::decode("x8exdMjVA2CiI+Thx7loHtVcevoeE2sZ7btRVAvmBqo+lkHwxb7FHRnWvuj6eJSlD2f0T50EewIhhiW3R9BmktCk7hXjbSCnC1u9Oxc1IAUm/7azRqyfCMx43XhLxpD+xkBCpWkKDLxGczsRwTuaP3lKS3bSdBrNlGmdblubvVBIq4YZ2vXVlnYtza0cS+dgCK7BGTqUsrCUd/ZbIvwcwZkZtpkhj1KQfto9X/0OMurBzAqbkeq1cyRHXHkOfN/qbUIIRqr9Ii7Eswf9Vk8xp2O1Nt8nzcYS9PFD12M5eyaeFEkEYfpNMNGuTzp/31oqVjbpoCxS6vuWAZyADxhISQ==").unwrap(),
            base64::decode("is7d0LY4HoXszlC2NO7gejkq7XqL4p1W6hZJPYTNx+r37t1CC2n3Vvzg6kNdpRixDhIpXVTLjN9O7UO/XuqSumYKJIKoP52eb4Tg+a3hw5Iz2Zsb5lUTNSLgkQSBPAf71LHxbL82JL4g1nBUog8ae60BwnVArThKY4EwlJguGNw09BAU4lwf6csDl/nX2vfVwiAloYpeZkHL+L8m+bueGZM5KE2jEz+7ztZCI+T+E5i69rZEYDjx0lfLKlEhQlCW3HbCPELqXgNJJkRfi6MP9kXa9lSfnZmoT081RMvqonB/FUa4HOcKyCrw9XZEtnbNCIdbitfDVEX+pSSD7596wQ==").unwrap(),
            base64::decode("GPs0injugfycacaeIP5jMa/WX55VEnKLDHom4k6WlfDF4L4gIGoJdekcPEUfxOI5faKvHyFwRP1wObkPoRBDM0qZxRfBl4zEtpvjHrd5MibSyJkM8+J0BIKk/nSjbRIGeb3hV5O56PvGB3S0dKhCUnuVObiC+ne7izplsD4OTG70l1Yud33UFntyoMxrxGYLUSqhBMmZfHquJg4NOWOzKNY/K+EcHDLj1Kjvkcgv9Vf7ocsVxvpFdD9uGPceQ6kwRDdEl6mb+6FDgWuXVyqR9+904oanEIkbJ7vfkthagLbEf57dyG6nJlqh5FBZWxGIR72YGypPuAh7qnnqXXjY2Q==").unwrap(),
            base64::decode("CUWC+hRWOT421kwRllgVjy6FYv6jQUcgDNHeAiYZnf5HjS9iK2ki7v8G5dL/0f+Yf+NhE/4q8w4m8go51hACrVpP1p8GJDjiT09+RsOzITsHwl+ceEKoe56ZW6iDHBLlrNw5/MtcYhKpjNU9KJ2udm5J/c9iislcjgckrZG2IB8ADgXHMEByZ5DgaMl4AKZ1Gx8/q6KftTvmOT5rNTMLi76VN5KWQcDWK/DqXiOiZHM7Nr4dX4me3XeRgABJyNR8Fqxj3N1+HrYLe/zs7LOaK0++F9Ul3tLelhrhsvLxei3oCZkF9A/foD3on3luYA+1cRcxWpSY3h2J4/22+yo4+Q==").unwrap(),
        ];

        RSAPrivateKey::from_components(
            BigUint::from_bytes_be(&n),
            BigUint::from_bytes_be(&e),
            BigUint::from_bytes_be(&d),
            primes.iter().map(|p| BigUint::from_bytes_be(p)).collect(),
        );
    }

    fn get_private_key() -> RSAPrivateKey {
        // -----BEGIN RSA PRIVATE KEY-----
        // MIIEpAIBAAKCAQEA05e4TZikwmE47RtpWoEG6tkdVTvwYEG2LT/cUKBB4iK49FKW
        // icG4LF5xVU9d1p+i9LYVjPDb61eBGg/DJ+HyjnT+dNO8Fmweq9wbi1e5NMqL5bAL
        // TymXW8yZrK9BW1m7KKZ4K7QaLDwpdrPBjbre9i8AxrsiZkAJUJbAzGDSL+fvmH11
        // xqgbENlr8pICivEQ3HzBu8Q9Iq2rN5oM1dgHjMeA/1zWIJ3qNMkiz3hPdxfkKNdb
        // WuyP8w5fAUFRB2bi4KuNRzyE6HELK5gifD2wlTN600UvGeK5v7zN2BSKv2d2+lUn
        // debnWVbkUimuWpxGlJurHmIvDkj1ZSSoTtNIOwIDAQABAoIBAQDE5wxokWLJTGYI
        // KBkbUrTYOSEV30hqmtvoMeRY1zlYMg3Bt1VFbpNwHpcC12+wuS+Q4B0f4kgVMoH+
        // eaqXY6kvrmnY1+zRRN4p+hNb0U+Vc+NJ5FAx47dpgvWDADgmxVLomjl8Gga9IWNI
        // hjDZLowrtkPXq+9wDaldaFyUFImkb1S1MW9itdLDp/G70TTLNzU6RGg/3J2V02RY
        // 3iL2xEBX/nSgpDbEMI9z9NpC81xHrBanE41IOvyR5B3DoRJzguDA9RGbAiG0/GOd
        // a5w4F3pt6bUm69iMONeYLAf5ig79h31Qiq4nW5RpFcAuLhEG0XXXTsZ3f16A0SwF
        // PZx74eNBAoGBAPgnu/OkGHfHzFmuv0LtSynDLe/LjtloY9WwkKBaiTDdYkohydz5
        // g4Vo/foN9luEYqXyrJE9bFb5dVMr2OePsHvUBcqZpIS89Z8Bm73cs5M/K85wYwC0
        // 97EQEgxd+QGBWQZ8NdowYaVshjWlK1QnOzEnG0MR8Hld9gIeY1XhpC5hAoGBANpI
        // F84Aid028q3mo/9BDHPsNL8bT2vaOEMb/t4RzvH39u+nDl+AY6Ox9uFylv+xX+76
        // CRKgMluNH9ZaVZ5xe1uWHsNFBy4OxSA9A0QdKa9NZAVKBFB0EM8dp457YRnZCexm
        // 5q1iW/mVsnmks8W+fYlc18W5xMSX/ecwkW/NtOQbAoGAHabpz4AhKFbodSLrWbzv
        // CUt4NroVFKdjnoodjfujfwJFF2SYMV5jN9LG3lVCxca43ulzc1tqka33Nfv8TBcg
        // WHuKQZ5ASVgm5VwU1wgDMSoQOve07MWy/yZTccTc1zA0ihDXgn3bfR/NnaVh2wlh
        // CkuI92eyW1494hztc7qlmqECgYEA1zenyOQ9ChDIW/ABGIahaZamNxsNRrDFMl3j
        // AD+cxHSRU59qC32CQH8ShRy/huHzTaPX2DZ9EEln76fnrS4Ey7uLH0rrFl1XvT6K
        // /timJgLvMEvXTx/xBtUdRN2fUqXtI9odbSyCtOYFL+zVl44HJq2UzY4pVRDrNcxs
        // SUkQJqsCgYBSaNfPBzR5rrstLtTdZrjImRW1LRQeDEky9WsMDtCTYUGJTsTSfVO8
        // hkU82MpbRVBFIYx+GWIJwcZRcC7OCQoV48vMJllxMAAjqG/p00rVJ+nvA7et/nNu
        // BoB0er/UmDm4Ly/97EO9A0PKMOE5YbMq9s3t3RlWcsdrU7dvw+p2+A==
        // -----END RSA PRIVATE KEY-----

        RSAPrivateKey::from_components(
            BigUint::parse_bytes(b"00d397b84d98a4c26138ed1b695a8106ead91d553bf06041b62d3fdc50a041e222b8f4529689c1b82c5e71554f5dd69fa2f4b6158cf0dbeb57811a0fc327e1f28e74fe74d3bc166c1eabdc1b8b57b934ca8be5b00b4f29975bcc99acaf415b59bb28a6782bb41a2c3c2976b3c18dbadef62f00c6bb226640095096c0cc60d22fe7ef987d75c6a81b10d96bf292028af110dc7cc1bbc43d22adab379a0cd5d8078cc780ff5cd6209dea34c922cf784f7717e428d75b5aec8ff30e5f0141510766e2e0ab8d473c84e8710b2b98227c3db095337ad3452f19e2b9bfbccdd8148abf6776fa552775e6e75956e45229ae5a9c46949bab1e622f0e48f56524a84ed3483b", 16).unwrap(),
            BigUint::from_u64(65537).unwrap(),
            BigUint::parse_bytes(b"00c4e70c689162c94c660828191b52b4d8392115df486a9adbe831e458d73958320dc1b755456e93701e9702d76fb0b92f90e01d1fe248153281fe79aa9763a92fae69d8d7ecd144de29fa135bd14f9573e349e45031e3b76982f583003826c552e89a397c1a06bd2163488630d92e8c2bb643d7abef700da95d685c941489a46f54b5316f62b5d2c3a7f1bbd134cb37353a44683fdc9d95d36458de22f6c44057fe74a0a436c4308f73f4da42f35c47ac16a7138d483afc91e41dc3a1127382e0c0f5119b0221b4fc639d6b9c38177a6de9b526ebd88c38d7982c07f98a0efd877d508aae275b946915c02e2e1106d175d74ec6777f5e80d12c053d9c7be1e341", 16).unwrap(),
            vec![
                BigUint::parse_bytes(b"00f827bbf3a41877c7cc59aebf42ed4b29c32defcb8ed96863d5b090a05a8930dd624a21c9dcf9838568fdfa0df65b8462a5f2ac913d6c56f975532bd8e78fb07bd405ca99a484bcf59f019bbddcb3933f2bce706300b4f7b110120c5df9018159067c35da3061a56c8635a52b54273b31271b4311f0795df6021e6355e1a42e61",16).unwrap(),
                BigUint::parse_bytes(b"00da4817ce0089dd36f2ade6a3ff410c73ec34bf1b4f6bda38431bfede11cef1f7f6efa70e5f8063a3b1f6e17296ffb15feefa0912a0325b8d1fd65a559e717b5b961ec345072e0ec5203d03441d29af4d64054a04507410cf1da78e7b6119d909ec66e6ad625bf995b279a4b3c5be7d895cd7c5b9c4c497fde730916fcdb4e41b", 16).unwrap()
            ],
        )
    }

    #[test]
    fn test_encrypt_decrypt_oaep() {
        let priv_key = get_private_key();
        do_test_encrypt_decrypt_oaep::<Sha1>(&priv_key);
        do_test_encrypt_decrypt_oaep::<Sha224>(&priv_key);
        do_test_encrypt_decrypt_oaep::<Sha256>(&priv_key);
        do_test_encrypt_decrypt_oaep::<Sha384>(&priv_key);
        do_test_encrypt_decrypt_oaep::<Sha512>(&priv_key);
        do_test_encrypt_decrypt_oaep::<Sha3_256>(&priv_key);
        do_test_encrypt_decrypt_oaep::<Sha3_384>(&priv_key);
        do_test_encrypt_decrypt_oaep::<Sha3_512>(&priv_key);
    }

    fn do_test_encrypt_decrypt_oaep<D: 'static + Digest + DynDigest>(prk: &RSAPrivateKey) {
        let mut rng = thread_rng();

        let k = prk.size();

        for i in 1..8 {
            let mut input: Vec<u8> = (0..i * 8).map(|_| rng.gen()).collect();
            if input.len() > k - 11 {
                input = input[0..k - 11].to_vec();
            }
            let has_label: bool = rng.gen();
            let label: Option<String> = if has_label {
                Some(rng.sample_iter(&Alphanumeric).take(30).collect())
            } else {
                None
            };

            let pub_key: RSAPublicKey = prk.into();

            let ciphertext = if let Some(ref label) = label {
                let padding = PaddingScheme::new_oaep_with_label::<D, _>(label);
                pub_key.encrypt(&mut rng, padding, &input).unwrap()
            } else {
                let padding = PaddingScheme::new_oaep::<D>();
                pub_key.encrypt(&mut rng, padding, &input).unwrap()
            };

            assert_ne!(input, ciphertext);
            let blind: bool = rng.gen();

            let padding = if let Some(ref label) = label {
                PaddingScheme::new_oaep_with_label::<D, _>(label)
            } else {
                PaddingScheme::new_oaep::<D>()
            };

            let plaintext = if blind {
                prk.decrypt(padding, &ciphertext).unwrap()
            } else {
                prk.decrypt_blinded(&mut rng, padding, &ciphertext).unwrap()
            };

            assert_eq!(input, plaintext);
        }
    }

    #[test]
    fn test_decrypt_oaep_invalid_hash() {
        let mut rng = thread_rng();
        let priv_key = get_private_key();
        let pub_key: RSAPublicKey = (&priv_key).into();
        let ciphertext = pub_key
            .encrypt(
                &mut rng,
                PaddingScheme::new_oaep::<Sha1>(),
                "a_plain_text".as_bytes(),
            )
            .unwrap();
        assert!(
            priv_key
                .decrypt_blinded(
                    &mut rng,
                    PaddingScheme::new_oaep_with_label::<Sha1, _>("label"),
                    &ciphertext,
                )
                .is_err(),
            "decrypt should have failed on hash verification"
        );
    }
}
