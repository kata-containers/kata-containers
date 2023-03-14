use anyhow::bail;

use crate::jwe::{JweContext, JweDecrypter, JweEncrypter, JweHeader};
use crate::jwk::{Jwk, JwkSet};
use crate::jws::{JwsContext, JwsHeader, JwsSigner, JwsVerifier};
use crate::jwt::{self, JwtPayload};
use crate::{JoseError, JoseHeader, Map, Value};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct JwtContext {
    jws_context: JwsContext,
    jwe_context: JweContext,
}

impl JwtContext {
    pub fn new() -> Self {
        Self {
            jws_context: JwsContext::new(),
            jwe_context: JweContext::new(),
        }
    }

    /// Test a critical header claim name is acceptable.
    ///
    /// # Arguments
    ///
    /// * `name` - a critical header claim name
    pub fn is_acceptable_critical(&self, name: &str) -> bool {
        self.jws_context.is_acceptable_critical(name)
    }

    /// Add a acceptable critical header claim name
    ///
    /// # Arguments
    ///
    /// * `name` - a acceptable critical header claim name
    pub fn add_acceptable_critical(&mut self, name: &str) {
        self.jws_context.add_acceptable_critical(name);
        self.jwe_context.add_acceptable_critical(name);
    }

    /// Remove a acceptable critical header claim name
    ///
    /// # Arguments
    ///
    /// * `name` - a acceptable critical header claim name
    pub fn remove_acceptable_critical(&mut self, name: &str) {
        self.jws_context.remove_acceptable_critical(name);
        self.jwe_context.remove_acceptable_critical(name);
    }

    /// Return the string repsentation of the JWT with a "none" algorithm.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWT heaser claims.
    pub fn encode_unsecured(
        &self,
        payload: &JwtPayload,
        header: &JwsHeader,
    ) -> Result<String, JoseError> {
        self.encode_with_signer(payload, header, &jwt::None.signer())
    }

    /// Return the string repsentation of the JWT with the siginig algorithm.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWS heaser claims.
    /// * `signer` - a signer object.
    pub fn encode_with_signer(
        &self,
        payload: &JwtPayload,
        header: &JwsHeader,
        signer: &dyn JwsSigner,
    ) -> Result<String, JoseError> {
        (|| -> anyhow::Result<String> {
            if let Some(vals) = header.critical() {
                if vals.contains(&"b64") {
                    bail!("JWT is not support b64 header claim.");
                }
            }

            let payload_bytes = serde_json::to_vec(payload.claims_set()).unwrap();
            let jwt = self
                .jws_context
                .serialize_compact(&payload_bytes, header, signer)?;
            Ok(jwt)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwtFormat(err),
        })
    }

    /// Return the string repsentation of the JWT with the encrypting algorithm.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWE heaser claims.
    /// * `encrypter` - a encrypter object.
    pub fn encode_with_encrypter(
        &self,
        payload: &JwtPayload,
        header: &JweHeader,
        encrypter: &dyn JweEncrypter,
    ) -> Result<String, JoseError> {
        let payload_bytes = serde_json::to_vec(payload.claims_set()).unwrap();
        let jwt = self
            .jwe_context
            .serialize_compact(&payload_bytes, header, encrypter)?;
        Ok(jwt)
    }

    /// Return the Jose header decoded from JWT.
    ///
    /// # Arguments
    ///
    /// * `input` - a JWT string representation.
    pub fn decode_header(&self, input: impl AsRef<[u8]>) -> Result<Box<dyn JoseHeader>, JoseError> {
        (|| -> anyhow::Result<Box<dyn JoseHeader>> {
            let input = input.as_ref();
            let parts: Vec<&[u8]> = input.split(|b| *b == '.' as u8).collect();
            if parts.len() == 3 {
                // JWS
                let header = base64::decode_config(parts[0], base64::URL_SAFE_NO_PAD)?;
                let header: Map<String, Value> = serde_json::from_slice(&header)?;
                let header = JwsHeader::from_map(header)?;
                Ok(Box::new(header))
            } else if parts.len() == 5 {
                // JWE
                let header = base64::decode_config(parts[0], base64::URL_SAFE_NO_PAD)?;
                let header: Map<String, Value> = serde_json::from_slice(&header)?;
                let header = JweHeader::from_map(header)?;
                Ok(Box::new(header))
            } else {
                bail!("The input cannot be recognized as a JWT.");
            }
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwtFormat(err),
        })
    }

