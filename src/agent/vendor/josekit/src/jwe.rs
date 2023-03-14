//! JSON Web Encryption (JWE) support.

pub mod alg;
pub mod enc;
mod jwe_algorithm;
mod jwe_compression;
mod jwe_content_encryption;
mod jwe_context;
mod jwe_header;
mod jwe_header_set;
pub mod zip;

use once_cell::sync::Lazy;

use crate::JoseError;

pub use crate::jwe::jwe_algorithm::JweAlgorithm;
pub use crate::jwe::jwe_algorithm::JweDecrypter;
pub use crate::jwe::jwe_algorithm::JweEncrypter;
pub use crate::jwe::jwe_compression::JweCompression;
pub use crate::jwe::jwe_content_encryption::JweContentEncryption;
pub use crate::jwe::jwe_context::JweContext;
pub use crate::jwe::jwe_header::JweHeader;
pub use crate::jwe::jwe_header_set::JweHeaderSet;

pub use crate::jwe::alg::direct::DirectJweAlgorithm::Dir;

use crate::jwe::alg::ecdh_es::EcdhEsJweAlgorithm;
pub use EcdhEsJweAlgorithm::EcdhEs as ECDH_ES;
pub use EcdhEsJweAlgorithm::EcdhEsA128kw as ECDH_ES_A128KW;
pub use EcdhEsJweAlgorithm::EcdhEsA192kw as ECDH_ES_A192KW;
pub use EcdhEsJweAlgorithm::EcdhEsA256kw as ECDH_ES_A256KW;

use crate::jwe::alg::aeskw::AeskwJweAlgorithm;
pub use AeskwJweAlgorithm::A128kw as A128KW;
pub use AeskwJweAlgorithm::A192kw as A192KW;
pub use AeskwJweAlgorithm::A256kw as A256KW;

use crate::jwe::alg::aesgcmkw::AesgcmkwJweAlgorithm;
pub use AesgcmkwJweAlgorithm::A128gcmkw as A128GCMKW;
pub use AesgcmkwJweAlgorithm::A192gcmkw as A192GCMKW;
pub use AesgcmkwJweAlgorithm::A256gcmkw as A256GCMKW;

use crate::jwe::alg::pbes2_hmac_aeskw::Pbes2HmacAeskwJweAlgorithm;
pub use Pbes2HmacAeskwJweAlgorithm::Pbes2Hs256A128kw as PBES2_HS256_A128KW;
pub use Pbes2HmacAeskwJweAlgorithm::Pbes2Hs384A192kw as PBES2_HS384_A192KW;
pub use Pbes2HmacAeskwJweAlgorithm::Pbes2Hs512A256kw as PBES2_HS512_A256KW;

use crate::jwe::alg::rsaes::RsaesJweAlgorithm;
#[allow(deprecated)]
pub use RsaesJweAlgorithm::Rsa1_5 as RSA1_5;
pub use RsaesJweAlgorithm::RsaOaep as RSA_OAEP;
pub use RsaesJweAlgorithm::RsaOaep256 as RSA_OAEP_256;
pub use RsaesJweAlgorithm::RsaOaep384 as RSA_OAEP_384;
pub use RsaesJweAlgorithm::RsaOaep512 as RSA_OAEP_512;

static DEFAULT_CONTEXT: Lazy<JweContext> = Lazy::new(|| JweContext::new());

/// Return a representation of the data that is formatted by compact serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWS heaser claims.
/// * `encrypter` - The JWS encrypter.
pub fn serialize_compact(
    payload: &[u8],
    header: &JweHeader,
    encrypter: &dyn JweEncrypter,
) -> Result<String, JoseError> {
    DEFAULT_CONTEXT.serialize_compact(payload, header, encrypter)
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
    header: &JweHeader,
    selector: F,
) -> Result<String, JoseError>
where
    F: Fn(&JweHeader) -> Option<&'a dyn JweEncrypter>,
{
    DEFAULT_CONTEXT.serialize_compact_with_selector(payload, header, selector)
}

/// Return a representation of the data that is formatted by flattened json serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWE shared protected and unprotected header claims.
/// * `recipients` - The JWE header claims and the JWE encrypter pair for recipients.
/// * `aad` - The JWE additional authenticated data.
pub fn serialize_general_json(
    payload: &[u8],
    header: Option<&JweHeaderSet>,
    recipients: &[(Option<&JweHeader>, &dyn JweEncrypter)],
    aad: Option<&[u8]>,
) -> Result<String, JoseError> {
    DEFAULT_CONTEXT.serialize_general_json(payload, header, recipients, aad)
}

