//! JSON Web Signature (JWS) support.

pub mod alg;
mod jws_algorithm;
mod jws_context;
mod jws_header;
mod jws_header_set;

use once_cell::sync::Lazy;

use crate::JoseError;

pub use crate::jws::jws_algorithm::JwsAlgorithm;
pub use crate::jws::jws_algorithm::JwsSigner;
pub use crate::jws::jws_algorithm::JwsVerifier;
pub use crate::jws::jws_context::JwsContext;
pub use crate::jws::jws_header::JwsHeader;
pub use crate::jws::jws_header_set::JwsHeaderSet;

use crate::jws::alg::hmac::HmacJwsAlgorithm;
pub use HmacJwsAlgorithm::Hs256 as HS256;
pub use HmacJwsAlgorithm::Hs384 as HS384;
pub use HmacJwsAlgorithm::Hs512 as HS512;

use crate::jws::alg::rsassa::RsassaJwsAlgorithm;
pub use RsassaJwsAlgorithm::Rs256 as RS256;
pub use RsassaJwsAlgorithm::Rs384 as RS384;
pub use RsassaJwsAlgorithm::Rs512 as RS512;

use crate::jws::alg::rsassa_pss::RsassaPssJwsAlgorithm;
pub use RsassaPssJwsAlgorithm::Ps256 as PS256;
pub use RsassaPssJwsAlgorithm::Ps384 as PS384;
pub use RsassaPssJwsAlgorithm::Ps512 as PS512;

use crate::jws::alg::ecdsa::EcdsaJwsAlgorithm;
pub use EcdsaJwsAlgorithm::Es256 as ES256;
pub use EcdsaJwsAlgorithm::Es256k as ES256K;
pub use EcdsaJwsAlgorithm::Es384 as ES384;
pub use EcdsaJwsAlgorithm::Es512 as ES512;

use crate::jws::alg::eddsa::EddsaJwsAlgorithm;
pub use EddsaJwsAlgorithm::Eddsa as EdDSA;

static DEFAULT_CONTEXT: Lazy<JwsContext> = Lazy::new(|| JwsContext::new());

/// Return a representation of the data that is formatted by compact serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWS heaser claims.
/// * `signer` - The JWS signer.
pub fn serialize_compact(
    payload: &[u8],
    header: &JwsHeader,
    signer: &dyn JwsSigner,
) -> Result<String, JoseError> {
    DEFAULT_CONTEXT.serialize_compact(payload, header, signer)
}

/// Return a representation of the data that is formatted by compact serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWS heaser claims.
/// * `selector` - a function for selecting the signing algorithm.
pub fn serialize_compact_with_selector<'a, F>(
    payload: &[u8],
    header: &JwsHeader,
    selector: F,
) -> Result<String, JoseError>
where
    F: Fn(&JwsHeader) -> Option<&'a dyn JwsSigner>,
{
    DEFAULT_CONTEXT.serialize_compact_with_selector(payload, header, selector)
}

/// Return a representation of the data that is formatted by general json serialization.
///
/// # Arguments
///
/// * `protected` - The JWS protected header claims.
/// * `header` - The JWS unprotected header claims.
/// * `payload` - The payload data.
/// * `signers` - The JWS signer.
pub fn serialize_general_json(
    payload: &[u8],
    signers: &[(&JwsHeaderSet, &dyn JwsSigner)],
) -> Result<String, JoseError> {
    DEFAULT_CONTEXT.serialize_general_json(payload, signers)
}

/// Return a representation of the data that is formatted by general json serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `headers` - The protected and unprotected header claims.
/// * `selector` - a function for selecting the signing algorithm.
pub fn serialize_general_json_with_selecter<'a, F>(
    payload: &[u8],
    headers: &[&JwsHeaderSet],
    selector: F,
) -> Result<String, JoseError>
where
    F: Fn(usize, &JwsHeader) -> Option<&'a dyn JwsSigner>,
{
    DEFAULT_CONTEXT.serialize_general_json_with_selecter(payload, headers, selector)
}

/// Return a representation of the data that is formatted by flattened json serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWS protected and unprotected header claims.
/// * `signer` - The JWS signer.
pub fn serialize_flattened_json(
    payload: &[u8],
    header: &JwsHeaderSet,
    signer: &dyn JwsSigner,
) -> Result<String, JoseError> {
    DEFAULT_CONTEXT.serialize_flattened_json(payload, header, signer)
}

/// Return a representation of the data that is formatted by flatted json serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWS protected and unprotected header claims.
/// * `selector` - a function for selecting the signing algorithm.
pub fn serialize_flattened_json_with_selector<'a, F>(
    payload: &[u8],
    header: &JwsHeaderSet,
    selector: F,
) -> Result<String, JoseError>
where
    F: Fn(&JwsHeader) -> Option<&'a dyn JwsSigner>,
{
    DEFAULT_CONTEXT.serialize_flattened_json_with_selector(payload, header, selector)
}

/// Deserialize the input that is formatted by compact serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `header` - The decoded JWS header claims.
/// * `verifier` - The JWS verifier.
pub fn deserialize_compact(
    input: impl AsRef<[u8]>,
    verifier: &dyn JwsVerifier,
) -> Result<(Vec<u8>, JwsHeader), JoseError> {
    DEFAULT_CONTEXT.deserialize_compact(input, verifier)
}

