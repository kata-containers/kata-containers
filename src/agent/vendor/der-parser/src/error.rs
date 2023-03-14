//! Error type for BER/DER parsers

use crate::ber::BerObject;
use crate::der::DerObject;
use nom::IResult;

pub use asn1_rs::{DerConstraint, Error};

pub type BerError = Error;

// pub use asn1_rs::Result;

/// Holds the result of parsing functions
///
/// `O` is the output type, and defaults to a `BerObject`.
///
/// Note that this type is also a `Result`, so usual functions (`map`, `unwrap` etc.) are available.
///
/// This type is a wrapper around nom's IResult type
pub type BerResult<'a, O = BerObject<'a>> = IResult<&'a [u8], O, BerError>;

/// Holds the result of parsing functions (DER)
///
/// Note that this type is also a `Result`, so usual functions (`map`, `unwrap` etc.) are available.
pub type DerResult<'a> = BerResult<'a, DerObject<'a>>;

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use std::boxed::Box;
    use std::error::Error;

    #[test]
    fn test_unwrap_bererror() {
        let e = BerError::IntegerTooLarge;
        // println!("{}", e);
        let _: Result<(), Box<dyn Error>> = Err(Box::new(e));
    }
}
