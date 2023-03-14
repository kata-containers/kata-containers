//! X.509 errors

use der_parser::error::BerError;
use nom::error::{ErrorKind, ParseError};
use nom::IResult;

/// An error that can occur while converting an OID to a Nid.
#[derive(Debug, PartialEq)]
pub struct NidError;

/// Holds the result of parsing functions (X.509)
///
/// Note that this type is also a `Result`, so usual functions (`map`, `unwrap` etc.) are available.
pub type X509Result<'a, T> = IResult<&'a [u8], T, X509Error>;

/// An error that can occur while parsing or validating a certificate.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum X509Error {
    #[error("generic error")]
    Generic,

    #[error("invalid version")]
    InvalidVersion,
    #[error("invalid serial")]
    InvalidSerial,
    #[error("invalid algorithm identifier")]
    InvalidAlgorithmIdentifier,
    #[error("invalid X.509 name")]
    InvalidX509Name,
    #[error("invalid date")]
    InvalidDate,
    #[error("invalid X.509 Subject Public Key Info")]
    InvalidSPKI,
    #[error("invalid X.509 Subject Unique ID")]
    InvalidSubjectUID,
    #[error("invalid X.509 Issuer Unique ID")]
    InvalidIssuerUID,
    #[error("invalid extensions")]
    InvalidExtensions,
    #[error("invalid attributes")]
    InvalidAttributes,
    #[error("duplicate extensions")]
    DuplicateExtensions,
    #[error("duplicate attributes")]
    DuplicateAttributes,
    #[error("invalid Signature DER Value")]
    InvalidSignatureValue,
    #[error("invalid TBS certificate")]
    InvalidTbsCertificate,

    // error types from CRL
    #[error("invalid User certificate")]
    InvalidUserCertificate,

    /// Top-level certificate structure is invalid
    #[error("invalid certificate")]
    InvalidCertificate,

    #[error("signature verification error")]
    SignatureVerificationError,
    #[error("signature unsupported algorithm")]
    SignatureUnsupportedAlgorithm,

    #[error("invalid number")]
    InvalidNumber,

    #[error("BER error: {0}")]
    Der(#[from] BerError),
    #[error("nom error: {0:?}")]
    NomError(ErrorKind),
}

impl From<nom::Err<BerError>> for X509Error {
    fn from(e: nom::Err<BerError>) -> Self {
        Self::Der(BerError::from(e))
    }
}

impl From<nom::Err<X509Error>> for X509Error {
    fn from(e: nom::Err<X509Error>) -> Self {
        match e {
            nom::Err::Error(e) | nom::Err::Failure(e) => e,
            nom::Err::Incomplete(i) => Self::Der(BerError::Incomplete(i)),
        }
    }
}

impl From<X509Error> for nom::Err<X509Error> {
    fn from(e: X509Error) -> nom::Err<X509Error> {
        nom::Err::Error(e)
    }
}

impl From<ErrorKind> for X509Error {
    fn from(e: ErrorKind) -> X509Error {
        X509Error::NomError(e)
    }
}

impl<I> ParseError<I> for X509Error {
    fn from_error_kind(_input: I, kind: ErrorKind) -> Self {
        X509Error::NomError(kind)
    }
    fn append(_input: I, kind: ErrorKind, _other: Self) -> Self {
        X509Error::NomError(kind)
    }
}

/// An error that can occur while parsing or validating a certificate.
#[derive(Debug, thiserror::Error)]
pub enum PEMError {
    #[error("base64 decode error")]
    Base64DecodeError,
    #[error("incomplete PEM")]
    IncompletePEM,
    #[error("invalid header")]
    InvalidHeader,
    #[error("missing header")]
    MissingHeader,

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),
}
