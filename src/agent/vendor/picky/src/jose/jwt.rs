use super::jwe::Jwe;
use crate::jose::jwe::{JweAlg, JweEnc, JweError, JweHeader};
use crate::jose::jws::{Jws, JwsAlg, JwsError, JwsHeader};
use crate::key::{PrivateKey, PublicKey};
use core::fmt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;

// === error type === //

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JwtError {
    /// JWS error
    #[error("JWS error: {source}")]
    Jws { source: JwsError },

    /// JWE error
    #[error("JWE error: {source}")]
    Jwe { source: JweError },

    /// Json error
    #[error("JSON error: {source}")]
    Json { source: serde_json::Error },

    /// registered claim type is invalid
    #[error("registered claim `{claim}` has invalid type")]
    InvalidRegisteredClaimType { claim: &'static str },

    /// a required claim is missing
    #[error("required claim `{claim}` is missing")]
    RequiredClaimMissing { claim: &'static str },

    /// token not yet valid
    #[error("token not yet valid (not before: {}, now: {} [leeway: {}])", not_before, now.numeric_date, now.leeway)]
    NotYetValid { not_before: i64, now: JwtDate },

    /// token expired
    #[error("token expired (not after: {}, now: {} [leeway: {}])", not_after, now.numeric_date, now.leeway)]
    Expired { not_after: i64, now: JwtDate },

    /// validator is invalid
    #[error("invalid validator: {description}")]
    InvalidValidator { description: &'static str },
}

impl From<JwsError> for JwtError {
    fn from(s: JwsError) -> Self {
        Self::Jws { source: s }
    }
}

impl From<serde_json::Error> for JwtError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json { source: e }
    }
}

impl From<JweError> for JwtError {
    fn from(s: JweError) -> Self {
        Self::Jwe { source: s }
    }
}

// === Validation states === //

pub struct CheckedState<C> {
    pub claims: C,
}

impl<C> Clone for CheckedState<C>
where
    C: Clone,
{
    fn clone(&self) -> Self {
        Self {
            claims: self.claims.clone(),
        }
    }
}

impl<C> fmt::Debug for CheckedState<C>
where
    C: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ValidatedClaims({:?})", self.claims)
    }
}

#[derive(Clone, Debug)]
pub struct UncheckedState {
    payload: Vec<u8>,
}

// === JWT date === //

/// Represent date as defined by [RFC7519](https://tools.ietf.org/html/rfc7519#section-2).
///
/// A leeway can be configured to account clock skew when comparing with another date.
/// Should be small (less than 120).
#[derive(Clone, Debug)]
pub struct JwtDate {
    pub numeric_date: i64,
    pub leeway: u16,
}

impl JwtDate {
    pub const fn new(numeric_date: i64) -> Self {
        Self {
            numeric_date,
            leeway: 0,
        }
    }

    pub const fn new_with_leeway(numeric_date: i64, leeway: u16) -> Self {
        Self { numeric_date, leeway }
    }

    pub const fn is_before(&self, other_numeric_date: i64) -> bool {
        self.numeric_date <= other_numeric_date + self.leeway as i64
    }

    pub const fn is_before_strict(&self, other_numeric_date: i64) -> bool {
        self.numeric_date < other_numeric_date + self.leeway as i64
    }

    pub const fn is_after(&self, other_numeric_date: i64) -> bool {
        self.numeric_date >= other_numeric_date - self.leeway as i64
    }

    pub const fn is_after_strict(&self, other_numeric_date: i64) -> bool {
        self.numeric_date > other_numeric_date - self.leeway as i64
    }
}

// === validator === //

#[derive(Debug, Clone, Copy)]
enum CheckStrictness {
    Ignored,
    Optional,
    Required,
}

#[derive(Debug, Clone)]
pub struct JwtValidator {
    current_date: Option<JwtDate>,
    expiration_claim: CheckStrictness,
    not_before_claim: CheckStrictness,
}

pub const NO_CHECK_VALIDATOR: JwtValidator = JwtValidator::no_check();

impl JwtValidator {
    /// Check signature and the registered exp and nbf claims. If a claim is missing token is rejected.
    pub const fn strict(current_date: JwtDate) -> Self {
        Self {
            current_date: Some(current_date),
            expiration_claim: CheckStrictness::Required,
            not_before_claim: CheckStrictness::Required,
        }
    }

    /// Check signature and the registered exp and nbf claims. Token isn't rejected if a claim is missing.
    pub const fn lenient(current_date: JwtDate) -> Self {
        Self {
            current_date: Some(current_date),
            expiration_claim: CheckStrictness::Optional,
            not_before_claim: CheckStrictness::Optional,
        }
    }

