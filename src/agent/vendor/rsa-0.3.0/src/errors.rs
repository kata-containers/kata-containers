use thiserror::Error;

pub type Result<T> = ::std::result::Result<T, Error>;

/// Error types
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid padding scheme")]
    InvalidPaddingScheme,
    #[error("decryption error")]
    Decryption,
    #[error("verification error")]
    Verification,
    #[error("message too long")]
    MessageTooLong,
    #[error("input must be hashed")]
    InputNotHashed,
    #[error("nprimes must be >= 2")]
    NprimesTooSmall,
    #[error("too few primes of given length to generate an RSA key")]
    TooFewPrimes,
    #[error("invalid prime value")]
    InvalidPrime,
    #[error("invalid modulus")]
    InvalidModulus,
    #[error("invalid exponent")]
    InvalidExponent,
    #[error("invalid coefficient")]
    InvalidCoefficient,
    #[error("public exponent too small")]
    PublicExponentTooSmall,
    #[error("public exponent too large")]
    PublicExponentTooLarge,
    #[error("parse error: {}", reason)]
    ParseError { reason: String },
    #[error("internal error")]
    Internal,
    #[error("label too long")]
    LabelTooLong,
}
