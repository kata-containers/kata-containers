pub type Result<T> = core::result::Result<T, Error>;

/// Error types
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    InvalidPaddingScheme,
    Decryption,
    Verification,
    MessageTooLong,
    InputNotHashed,
    NprimesTooSmall,
    TooFewPrimes,
    InvalidPrime,
    InvalidModulus,
    InvalidExponent,
    InvalidCoefficient,
    PublicExponentTooSmall,
    PublicExponentTooLarge,
    Pkcs1(pkcs1::Error),
    Pkcs8(pkcs8::Error),
    Internal,
    LabelTooLong,
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Error::InvalidPaddingScheme => write!(f, "invalid padding scheme"),
            Error::Decryption => write!(f, "decryption error"),
            Error::Verification => write!(f, "verification error"),
            Error::MessageTooLong => write!(f, "message too long"),
            Error::InputNotHashed => write!(f, "input must be hashed"),
            Error::NprimesTooSmall => write!(f, "nprimes must be >= 2"),
            Error::TooFewPrimes => {
                write!(f, "too few primes of given length to generate an RSA key")
            }
            Error::InvalidPrime => write!(f, "invalid prime value"),
            Error::InvalidModulus => write!(f, "invalid modulus"),
            Error::InvalidExponent => write!(f, "invalid exponent"),
            Error::InvalidCoefficient => write!(f, "invalid coefficient"),
            Error::PublicExponentTooSmall => write!(f, "public exponent too small"),
            Error::PublicExponentTooLarge => write!(f, "public exponent too large"),
            Error::Pkcs1(err) => write!(f, "{}", err),
            Error::Pkcs8(err) => write!(f, "{}", err),
            Error::Internal => write!(f, "internal error"),
            Error::LabelTooLong => write!(f, "label too long"),
        }
    }
}

impl From<pkcs1::Error> for Error {
    fn from(err: pkcs1::Error) -> Error {
        Error::Pkcs1(err)
    }
}

impl From<pkcs8::Error> for Error {
    fn from(err: pkcs8::Error) -> Error {
        Error::Pkcs8(err)
    }
}