    /// No check.
    pub const fn no_check() -> Self {
        Self {
            current_date: None,
            expiration_claim: CheckStrictness::Ignored,
            not_before_claim: CheckStrictness::Ignored,
        }
    }

    pub fn current_date(self, current_date: JwtDate) -> Self {
        Self {
            current_date: Some(current_date),
            expiration_claim: CheckStrictness::Required,
            not_before_claim: CheckStrictness::Required,
        }
    }

    pub fn expiration_check_required(self) -> Self {
        Self {
            expiration_claim: CheckStrictness::Required,
            ..self
        }
    }

    pub fn expiration_check_optional(self) -> Self {
        Self {
            expiration_claim: CheckStrictness::Optional,
            ..self
        }
    }

    pub fn expiration_check_ignored(self) -> Self {
        Self {
            expiration_claim: CheckStrictness::Ignored,
            ..self
        }
    }

    pub fn not_before_check_required(self) -> Self {
        Self {
            not_before_claim: CheckStrictness::Required,
            ..self
        }
    }

    pub fn not_before_check_optional(self) -> Self {
        Self {
            not_before_claim: CheckStrictness::Optional,
            ..self
        }
    }

    pub fn not_before_check_ignored(self) -> Self {
        Self {
            not_before_claim: CheckStrictness::Ignored,
            ..self
        }
    }
}

// === JWT === //

const JWT_TYPE: &str = "JWT";
const EXPIRATION_TIME_CLAIM: &str = "exp";
const NOT_BEFORE_CLAIM: &str = "nbf";

pub struct Jwt<H, State> {
    pub header: H,
    pub state: State,
}

pub type JwtSig = Jwt<JwsHeader, UncheckedState>;
pub type CheckedJwtSig<C> = Jwt<JwsHeader, CheckedState<C>>;
pub type JwtEnc = Jwt<JweHeader, UncheckedState>;
pub type CheckedJwtEnc<C> = Jwt<JweHeader, CheckedState<C>>;

impl<H, State> Clone for Jwt<H, State>
where
    H: Clone,
    State: Clone,
{
    fn clone(&self) -> Self {
        Self {
            header: self.header.clone(),
            state: self.state.clone(),
        }
    }
}

impl<H, State> fmt::Debug for Jwt<H, State>
where
    H: fmt::Debug,
    State: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Jwt")
            .field("header", &self.header)
            .field("state", &self.state)
            .finish()
    }
}

impl<H, C> Jwt<H, CheckedState<C>> {
    pub fn new_with_header(header: H, claims: C) -> Self {
        Jwt {
            header,
            state: CheckedState { claims },
        }
    }
}

impl<C> CheckedJwtSig<C> {
    pub fn new(alg: JwsAlg, claims: C) -> Self {
        Jwt {
            header: JwsHeader {
                typ: Some(JWT_TYPE.to_owned()),
                ..JwsHeader::new(alg)
            },
            state: CheckedState { claims },
        }
    }

    pub fn new_with_cty(alg: JwsAlg, cty: impl Into<String>, claims: C) -> Self {
        Jwt {
            header: JwsHeader {
                typ: Some(JWT_TYPE.to_owned()),
                ..JwsHeader::new_with_cty(alg, cty)
            },
            state: CheckedState { claims },
        }
    }
}

impl<C> CheckedJwtSig<C>
where
    C: Serialize,
{
    pub fn encode(self, private_key: &PrivateKey) -> Result<String, JwtError> {
        let jws = Jws {
            header: self.header,
            payload: serde_json::to_vec(&self.state.claims)?,
        };
        let encoded = jws.encode(private_key)?;
        Ok(encoded)
    }
}

impl JwtSig {
    pub fn encode(self, private_key: &PrivateKey) -> Result<String, JwtError> {
        let jws = Jws {
            header: self.header,
            payload: self.state.payload,
        };
        let encoded = jws.encode(private_key)?;
        Ok(encoded)
    }
}

impl JwtSig {
    /// Verifies signature and returns decoded JWS payload.
    pub fn decode(encoded_token: &str, public_key: &PublicKey) -> Result<Self, JwtError> {
        let jws = Jws::decode(encoded_token, public_key)?;
        Ok(Self::from(jws))
    }
}

impl From<Jws> for JwtSig {
    fn from(jws: Jws) -> Self {
        Self {
            header: jws.header,
            state: UncheckedState { payload: jws.payload },
        }
    }
}