/// Deserialize the input that is formatted by compact serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `header` - The decoded JWS header claims.
/// * `selector` - a function for selecting the verifying algorithm.
pub fn deserialize_compact_with_selector<'a, F>(
    input: impl AsRef<[u8]>,
    selector: F,
) -> Result<(Vec<u8>, JwsHeader), JoseError>
where
    F: Fn(&JwsHeader) -> Result<Option<&'a dyn JwsVerifier>, JoseError>,
{
    DEFAULT_CONTEXT.deserialize_compact_with_selector(input, selector)
}

/// Deserialize the input that is formatted by json serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `header` - The decoded JWS header claims.
/// * `verifier` - The JWS verifier.
pub fn deserialize_json<'a>(
    input: impl AsRef<[u8]>,
    verifier: &'a dyn JwsVerifier,
) -> Result<(Vec<u8>, JwsHeader), JoseError> {
    DEFAULT_CONTEXT.deserialize_json(input, verifier)
}

/// Deserialize the input that is formatted by json serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `header` - The decoded JWS header claims.
/// * `selector` - a function for selecting the verifying algorithm.
pub fn deserialize_json_with_selector<'a, F>(
    input: impl AsRef<[u8]>,
    selector: F,
) -> Result<(Vec<u8>, JwsHeader), JoseError>
where
    F: Fn(&JwsHeader) -> Result<Option<&'a dyn JwsVerifier>, JoseError>,
{
    DEFAULT_CONTEXT.deserialize_json_with_selector(input, selector)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use anyhow::Result;

    use crate::jws::{self, EdDSA, JwsHeader, JwsHeaderSet, ES256, RS256};
    use crate::Value;

    #[test]
    fn test_jws_compact_serialization() -> Result<()> {
        let alg = RS256;

        let private_key = load_file("pem/RSA_2048bit_private.pem")?;
        let public_key = load_file("pem/RSA_2048bit_public.pem")?;

        let mut src_header = JwsHeader::new();
        src_header.set_token_type("JWT");
        let src_payload = b"test payload!";
        let signer = alg.signer_from_pem(&private_key)?;
        let jwt = jws::serialize_compact(src_payload, &src_header, &signer)?;

        let verifier = alg.verifier_from_pem(&public_key)?;
        let (dst_payload, dst_header) = jws::deserialize_compact(&jwt, &verifier)?;

        src_header.set_claim("alg", Some(Value::String(alg.name().to_string())))?;
        assert_eq!(src_header, dst_header);
        assert_eq!(src_payload.to_vec(), dst_payload);

        Ok(())
    }

    #[test]
    fn test_jws_json_serialization() -> Result<()> {
        let alg = RS256;

        let private_key = load_file("pem/RSA_2048bit_private.pem")?;
        let public_key = load_file("pem/RSA_2048bit_public.pem")?;

        let src_payload = b"test payload!";
        let mut src_header = JwsHeaderSet::new();
        src_header.set_key_id("xxx", true);
        src_header.set_token_type("JWT", false);
        let signer = alg.signer_from_pem(&private_key)?;
        let jwt = jws::serialize_flattened_json(src_payload, &src_header, &signer)?;

        let verifier = alg.verifier_from_pem(&public_key)?;
        let (dst_payload, dst_header) = jws::deserialize_json(&jwt, &verifier)?;

        src_header.set_algorithm(alg.name(), true);
        assert_eq!(src_header.key_id(), dst_header.key_id());
        assert_eq!(src_header.token_type(), dst_header.token_type());
        assert_eq!(src_payload.to_vec(), dst_payload);

        Ok(())
    }

    #[test]
    fn test_jws_general_json_serialization() -> Result<()> {
        let private_key_1 = load_file("pem/RSA_2048bit_private.pem")?;
        let private_key_2 = load_file("pem/EC_P-256_private.pem")?;
        let private_key_3 = load_file("pem/ED25519_private.pem")?;

        let public_key = load_file("pem/EC_P-256_public.pem")?;

        let src_payload = b"test payload!";

        let mut src_header_1 = JwsHeaderSet::new();
        src_header_1.set_key_id("xxx-1", true);
        src_header_1.set_token_type("JWT-1", false);
        let signer_1 = RS256.signer_from_pem(&private_key_1)?;

        let mut src_header_2 = JwsHeaderSet::new();
        src_header_2.set_key_id("xxx-2", true);
        src_header_2.set_token_type("JWT-2", false);
        let signer_2 = ES256.signer_from_pem(&private_key_2)?;

        let mut src_header_3 = JwsHeaderSet::new();
        src_header_3.set_key_id("xxx-3", true);
        src_header_3.set_token_type("JWT-3", false);
        let signer_3 = EdDSA.signer_from_pem(&private_key_3)?;

        let json = jws::serialize_general_json(
            src_payload,
            &vec![
                (&src_header_1, &*signer_1),
                (&src_header_2, &*signer_2),
                (&src_header_3, &*signer_3),
            ],
        )?;

        let verifier = ES256.verifier_from_pem(&public_key)?;
        let (dst_payload, dst_header) = jws::deserialize_json(&json, &verifier)?;

        assert_eq!(dst_header.algorithm(), Some("ES256"));
        assert_eq!(src_header_2.key_id(), dst_header.key_id());
        assert_eq!(src_header_2.token_type(), dst_header.token_type());
        assert_eq!(src_payload.to_vec(), dst_payload);

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