/// Return a representation of the data that is formatted by flattened json serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWS shared protected and unprotected header claims.
/// * `recipient_headers` - The JWE unprotected header claims for recipients.
/// * `aad` - The JWE additional authenticated data.
/// * `selector` - a function for selecting the encrypting algorithm.
pub fn serialize_general_json_with_selector<'a, F>(
    payload: &[u8],
    header: Option<&JweHeaderSet>,
    recipient_headers: &[Option<&JweHeader>],
    aad: Option<&[u8]>,
    selector: F,
) -> Result<String, JoseError>
where
    F: Fn(usize, &JweHeader) -> Option<&'a dyn JweEncrypter>,
{
    DEFAULT_CONTEXT.serialize_general_json_with_selector(
        payload,
        header,
        recipient_headers,
        aad,
        selector,
    )
}

/// Return a representation of the data that is formatted by flattened json serialization.
///
/// # Arguments
///
/// * `header` - The JWE shared protected and unprotected header claims.
/// * `recipient_header` - The JWE unprotected header claims.
/// * `aad` - The JWE additional authenticated data.
/// * `payload` - The payload data.
/// * `encrypter` - The JWS encrypter.
pub fn serialize_flattened_json(
    payload: &[u8],
    header: Option<&JweHeaderSet>,
    recipient_header: Option<&JweHeader>,
    aad: Option<&[u8]>,
    encrypter: &dyn JweEncrypter,
) -> Result<String, JoseError> {
    DEFAULT_CONTEXT.serialize_flattened_json(payload, header, recipient_header, aad, encrypter)
}

/// Return a representation of the data that is formatted by flatted json serialization.
///
/// # Arguments
///
/// * `payload` - The payload data.
/// * `header` - The JWS shared protected and unprotected header claims.
/// * `recipient_header` - The JWS unprotected header claims.
/// * `aad` - The JWE additional authenticated data.
/// * `selector` - a function for selecting the encrypting algorithm.
pub fn serialize_flattened_json_with_selector<'a, F>(
    payload: &[u8],
    header: Option<&JweHeaderSet>,
    recipient_header: Option<&JweHeader>,
    aad: Option<&[u8]>,
    selector: F,
) -> Result<String, JoseError>
where
    F: Fn(&JweHeader) -> Option<&'a dyn JweEncrypter>,
{
    DEFAULT_CONTEXT.serialize_flattened_json_with_selector(
        payload,
        header,
        recipient_header,
        aad,
        selector,
    )
}

/// Deserialize the input that is formatted by compact serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `decrypter` - The JWS decrypter.
pub fn deserialize_compact(
    input: &str,
    decrypter: &dyn JweDecrypter,
) -> Result<(Vec<u8>, JweHeader), JoseError> {
    DEFAULT_CONTEXT.deserialize_compact(input, decrypter)
}

/// Deserialize the input that is formatted by compact serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `selector` - a function for selecting the decrypting algorithm.
pub fn deserialize_compact_with_selector<'a, F>(
    input: &str,
    selector: F,
) -> Result<(Vec<u8>, JweHeader), JoseError>
where
    F: Fn(&JweHeader) -> Result<Option<&'a dyn JweDecrypter>, JoseError>,
{
    DEFAULT_CONTEXT.deserialize_compact_with_selector(input, selector)
}

/// Deserialize the input that is formatted by flattened json serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `header` - The decoded JWS header claims.
/// * `decrypter` - The JWE decrypter.
pub fn deserialize_json<'a>(
    input: &str,
    decrypter: &'a dyn JweDecrypter,
) -> Result<(Vec<u8>, JweHeader), JoseError> {
    DEFAULT_CONTEXT.deserialize_json(input, decrypter)
}