impl<C> CheckedJwtEnc<C> {
    pub fn new(alg: JweAlg, enc: JweEnc, claims: C) -> Self {
        Jwt {
            header: JweHeader {
                typ: Some(JWT_TYPE.to_owned()),
                ..JweHeader::new(alg, enc)
            },
            state: CheckedState { claims },
        }
    }

    pub fn new_with_cty(alg: JweAlg, enc: JweEnc, cty: impl Into<String>, claims: C) -> Self {
        Jwt {
            header: JweHeader {
                typ: Some(JWT_TYPE.to_owned()),
                ..JweHeader::new_with_cty(alg, enc, cty)
            },
            state: CheckedState { claims },
        }
    }
}

impl<C> CheckedJwtEnc<C>
where
    C: Serialize,
{
    /// Encode with CEK encrypted and included in the token using asymmetric cryptography.
    pub fn encode(self, asymmetric_key: &PublicKey) -> Result<String, JwtError> {
        let jwe = Jwe {
            header: self.header,
            payload: serde_json::to_vec(&self.state.claims)?,
        };
        let encoded = jwe.encode(asymmetric_key)?;
        Ok(encoded)
    }

    /// Encode with provided CEK (a symmetric key). This will ignore `alg` value and override it with "dir".
    pub fn encode_direct(self, cek: &[u8]) -> Result<String, JweError> {
        let jwe = Jwe {
            header: self.header,
            payload: serde_json::to_vec(&self.state.claims)?,
        };
        let encoded = jwe.encode_direct(cek)?;
        Ok(encoded)
    }
}

impl JwtEnc {
    /// Decode using asymmetric cryptography.
    pub fn decode(encoded_token: &str, key: &PrivateKey) -> Result<Self, JwtError> {
        let jwe = Jwe::decode(encoded_token, key)?;
        Ok(Self::from(jwe))
    }

    /// Decode with provided CEK (a symmetric key).
    pub fn decode_direct(encoded_token: &str, cek: &[u8]) -> Result<Self, JwtError> {
        let jwe = Jwe::decode_direct(encoded_token, cek)?;
        Ok(Self::from(jwe))
    }
}

impl From<Jwe> for JwtEnc {
    fn from(jwe: Jwe) -> Self {
        Self {
            header: jwe.header,
            state: UncheckedState { payload: jwe.payload },
        }
    }
}

impl<H> Jwt<H, UncheckedState> {
    /// Validate JWT claims using validator and convert payload to a user-defined typed struct.
    pub fn validate<C>(self, validator: &JwtValidator) -> Result<Jwt<H, CheckedState<C>>, JwtError>
    where
        C: DeserializeOwned,
    {
        Ok(Jwt {
            header: self.header,
            state: CheckedState {
                claims: h_decode_and_validate_claims(&self.state.payload, validator)?,
            },
        })
    }
}

