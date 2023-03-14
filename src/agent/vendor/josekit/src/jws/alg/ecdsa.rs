use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;
use openssl::pkey::{PKey, Private, Public};
use openssl::sign::{Signer, Verifier};

use crate::jwk::{
    alg::ec::{EcCurve, EcKeyPair},
    Jwk,
};
use crate::jws::{JwsAlgorithm, JwsSigner, JwsVerifier};
use crate::util::der::{DerBuilder, DerReader, DerType};
use crate::util::{self, HashAlgorithm};
use crate::{JoseError, Value};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum EcdsaJwsAlgorithm {
    /// ECDSA using P-256 and SHA-256
    Es256,
    /// ECDSA using P-384 and SHA-384
    Es384,
    /// ECDSA using P-521 and SHA-512
    Es512,
    /// ECDSA using secp256k1 curve and SHA-256
    Es256k,
}

impl EcdsaJwsAlgorithm {
    /// Generate ECDSA key pair.
    pub fn generate_key_pair(&self) -> Result<EcKeyPair, JoseError> {
        let mut key_pair = EcKeyPair::generate(self.curve())?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Create a EcDSA key pair from a private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    ///
    /// # Arguments
    /// * `input` - A private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    pub fn key_pair_from_der(&self, input: impl AsRef<[u8]>) -> Result<EcKeyPair, JoseError> {
        let mut key_pair = EcKeyPair::from_der(input, Some(self.curve()))?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Create a EcDSA key pair from a private key of common or traditinal PEM format.
    ///
    /// Common PEM format is a DER and base64 encoded PKCS#8 PrivateKeyInfo
    /// that surrounded by "-----BEGIN/END PRIVATE KEY----".
    ///
    /// Traditional PEM format is a DER and base64 encoded ECPrivateKey
    /// that surrounded by "-----BEGIN/END EC PRIVATE KEY----".
    ///
    /// # Arguments
    /// * `input` - A private key of common or traditinal PEM format.
    pub fn key_pair_from_pem(&self, input: impl AsRef<[u8]>) -> Result<EcKeyPair, JoseError> {
        let mut key_pair = EcKeyPair::from_pem(input.as_ref(), Some(self.curve()))?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Return a signer from a private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    ///
    /// # Arguments
    /// * `input` - A private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    pub fn signer_from_der(&self, input: impl AsRef<[u8]>) -> Result<EcdsaJwsSigner, JoseError> {
        let key_pair = self.key_pair_from_der(input.as_ref())?;
        Ok(EcdsaJwsSigner {
            algorithm: self.clone(),
            private_key: key_pair.into_private_key(),
            key_id: None,
        })
    }

    /// Return a signer from a private key of common or traditinal PEM format.
    ///
    /// Common PEM format is a DER and base64 encoded PKCS#8 PrivateKeyInfo
    /// that surrounded by "-----BEGIN/END PRIVATE KEY----".
    ///
    /// Traditional PEM format is a DER and base64 encoded ECPrivateKey
    /// that surrounded by "-----BEGIN/END EC PRIVATE KEY----".
    ///
    /// # Arguments
    /// * `input` - A private key of common or traditinal PEM format.
    pub fn signer_from_pem(&self, input: impl AsRef<[u8]>) -> Result<EcdsaJwsSigner, JoseError> {
        let key_pair = self.key_pair_from_pem(input.as_ref())?;
        Ok(EcdsaJwsSigner {
            algorithm: self.clone(),
            private_key: key_pair.into_private_key(),
            key_id: None,
        })
    }

    /// Return a signer from a private key that is formatted by a JWK of EC type.
    ///
    /// # Arguments
    /// * `jwk` - A private key that is formatted by a JWK of EC type.
    pub fn signer_from_jwk(&self, jwk: &Jwk) -> Result<EcdsaJwsSigner, JoseError> {
        (|| -> anyhow::Result<EcdsaJwsSigner> {
            match jwk.key_use() {
                Some(val) if val == "sig" => {}
                None => {}
                Some(val) => bail!("A parameter use must be sig: {}", val),
            }
            if !jwk.is_for_key_operation("sign") {
                bail!("A parameter key_ops must contains sign.");
            }
            match jwk.algorithm() {
                Some(val) if val == self.name() => {}
                Some(val) => bail!("A parameter alg must be {} but {}", self.name(), val),
                None => {}
            }
            match jwk.curve() {
                Some(val) if val == self.curve().name() => {}
                Some(val) => bail!("A parameter crv must be {} but {}", self.name(), val),
                None => bail!("A parameter crv is required."),
            }

            let key_pair = EcKeyPair::from_jwk(jwk)?;
            let private_key = key_pair.into_private_key();
            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(EcdsaJwsSigner {
                algorithm: self.clone(),
                private_key,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Return a verifier from a public key that is a DER encoded SubjectPublicKeyInfo.
    ///
    /// # Arguments
    /// * `input` - A public key that is a DER encoded SubjectPublicKeyInfo.
    pub fn verifier_from_der(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<EcdsaJwsVerifier, JoseError> {
        (|| -> anyhow::Result<EcdsaJwsVerifier> {
            let spki_der = match EcKeyPair::detect_pkcs8(input.as_ref(), true) {
                Some(curve) if curve == self.curve() => input.as_ref(),
                Some(curve) => bail!("The curve is mismatched: {}", curve),
                None => {
                    bail!("The ECDSA public key must be wrapped by SubjectPublicKeyInfo format.")
                }
            };

            let public_key = PKey::public_key_from_der(spki_der)?;

            Ok(EcdsaJwsVerifier {
                algorithm: self.clone(),
                public_key,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Return a verifier from a key of common PEM format.
    ///
    /// Common PEM format is a DER and base64 encoded SubjectPublicKeyInfo
    /// that surrounded by "-----BEGIN/END PUBLIC KEY----".
    ///
    /// # Arguments
    /// * `input` - A public key of common or traditional PEM format.
    pub fn verifier_from_pem(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<EcdsaJwsVerifier, JoseError> {
        (|| -> anyhow::Result<EcdsaJwsVerifier> {
            let (alg, data) = util::parse_pem(input.as_ref())?;

            let spki = match alg.as_str() {
                "PUBLIC KEY" => {
                    if let None = EcKeyPair::detect_pkcs8(&data, true) {
                        bail!("PEM contents is expected SubjectPublicKeyInfo wrapped key.");
                    }
                    &data
                }
                alg => bail!("Inappropriate algorithm: {}", alg),
            };

            let public_key = PKey::public_key_from_der(spki)?;

            Ok(EcdsaJwsVerifier {
                algorithm: self.clone(),
                public_key,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Return a verifier from a public key that is formatted by a JWK of EC type.
    ///
    /// # Arguments
    /// * `jwk` - A public key that is formatted by a JWK of EC type.
    pub fn verifier_from_jwk(&self, jwk: &Jwk) -> Result<EcdsaJwsVerifier, JoseError> {
        (|| -> anyhow::Result<EcdsaJwsVerifier> {
            let curve = self.curve();

            match jwk.key_type() {
                val if val == "EC" => {}
                val => bail!("A parameter kty must be EC: {}", val),
            }
            match jwk.key_use() {
                Some(val) if val == "sig" => {}
                None => {}
                Some(val) => bail!("A parameter use must be sig: {}", val),
            }
            if !jwk.is_for_key_operation("verify") {
                bail!("A parameter key_ops must contains verify.");
            }
            match jwk.algorithm() {
                Some(val) if val == self.name() => {}
                None => {}
                Some(val) => bail!("A parameter alg must be {} but {}", self.name(), val),
            }
            match jwk.parameter("crv") {
                Some(Value::String(val)) if val == curve.name() => {}
                Some(Value::String(val)) => {
                    bail!("A parameter crv must be {} but {}", curve.name(), val)
                }
                Some(_) => bail!("A parameter crv must be a string."),
                None => bail!("A parameter crv is required."),
            }
            let x = match jwk.parameter("x") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(_) => bail!("A parameter x must be a string."),
                None => bail!("A parameter x is required."),
            };
            let y = match jwk.parameter("y") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(_) => bail!("A parameter y must be a string."),
                None => bail!("A parameter y is required."),
            };

            let mut vec = Vec::with_capacity(1 + x.len() + y.len());
            vec.push(0x04);
            vec.extend_from_slice(&x);
            vec.extend_from_slice(&y);

            let pkcs8 = EcKeyPair::to_pkcs8(&vec, true, self.curve());
            let public_key = PKey::public_key_from_der(&pkcs8)?;
            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(EcdsaJwsVerifier {
                algorithm: self.clone(),
                public_key,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn curve(&self) -> EcCurve {
        match self {
            Self::Es256 => EcCurve::P256,
            Self::Es384 => EcCurve::P384,
            Self::Es512 => EcCurve::P521,
            Self::Es256k => EcCurve::Secp256k1,
        }
    }

    fn signature_len(&self) -> usize {
        match self {
            Self::Es256 | Self::Es256k => 64,
            Self::Es384 => 96,
            Self::Es512 => 132,
        }
    }

    fn hash_algorithm(&self) -> HashAlgorithm {
        match self {
            Self::Es256 => HashAlgorithm::Sha256,
            Self::Es384 => HashAlgorithm::Sha384,
            Self::Es512 => HashAlgorithm::Sha512,
            Self::Es256k => HashAlgorithm::Sha256,
        }
    }
}

impl JwsAlgorithm for EcdsaJwsAlgorithm {
    fn name(&self) -> &str {
        match self {
            Self::Es256 => "ES256",
            Self::Es384 => "ES384",
            Self::Es512 => "ES512",
            Self::Es256k => "ES256K",
        }
    }

    fn box_clone(&self) -> Box<dyn JwsAlgorithm> {
        Box::new(self.clone())
    }
}

impl Display for EcdsaJwsAlgorithm {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

impl Deref for EcdsaJwsAlgorithm {
    type Target = dyn JwsAlgorithm;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct EcdsaJwsSigner {
    algorithm: EcdsaJwsAlgorithm,
    private_key: PKey<Private>,
    key_id: Option<String>,
}

impl EcdsaJwsSigner {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JwsSigner for EcdsaJwsSigner {
    fn algorithm(&self) -> &dyn JwsAlgorithm {
        &self.algorithm
    }

    fn signature_len(&self) -> usize {
        self.algorithm.signature_len()
    }

    fn key_id(&self) -> Option<&str> {
        match &self.key_id {
            Some(val) => Some(val.as_ref()),
            None => None,
        }
    }

    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, JoseError> {
        (|| -> anyhow::Result<Vec<u8>> {
            let md = self.algorithm.hash_algorithm().message_digest();

            let mut signer = Signer::new(md, &self.private_key)?;
            signer.update(message)?;
            let der_signature = signer.sign_to_vec()?;

            let signature_len = self.signature_len();
            let sep = signature_len / 2;

            let mut signature = Vec::with_capacity(signature_len);
            let mut reader = DerReader::from_bytes(&der_signature);
            match reader.next()? {
                Some(DerType::Sequence) => {}
                _ => unreachable!("A generated signature is invalid."),
            }
            match reader.next()? {
                Some(DerType::Integer) => {
                    signature.extend_from_slice(&reader.to_be_bytes(false, sep));
                }
                _ => unreachable!("A generated signature is invalid."),
            }
            match reader.next()? {
                Some(DerType::Integer) => {
                    signature.extend_from_slice(&reader.to_be_bytes(false, sep));
                }
                _ => unreachable!("A generated signature is invalid."),
            }

            Ok(signature)
        })()
        .map_err(|err| JoseError::InvalidSignature(err))
    }

    fn box_clone(&self) -> Box<dyn JwsSigner> {
        Box::new(self.clone())
    }
}

impl Deref for EcdsaJwsSigner {
    type Target = dyn JwsSigner;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct EcdsaJwsVerifier {
    algorithm: EcdsaJwsAlgorithm,
    public_key: PKey<Public>,
    key_id: Option<String>,
}

impl EcdsaJwsVerifier {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JwsVerifier for EcdsaJwsVerifier {
    fn algorithm(&self) -> &dyn JwsAlgorithm {
        &self.algorithm
    }

    fn key_id(&self) -> Option<&str> {
        match &self.key_id {
            Some(val) => Some(val.as_ref()),
            None => None,
        }
    }

    fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), JoseError> {
        (|| -> anyhow::Result<()> {
            let signature_len = self.algorithm.signature_len();
            if signature.len() != signature_len {
                bail!(
                    "A signature size must be {}: {}",
                    signature_len,
                    signature.len()
                );
            }

            let mut der_builder = DerBuilder::new();
            der_builder.begin(DerType::Sequence);
            {
                let sep = signature_len / 2;

                let zeros = signature[..sep].iter().take_while(|b| **b == 0).count();
                der_builder.append_integer_from_be_slice(&signature[zeros..sep], true);
                let zeros = signature[sep..].iter().take_while(|b| **b == 0).count();
                der_builder.append_integer_from_be_slice(&signature[(sep + zeros)..], true);
            }
            der_builder.end();
            let der_signature = der_builder.build();

            let md = self.algorithm.hash_algorithm().message_digest();

            let mut verifier = Verifier::new(md, &self.public_key)?;
            verifier.update(message)?;
            if !verifier.verify(&der_signature)? {
                bail!("The signature does not match.");
            }
            Ok(())
        })()
        .map_err(|err| JoseError::InvalidSignature(err))
    }

    fn box_clone(&self) -> Box<dyn JwsVerifier> {
        Box::new(self.clone())
    }
}

impl Deref for EcdsaJwsVerifier {
    type Target = dyn JwsVerifier;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn sign_and_verify_ecdsa_generated_der() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let key_pair = alg.generate_key_pair()?;

            let signer = alg.signer_from_der(&key_pair.to_der_private_key())?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_der(&key_pair.to_der_public_key())?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_generated_raw() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let key_pair = alg.generate_key_pair()?;

            let signer = alg.signer_from_der(&key_pair.to_raw_private_key())?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_der(&key_pair.to_der_public_key())?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_generated_pem() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let key_pair = alg.generate_key_pair()?;

            let signer = alg.signer_from_pem(&key_pair.to_pem_private_key())?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_pem(&key_pair.to_pem_public_key())?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_generated_traditional_pem() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let key_pair = alg.generate_key_pair()?;

            let signer = alg.signer_from_pem(&key_pair.to_traditional_pem_private_key())?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_pem(&key_pair.to_pem_public_key())?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_generated_jwk() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let key_pair = alg.generate_key_pair()?;

            let signer = alg.signer_from_jwk(&key_pair.to_jwk_private_key())?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_jwk(&key_pair.to_jwk_public_key())?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_jwt() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let private_key = load_file(match alg {
                EcdsaJwsAlgorithm::Es256 => "jwk/EC_P-256_private.jwk",
                EcdsaJwsAlgorithm::Es384 => "jwk/EC_P-384_private.jwk",
                EcdsaJwsAlgorithm::Es512 => "jwk/EC_P-521_private.jwk",
                EcdsaJwsAlgorithm::Es256k => "jwk/EC_secp256k1_private.jwk",
            })?;
            let public_key = load_file(match alg {
                EcdsaJwsAlgorithm::Es256 => "jwk/EC_P-256_public.jwk",
                EcdsaJwsAlgorithm::Es384 => "jwk/EC_P-384_public.jwk",
                EcdsaJwsAlgorithm::Es512 => "jwk/EC_P-521_public.jwk",
                EcdsaJwsAlgorithm::Es256k => "jwk/EC_secp256k1_public.jwk",
            })?;

            let signer = alg.signer_from_jwk(&Jwk::from_bytes(&private_key)?)?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_jwk(&Jwk::from_bytes(&public_key)?)?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_pkcs8_pem() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            println!("{}", alg);

            let private_key = load_file(match alg {
                EcdsaJwsAlgorithm::Es256 => "pem/EC_P-256_private.pem",
                EcdsaJwsAlgorithm::Es384 => "pem/EC_P-384_private.pem",
                EcdsaJwsAlgorithm::Es512 => "pem/EC_P-521_private.pem",
                EcdsaJwsAlgorithm::Es256k => "pem/EC_secp256k1_private.pem",
            })?;
            let public_key = load_file(match alg {
                EcdsaJwsAlgorithm::Es256 => "pem/EC_P-256_public.pem",
                EcdsaJwsAlgorithm::Es384 => "pem/EC_P-384_public.pem",
                EcdsaJwsAlgorithm::Es512 => "pem/EC_P-521_public.pem",
                EcdsaJwsAlgorithm::Es256k => "pem/EC_secp256k1_public.pem",
            })?;

            let signer = alg.signer_from_pem(&private_key)?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_pem(&public_key)?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_pkcs8_der() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let private_key = load_file(match alg {
                EcdsaJwsAlgorithm::Es256 => "der/EC_P-256_pkcs8_private.der",
                EcdsaJwsAlgorithm::Es384 => "der/EC_P-384_pkcs8_private.der",
                EcdsaJwsAlgorithm::Es512 => "der/EC_P-521_pkcs8_private.der",
                EcdsaJwsAlgorithm::Es256k => "der/EC_secp256k1_pkcs8_private.der",
            })?;
            let public_key = load_file(match alg {
                EcdsaJwsAlgorithm::Es256 => "der/EC_P-256_spki_public.der",
                EcdsaJwsAlgorithm::Es384 => "der/EC_P-384_spki_public.der",
                EcdsaJwsAlgorithm::Es512 => "der/EC_P-521_spki_public.der",
                EcdsaJwsAlgorithm::Es256k => "der/EC_secp256k1_spki_public.der",
            })?;

            let signer = alg.signer_from_der(&private_key)?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_der(&public_key)?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_ecdsa_mismatch() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            EcdsaJwsAlgorithm::Es256,
            EcdsaJwsAlgorithm::Es384,
            EcdsaJwsAlgorithm::Es512,
            EcdsaJwsAlgorithm::Es256k,
        ] {
            let signer_key_pair = alg.generate_key_pair()?;
            let verifier_key_pair = alg.generate_key_pair()?;

            let signer = alg.signer_from_der(&signer_key_pair.to_der_private_key())?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_der(&verifier_key_pair.to_der_public_key())?;
            verifier
                .verify(input, &signature)
                .expect_err("Unmatched signature did not fail");
        }

        Ok(())
    }

    fn load_file(path: &str) -> Result<Vec<u8>> {
        let mut pb = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pb.push("data");
        pb.push(path);

        let data = fs::read(&pb)?;
        Ok(data)
    }
}