/// Deserialize the input that is formatted by flattened json serialization.
///
/// # Arguments
///
/// * `input` - The input data.
/// * `selector` - a function for selecting the decrypting algorithm.
pub fn deserialize_json_with_selector<'a, F>(
    input: &str,
    selector: F,
) -> Result<(Vec<u8>, JweHeader), JoseError>
where
    F: Fn(&JweHeader) -> Result<Option<&'a dyn JweDecrypter>, JoseError>,
{
    DEFAULT_CONTEXT.deserialize_json_with_selector(input, selector)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use anyhow::Result;

    use crate::jwe::{
        self, Dir, JweAlgorithm, JweHeader, JweHeaderSet, ECDH_ES_A128KW, PBES2_HS256_A128KW,
        RSA_OAEP,
    };
    use crate::jwk::Jwk;
    use crate::util;
    use crate::Value;

    #[test]
    fn test_jwe_compact_serialization() -> Result<()> {
        for enc in vec![
            "A128CBC-HS256",
            "A192CBC-HS384",
            "A256CBC-HS512",
            "A128GCM",
            "A256GCM",
            "A256GCM",
        ] {
            let mut src_header = JweHeader::new();
            src_header.set_content_encryption(enc);
            src_header.set_token_type("JWT");
            let src_payload = b"test payload!";

            //println!("{}", enc);

            let alg = Dir;
            let key = match enc {
                "A128CBC-HS256" => util::random_bytes(32),
                "A192CBC-HS384" => util::random_bytes(48),
                "A256CBC-HS512" => util::random_bytes(64),
                "A128GCM" => util::random_bytes(16),
                "A192GCM" => util::random_bytes(24),
                "A256GCM" => util::random_bytes(32),
                _ => unreachable!(),
            };
            let encrypter = alg.encrypter_from_bytes(&key)?;
            let jwe = jwe::serialize_compact(src_payload, &src_header, &encrypter)?;

            let decrypter = alg.decrypter_from_bytes(&key)?;
            let (dst_payload, dst_header) = jwe::deserialize_compact(&jwe, &decrypter)?;

            src_header.set_claim("alg", Some(Value::String(alg.name().to_string())))?;
            assert_eq!(src_header, dst_header);
            assert_eq!(src_payload.to_vec(), dst_payload);
        }

        Ok(())
    }

    #[test]
    fn test_jwe_json_serialization() -> Result<()> {
        let alg = RSA_OAEP;

        let private_key = load_file("pem/RSA_2048bit_private.pem")?;
        let public_key = load_file("pem/RSA_2048bit_public.pem")?;

        let src_payload = b"test payload!";
        let mut src_header = JweHeaderSet::new();
        src_header.set_key_id("xxx", true);
        src_header.set_token_type("JWT", false);
        let mut src_rheader = JweHeader::new();
        src_rheader.set_content_encryption("A128GCM");

        let encrypter = alg.encrypter_from_pem(&public_key)?;
        let jwt = jwe::serialize_flattened_json(
            src_payload,
            Some(&src_header),
            Some(&src_rheader),
            None,
            &encrypter,
        )?;

        let decrypter = alg.decrypter_from_pem(&private_key)?;
        let (dst_payload, dst_header) = jwe::deserialize_json(&jwt, &decrypter)?;

        src_header.set_algorithm(alg.name(), true);
        assert_eq!(
            src_rheader.content_encryption(),
            dst_header.content_encryption()
        );
        assert_eq!(src_header.key_id(), dst_header.key_id());
        assert_eq!(src_header.token_type(), dst_header.token_type());
        assert_eq!(src_payload.to_vec(), dst_payload);

        Ok(())
    }

    #[test]
    fn test_jwe_general_json_serialization() -> Result<()> {
        let public_key_1 = load_file("pem/RSA_2048bit_public.pem")?;
        let public_key_2 = load_file("der/EC_P-256_spki_public.der")?;
        let public_key_3 = load_file("jwk/oct_128bit_private.jwk")?;

        let private_key = load_file("der/EC_P-256_pkcs8_private.der")?;

        let src_payload = b"test payload!";

        let mut src_header = JweHeaderSet::new();
        src_header.set_content_encryption("A128CBC-HS256", true);
        src_header.set_token_type("JWT-1", false);

        let mut src_rheader_1 = JweHeader::new();
        src_rheader_1.set_key_id("xxx-1");
        let encrypter_1 = RSA_OAEP.encrypter_from_pem(&public_key_1)?;

        let mut src_rheader_2 = JweHeader::new();
        src_rheader_2.set_key_id("xxx-2");
        let encrypter_2 = ECDH_ES_A128KW.encrypter_from_der(&public_key_2)?;

        let mut src_rheader_3 = JweHeader::new();
        src_rheader_3.set_key_id("xxx-3");
        let encrypter_3 =
            PBES2_HS256_A128KW.encrypter_from_jwk(&Jwk::from_bytes(&public_key_3)?)?;

        let json = jwe::serialize_general_json(
            src_payload,
            Some(&src_header),
            &vec![
                (Some(&src_rheader_1), &*encrypter_1),
                (Some(&src_rheader_2), &*encrypter_2),
                (Some(&src_rheader_3), &*encrypter_3),
            ],
            None,
        )?;

        let decrypter = ECDH_ES_A128KW.decrypter_from_der(&private_key)?;
        let (dst_payload, dst_header) = jwe::deserialize_json(&json, &decrypter)?;

        assert_eq!(dst_header.algorithm(), Some("ECDH-ES+A128KW"));
        assert_eq!(src_header.token_type(), dst_header.token_type());
        assert_eq!(src_rheader_2.key_id(), dst_header.key_id());
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