fn h_decode_and_validate_claims<C: DeserializeOwned>(
    claims_json: &[u8],
    validator: &JwtValidator,
) -> Result<C, JwtError> {
    let claims = match (
        &validator.current_date,
        validator.not_before_claim,
        validator.expiration_claim,
    ) {
        (None, CheckStrictness::Required, _) | (None, _, CheckStrictness::Required) => {
            return Err(JwtError::InvalidValidator {
                description: "current date is missing",
            })
        }
        (Some(current_date), nbf_strictness, exp_strictness) => {
            let claims = serde_json::from_slice::<serde_json::Value>(claims_json)?;

            let nbf_opt = claims.get(NOT_BEFORE_CLAIM);
            match (nbf_strictness, nbf_opt) {
                (CheckStrictness::Ignored, _) | (CheckStrictness::Optional, None) => {}
                (CheckStrictness::Required, None) => {
                    return Err(JwtError::RequiredClaimMissing {
                        claim: NOT_BEFORE_CLAIM,
                    })
                }
                (_, Some(nbf)) => {
                    let nbf_i64 = nbf.as_i64().ok_or(JwtError::InvalidRegisteredClaimType {
                        claim: NOT_BEFORE_CLAIM,
                    })?;
                    if !current_date.is_after(nbf_i64) {
                        return Err(JwtError::NotYetValid {
                            not_before: nbf_i64,
                            now: current_date.clone(),
                        });
                    }
                }
            }

            let exp_opt = claims.get(EXPIRATION_TIME_CLAIM);
            match (exp_strictness, exp_opt) {
                (CheckStrictness::Ignored, _) | (CheckStrictness::Optional, None) => {}
                (CheckStrictness::Required, None) => {
                    return Err(JwtError::RequiredClaimMissing {
                        claim: EXPIRATION_TIME_CLAIM,
                    })
                }
                (_, Some(exp)) => {
                    let exp_i64 = exp.as_i64().ok_or(JwtError::InvalidRegisteredClaimType {
                        claim: EXPIRATION_TIME_CLAIM,
                    })?;
                    if !current_date.is_before_strict(exp_i64) {
                        return Err(JwtError::Expired {
                            not_after: exp_i64,
                            now: current_date.clone(),
                        });
                    }
                }
            }

            serde_json::value::from_value(claims)?
        }
        (None, _, _) => serde_json::from_slice(claims_json)?,
    };

    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{jose::jws::RawJws, pem::Pem};
    use serde::Deserialize;
    use std::borrow::Cow;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct MyClaims {
        sub: Cow<'static, str>,
        name: Cow<'static, str>,
        admin: bool,
        iat: i32,
    }

    const fn get_strongly_typed_claims() -> MyClaims {
        MyClaims {
            sub: Cow::Borrowed("1234567890"),
            name: Cow::Borrowed("John Doe"),
            admin: true,
            iat: 1516239022,
        }
    }

    fn get_private_key_1() -> PrivateKey {
        let pk_pem = crate::test_files::RSA_2048_PK_1.parse::<Pem>().unwrap();
        PrivateKey::from_pkcs8(pk_pem.data()).unwrap()
    }

    #[test]
    fn encode_jws_rsa_sha256() {
        let claims = get_strongly_typed_claims();
        let jwt = CheckedJwtSig::new(JwsAlg::RS256, claims);
        let encoded = jwt.encode(&get_private_key_1()).unwrap();
        assert_eq!(encoded, crate::test_files::JOSE_JWT_SIG_EXAMPLE);
    }

    #[test]
    fn decode_jws_rsa_sha256() {
        let public_key = get_private_key_1().to_public_key();
        let jwt = JwtSig::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE, &public_key)
            .unwrap()
            .validate::<MyClaims>(&JwtValidator::no_check())
            .unwrap();
        assert_eq!(jwt.state.claims, get_strongly_typed_claims());

        // exp and nbf claims aren't present but this should pass with lenient validator
        let now = JwtDate::new(0);
        JwtSig::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE, &public_key)
            .unwrap()
            .validate::<MyClaims>(&JwtValidator::lenient(now))
            .unwrap();
    }

    #[test]
    fn decode_jws_invalid_validator_err() {
        let public_key = get_private_key_1().to_public_key();
        let validator = JwtValidator::no_check()
            .expiration_check_required()
            .not_before_check_optional();
        let err = JwtSig::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE, &public_key)
            .unwrap()
            .validate::<MyClaims>(&validator)
            .err()
            .unwrap();
        assert_eq!(err.to_string(), "invalid validator: current date is missing");
    }

    #[test]
    fn decode_jws_required_claim_missing_err() {
        let public_key = get_private_key_1().to_public_key();
        let now = JwtDate::new(0);
        let validator = JwtValidator::strict(now);
        let err = JwtSig::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE, &public_key)
            .unwrap()
            .validate::<MyClaims>(&validator)
            .err()
            .unwrap();
        assert_eq!(err.to_string(), "required claim `nbf` is missing");
    }

    #[test]
    fn decode_jws_rsa_sha256_using_json_value_claims() {
        let public_key = get_private_key_1().to_public_key();
        let validator = JwtValidator::no_check();
        let jwt = JwtSig::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE, &public_key)
            .unwrap()
            .validate::<serde_json::Value>(&validator)
            .unwrap();
        assert_eq!(jwt.state.claims["sub"].as_str().expect("sub"), "1234567890");
        assert_eq!(jwt.state.claims["name"].as_str().expect("name"), "John Doe");
        assert_eq!(jwt.state.claims["admin"].as_bool().expect("sub"), true);
        assert_eq!(jwt.state.claims["iat"].as_i64().expect("iat"), 1516239022);
    }

    #[test]
    fn jwe_direct_aes_256_gcm() {
        let claims = get_strongly_typed_claims();
        let key = crate::hash::HashAlgorithm::SHA2_256.digest(b"magic_password");
        let jwt = CheckedJwtEnc::new(JweAlg::Direct, JweEnc::Aes256Gcm, claims);
        let encoded = jwt.encode_direct(&key).unwrap();
        let decoded = JwtEnc::decode_direct(&encoded, &key)
            .unwrap()
            .validate::<MyClaims>(&NO_CHECK_VALIDATOR)
            .unwrap();
        assert_eq!(decoded.state.claims, get_strongly_typed_claims());
    }

    #[derive(Deserialize)]
    struct MyExpirableClaims {
        exp: i64,
        nbf: i64,
        msg: String,
    }

    #[test]
    fn decode_jws_not_expired() {
        let public_key = get_private_key_1().to_public_key();

        let jwt = JwtSig::decode(crate::test_files::JOSE_JWT_SIG_WITH_EXP, &public_key)
            .unwrap()
            .validate::<MyExpirableClaims>(&JwtValidator::strict(JwtDate::new(1545263999)))
            .expect("couldn't decode jwt without leeway");

        assert_eq!(jwt.state.claims.exp, 1545264000);
        assert_eq!(jwt.state.claims.nbf, 1545263000);
        assert_eq!(jwt.state.claims.msg, "THIS IS TIME SENSITIVE DATA");

        // alternatively, a leeway can account for small clock skew
        JwtSig::decode(crate::test_files::JOSE_JWT_SIG_WITH_EXP, &public_key)
            .unwrap()
            .validate::<MyExpirableClaims>(&JwtValidator::strict(JwtDate::new_with_leeway(1545264001, 10)))
            .expect("couldn't decode jwt with leeway for exp");

        JwtSig::decode(crate::test_files::JOSE_JWT_SIG_WITH_EXP, &public_key)
            .unwrap()
            .validate::<MyExpirableClaims>(&JwtValidator::strict(JwtDate::new_with_leeway(1545262999, 10)))
            .expect("couldn't decode jwt with leeway for nbf");
    }

    #[test]
    fn decode_jws_invalid_date_err() {
        let public_key = get_private_key_1().to_public_key();

        let err = JwtSig::decode(crate::test_files::JOSE_JWT_SIG_WITH_EXP, &public_key)
            .unwrap()
            .validate::<MyExpirableClaims>(&JwtValidator::strict(JwtDate::new(1545264001)))
            .err()
            .unwrap();

        assert_eq!(
            err.to_string(),
            "token expired (not after: 1545264000, now: 1545264001 [leeway: 0])"
        );

        let err = JwtSig::decode(crate::test_files::JOSE_JWT_SIG_WITH_EXP, &public_key)
            .unwrap()
            .validate::<MyExpirableClaims>(&JwtValidator::strict(JwtDate::new_with_leeway(1545262998, 1)))
            .err()
            .unwrap();

        assert_eq!(
            err.to_string(),
            "token not yet valid (not before: 1545263000, now: 1545262998 [leeway: 1])"
        );
    }

    #[test]
    fn decode_step_cli_generated_token() {
        #[derive(Deserialize)]
        struct SomeJetClaims {
            jet_ap: String,
            prx_usr: String,
            nbf: i64,
        }

        let jws = RawJws::decode("eyJhbGciOiJSUzI1NiIsImtpZCI6InUzQkF1b3lrZ21FY0F2Z21ydm5PVWxNZUYxN2JjS09EbGYweFdHcDhMY2MiLCJ0eXAiOiJKV1QifQ.eyJpYXQiOjE2NTkxMTg1NjMsImpldF9hcCI6InJkcCIsImp0aSI6IjY1YjkwZmQwMjM2YWU3Mjg1OWE1YWZlZTM3MTEzOTdjOWU4NTI1YzA4YzIyNjE4N2NlNjJjOWQwNTEzNDUzOTUiLCJuYmYiOjE2NTkxMTg1NjMsInByeF91c3IiOiJ1c2VybmFtZSJ9.MzULmkNyVY48nOgN7zbtN9q8Ni8JRavpkbw34aD-lMfqJzl5pFEJQPV9G1iM1HCbcMPRJfMDjVP31dAHOVtsu-gqGRx9qw1ogpNffcJI0nh5-VPPnqBbT5u8H2rJ7WeXO5kx4KAnD2Fbc45Nb6YEM-f_s9RyFipub0LI5AwiUHcbicJno0Lxz0dFKMiSA4cTNOe22vY7STf-E52LnsdHhnTt3JKDPP-7i5FzL1wOdBHzvxhRpyLqNU1kcSXrV_1L07XekeR6Kp3JoWaaJsIWm1Sk27W13Q575gS0a9OJgGX0bumq9fCneOJgLU8HrelUP8-qRM2IaGV81NRAr5HasQ")
            .map(RawJws::discard_signature)
            .map(JwtSig::from)
            .unwrap()
            .validate::<SomeJetClaims>(&JwtValidator::no_check())
            .unwrap();

        assert_eq!(jws.state.claims.jet_ap, "rdp");
        assert_eq!(jws.state.claims.prx_usr, "username");
        assert_eq!(jws.state.claims.nbf, 1659118563);
    }
}
