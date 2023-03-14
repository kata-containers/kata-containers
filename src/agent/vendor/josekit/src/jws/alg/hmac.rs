use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;
use openssl::pkey::{PKey, Private};
use openssl::sign::Signer;

use crate::jwk::Jwk;
use crate::jws::{JwsAlgorithm, JwsSigner, JwsVerifier};
use crate::util::HashAlgorithm;
use crate::{JoseError, Value};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum HmacJwsAlgorithm {
    /// HMAC using SHA-256
    Hs256,
    /// HMAC using SHA-384
    Hs384,
    /// HMAC using SHA-512
    Hs512,
}

impl HmacJwsAlgorithm {
    /// Make a JWK encoded oct private key.
    ///
    /// # Arguments
    /// * `secret` - A secret key
    pub fn to_jwk(&self, secret: &[u8]) -> Jwk {
        let k = base64::encode_config(secret, base64::URL_SAFE_NO_PAD);

        let mut jwk = Jwk::new("oct");
        jwk.set_key_use("sig");
        jwk.set_key_operations(vec!["sign", "verify"]);
        jwk.set_algorithm(self.name());
        jwk.set_parameter("k", Some(Value::String(k))).unwrap();

        jwk
    }

    /// Return a signer from a secret key.
    ///
    /// # Arguments
    /// * `data` - A secret key.
    pub fn signer_from_bytes(&self, input: impl AsRef<[u8]>) -> Result<HmacJwsSigner, JoseError> {
        (|| -> anyhow::Result<HmacJwsSigner> {
            let input = input.as_ref();

            let min_key_len = self.hash_algorithm().output_len();
            if input.len() < min_key_len {
                bail!(
                    "Secret key size must be larger than or equal to {}: {}",
                    min_key_len,
                    input.len()
                );
            }

            let private_key = PKey::hmac(input)?;

            Ok(HmacJwsSigner {
                algorithm: self.clone(),
                private_key,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Return a signer from a secret key that is formatted by a JWK of oct type.
    ///
    /// # Arguments
    /// * `jwk` - A secret key that is formatted by a JWK of oct type.
    pub fn signer_from_jwk(&self, jwk: &Jwk) -> Result<HmacJwsSigner, JoseError> {
        (|| -> anyhow::Result<HmacJwsSigner> {
            match jwk.key_type() {
                val if val == "oct" => {}
                val => bail!("A parameter kty must be oct: {}", val),
            }
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
                None => {}
                Some(val) => bail!("A parameter alg must be {} but {}", self.name(), val),
            }
            let k = match jwk.parameter("k") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(val) => bail!("A parameter k must be string type but {:?}", val),
                None => bail!("A parameter k is required."),
            };

            let min_key_len = self.hash_algorithm().output_len();
            if k.len() < min_key_len {
                bail!(
                    "Secret key size must be larger than or equal to {}: {}",
                    min_key_len,
                    k.len()
                );
            }

            let private_key = PKey::hmac(&k)?;
            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(HmacJwsSigner {
                algorithm: self.clone(),
                private_key,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Return a verifier from a secret key.
    ///
    /// # Arguments
    /// * `input` - A secret key.
    pub fn verifier_from_bytes(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<HmacJwsVerifier, JoseError> {
        (|| -> anyhow::Result<HmacJwsVerifier> {
            let input = input.as_ref();

            let min_key_len = self.hash_algorithm().output_len();
            if input.len() < min_key_len {
                bail!(
                    "Secret key size must be larger than or equal to {}: {}",
                    min_key_len,
                    input.len()
                );
            }

            let private_key = PKey::hmac(input)?;

            Ok(HmacJwsVerifier {
                algorithm: self.clone(),
                private_key,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Return a verifier from a secret key that is formatted by a JWK of oct type.
    ///
    /// # Arguments
    /// * `jwk` - A secret key that is formatted by a JWK of oct type.
    pub fn verifier_from_jwk(&self, jwk: &Jwk) -> Result<HmacJwsVerifier, JoseError> {
        (|| -> anyhow::Result<HmacJwsVerifier> {
            match jwk.key_type() {
                val if val == "oct" => {}
                val => bail!("A parameter kty must be oct: {}", val),
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

            let k = match jwk.parameter("k") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(val) => bail!("A parameter k must be string type but {:?}", val),
                None => bail!("A parameter k is required."),
            };

            let min_key_len = self.hash_algorithm().output_len();
            if k.len() < min_key_len {
                bail!(
                    "Secret key size must be larger than or equal to {}: {}",
                    min_key_len,
                    k.len()
                );
            }

            let private_key = PKey::hmac(&k)?;
            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(HmacJwsVerifier {
                algorithm: self.clone(),
                private_key,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn hash_algorithm(&self) -> HashAlgorithm {
        match self {
            Self::Hs256 => HashAlgorithm::Sha256,
            Self::Hs384 => HashAlgorithm::Sha384,
            Self::Hs512 => HashAlgorithm::Sha512,
        }
    }
}

impl JwsAlgorithm for HmacJwsAlgorithm {
    fn name(&self) -> &str {
        match self {
            Self::Hs256 => "HS256",
            Self::Hs384 => "HS384",
            Self::Hs512 => "HS512",
        }
    }

    fn box_clone(&self) -> Box<dyn JwsAlgorithm> {
        Box::new(self.clone())
    }
}

impl Display for HmacJwsAlgorithm {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

impl Deref for HmacJwsAlgorithm {
    type Target = dyn JwsAlgorithm;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct HmacJwsSigner {
    algorithm: HmacJwsAlgorithm,
    private_key: PKey<Private>,
    key_id: Option<String>,
}

impl HmacJwsSigner {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JwsSigner for HmacJwsSigner {
    fn algorithm(&self) -> &dyn JwsAlgorithm {
        &self.algorithm
    }

    fn signature_len(&self) -> usize {
        self.algorithm.hash_algorithm().output_len()
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
            let signature = signer.sign_to_vec()?;
            Ok(signature)
        })()
        .map_err(|err| JoseError::InvalidSignature(err))
    }

    fn box_clone(&self) -> Box<dyn JwsSigner> {
        Box::new(self.clone())
    }
}

impl Deref for HmacJwsSigner {
    type Target = dyn JwsSigner;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct HmacJwsVerifier {
    algorithm: HmacJwsAlgorithm,
    private_key: PKey<Private>,
    key_id: Option<String>,
}

impl HmacJwsVerifier {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JwsVerifier for HmacJwsVerifier {
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
            let md = self.algorithm.hash_algorithm().message_digest();

            let mut signer = Signer::new(md, &self.private_key)?;
            signer.update(message)?;
            let new_signature = signer.sign_to_vec()?;
            if new_signature.as_slice() != signature {
                bail!("Failed to verify.");
            }
            Ok(())
        })()
        .map_err(|err| JoseError::InvalidSignature(err))
    }

    fn box_clone(&self) -> Box<dyn JwsVerifier> {
        Box::new(self.clone())
    }
}

impl Deref for HmacJwsVerifier {
    type Target = dyn JwsVerifier;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::util;
    use anyhow::Result;
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;

    #[test]
    fn sign_and_verify_hmac_generated_jwk() -> Result<()> {
        let private_key = util::random_bytes(64);
        let input = b"12345abcde";

        for alg in &[
            HmacJwsAlgorithm::Hs256,
            HmacJwsAlgorithm::Hs384,
            HmacJwsAlgorithm::Hs512,
        ] {
            let private_key = alg.to_jwk(&private_key);

            let signer = alg.signer_from_jwk(&private_key)?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_jwk(&private_key)?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_hmac_jwk() -> Result<()> {
        let input = b"abcde12345";

        for alg in &[
            HmacJwsAlgorithm::Hs256,
            HmacJwsAlgorithm::Hs384,
            HmacJwsAlgorithm::Hs512,
        ] {
            let private_key = load_file("jwk/oct_512bit_private.jwk")?;
            let private_key = Jwk::from_bytes(&private_key)?;

            let signer = alg.signer_from_jwk(&private_key)?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_jwk(&private_key)?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    #[test]
    fn sign_and_verify_hmac_bytes() -> Result<()> {
        let private_key = util::random_bytes(64);
        let input = b"abcde12345";

        for alg in &[
            HmacJwsAlgorithm::Hs256,
            HmacJwsAlgorithm::Hs384,
            HmacJwsAlgorithm::Hs512,
        ] {
            let signer = alg.signer_from_bytes(&private_key)?;
            let signature = signer.sign(input)?;

            let verifier = alg.verifier_from_bytes(&private_key)?;
            verifier.verify(input, &signature)?;
        }

        Ok(())
    }

    fn load_file(path: &str) -> Result<Vec<u8>> {
        let mut pb = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pb.push("data");
        pb.push(path);

        let mut file = File::open(&pb)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    }
}