    /// Return the JWT object decoded with the "none" algorithm.
    ///
    /// # Arguments
    ///
    /// * `input` - a JWT string representation.
    pub fn decode_unsecured(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<(JwtPayload, JwsHeader), JoseError> {
        self.decode_with_verifier(input, &jwt::None.verifier())
    }

    /// Return the JWT object decoded by the selected verifier.
    ///
    /// # Arguments
    ///
    /// * `verifier` - a verifier of the signing algorithm.
    /// * `input` - a JWT string representation.
    pub fn decode_with_verifier(
        &self,
        input: impl AsRef<[u8]>,
        verifier: &dyn JwsVerifier,
    ) -> Result<(JwtPayload, JwsHeader), JoseError> {
        self.decode_with_verifier_selector(input, |_header| Ok(Some(verifier)))
    }

    /// Return the JWT object decoded with a selected verifying algorithm.
    ///
    /// # Arguments
    ///
    /// * `input` - a JWT string representation.
    /// * `selector` - a function for selecting the verifying algorithm.
    pub fn decode_with_verifier_selector<'a, F>(
        &self,
        input: impl AsRef<[u8]>,
        selector: F,
    ) -> Result<(JwtPayload, JwsHeader), JoseError>
    where
        F: Fn(&JwsHeader) -> Result<Option<&'a dyn JwsVerifier>, JoseError>,
    {
        (|| -> anyhow::Result<(JwtPayload, JwsHeader)> {
            let (payload, header) =
                self.jws_context
                    .deserialize_compact_with_selector(input, |header| {
                        (|| -> anyhow::Result<Option<&'a dyn JwsVerifier>> {
                            let verifier = match selector(&header)? {
                                Some(val) => val,
                                None => return Ok(None),
                            };

                            if self.is_acceptable_critical("b64") {
                                bail!("JWT is not supported b64 header claim.");
                            }

                            Ok(Some(verifier))
                        })()
                        .map_err(|err| {
                            match err.downcast::<JoseError>() {
                                Ok(err) => err,
                                Err(err) => JoseError::InvalidJwtFormat(err),
                            }
                        })
                    })?;

            let payload: Map<String, Value> = serde_json::from_slice(&payload)?;
            let payload = JwtPayload::from_map(payload)?;

            Ok((payload, header))
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwtFormat(err),
        })
    }

    /// Return the JWT object decoded by using a JWK set.
    ///
    /// # Arguments
    ///
    /// * `input` - a JWT string representation.
    /// * `jwk_set` - a JWK set.
    /// * `selector` - a function for selecting the verifying algorithm.
    pub fn decode_with_verifier_in_jwk_set<F>(
        &self,
        input: impl AsRef<[u8]>,
        jwk_set: &JwkSet,
        selector: F,
    ) -> Result<(JwtPayload, JwsHeader), JoseError>
    where
        F: Fn(&Jwk) -> Result<Option<&dyn JwsVerifier>, JoseError>,
    {
        self.decode_with_verifier_selector(input, |header| {
            let key_id = match header.key_id() {
                Some(val) => val,
                None => return Ok(None),
            };

            for jwk in jwk_set.get(key_id) {
                if let Some(val) = selector(jwk)? {
                    return Ok(Some(val));
                }
            }
            Ok(None)
        })
    }

    /// Return the JWT object decoded by the selected decrypter.
    ///
    /// # Arguments
    ///
    /// * `input` - a JWT string representation.
    /// * `decrypter` - a decrypter of the decrypting algorithm.
    pub fn decode_with_decrypter(
        &self,
        input: impl AsRef<[u8]>,
        decrypter: &dyn JweDecrypter,
    ) -> Result<(JwtPayload, JweHeader), JoseError> {
        self.decode_with_decrypter_selector(input, |_header| Ok(Some(decrypter)))
    }

    /// Return the JWT object decoded with a selected decrypting algorithm.
    ///
    /// # Arguments
    ///
    /// * `input` - a JWT string representation.
    /// * `decrypter_selector` - a function for selecting the decrypting algorithm.
    pub fn decode_with_decrypter_selector<'a, F>(
        &self,
        input: impl AsRef<[u8]>,
        selector: F,
    ) -> Result<(JwtPayload, JweHeader), JoseError>
    where
        F: Fn(&JweHeader) -> Result<Option<&'a dyn JweDecrypter>, JoseError>,
    {
        (|| -> anyhow::Result<(JwtPayload, JweHeader)> {
            let (payload, header) =
                self.jwe_context
                    .deserialize_compact_with_selector(input, |header| {
                        let decrypter = match selector(&header)? {
                            Some(val) => val,
                            None => return Ok(None),
                        };

                        Ok(Some(decrypter))
                    })?;

            let payload: Map<String, Value> = serde_json::from_slice(&payload)?;
            let payload = JwtPayload::from_map(payload)?;

            Ok((payload, header))
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwtFormat(err),
        })
    }

    /// Return the JWT object decoded by using a JWK set.
    ///
    /// # Arguments
    ///
    /// * `input` - a JWT string representation.
    /// * `jwk_set` - a JWK set.
    /// * `selector` - a function for selecting the decrypting algorithm.
    pub fn decode_with_decrypter_in_jwk_set<F>(
        &self,
        input: impl AsRef<[u8]>,
        jwk_set: &JwkSet,
        selector: F,
    ) -> Result<(JwtPayload, JweHeader), JoseError>
    where
        F: Fn(&Jwk) -> Result<Option<&dyn JweDecrypter>, JoseError>,
    {
        self.decode_with_decrypter_selector(input, |header| {
            let key_id = match header.key_id() {
                Some(val) => val,
                None => return Ok(None),
            };

            for jwk in jwk_set.get(key_id) {
                if let Some(val) = selector(jwk)? {
                    return Ok(Some(val));
                }
            }
            Ok(None)
        })
    }
}
