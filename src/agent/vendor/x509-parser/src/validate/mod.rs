mod certificate;
mod extensions;
mod loggers;
mod name;
mod structure;
use std::marker::PhantomData;

pub use certificate::*;
pub use extensions::*;
pub use loggers::*;
pub use name::*;
pub use structure::*;

/// Trait for validating item (for ex. validate X.509 structure)
///
/// # Examples
///
/// Using callbacks:
///
/// ```
/// use x509_parser::certificate::X509Certificate;
/// # #[allow(deprecated)]
/// use x509_parser::validate::Validate;
/// # #[allow(deprecated)]
/// #[cfg(feature = "validate")]
/// fn validate_certificate(x509: &X509Certificate<'_>) -> Result<(), &'static str> {
///     println!("  Subject: {}", x509.subject());
///     // validate and print warnings and errors to stderr
///     let ok = x509.validate(
///         |msg| {
///             eprintln!("  [W] {}", msg);
///         },
///         |msg| {
///             eprintln!("  [E] {}", msg);
///         },
///     );
///     print!("Structure validation status: ");
///     if ok {
///         println!("Ok");
///         Ok(())
///     } else {
///         println!("FAIL");
///         Err("validation failed")
///     }
/// }
/// ```
///
/// Collecting warnings and errors to `Vec`:
///
/// ```
/// use x509_parser::certificate::X509Certificate;
/// # #[allow(deprecated)]
/// use x509_parser::validate::Validate;
///
/// # #[allow(deprecated)]
/// #[cfg(feature = "validate")]
/// fn validate_certificate(x509: &X509Certificate<'_>) -> Result<(), &'static str> {
///     println!("  Subject: {}", x509.subject());
///     // validate and print warnings and errors to stderr
///     let (ok, warnings, errors) = x509.validate_to_vec();
///     print!("Structure validation status: ");
///     if ok {
///         println!("Ok");
///     } else {
///         println!("FAIL");
///     }
///     for warning in &warnings {
///         eprintln!("  [W] {}", warning);
///     }
///     for error in &errors {
///         eprintln!("  [E] {}", error);
///     }
///     println!();
///     if !errors.is_empty() {
///         return Err("validation failed");
///     }
///     Ok(())
/// }
/// ```
#[deprecated(since = "0.13.0", note = "please use `X509StructureValidator` instead")]
pub trait Validate {
    /// Attempts to validate current item.
    ///
    /// Returns `true` if item was validated.
    ///
    /// Call `warn()` if a non-fatal error was encountered, and `err()`
    /// if the error is fatal. These fucntions receive a description of the error.
    fn validate<W, E>(&self, warn: W, err: E) -> bool
    where
        W: FnMut(&str),
        E: FnMut(&str);

    /// Attempts to validate current item, storing warning and errors in `Vec`.
    ///
    /// Returns the validation result (`true` if validated), the list of warnings,
    /// and the list of errors.
    fn validate_to_vec(&self) -> (bool, Vec<String>, Vec<String>) {
        let mut warn_list = Vec::new();
        let mut err_list = Vec::new();
        let res = self.validate(
            |s| warn_list.push(s.to_owned()),
            |s| err_list.push(s.to_owned()),
        );
        (res, warn_list, err_list)
    }
}

/// Trait for build item validators (for ex. validate X.509 structure)
///
/// See [`X509StructureValidator`] for a default implementation, validating the
/// DER structure of a X.509 Certificate.
///
/// See implementors of the [`Logger`] trait for methods to collect or handle warnings and errors.
///
/// # Examples
///
/// Collecting warnings and errors to `Vec`:
///
/// ```
/// use x509_parser::certificate::X509Certificate;
/// use x509_parser::validate::*;
///
/// # #[allow(deprecated)]
/// #[cfg(feature = "validate")]
/// fn validate_certificate(x509: &X509Certificate<'_>) -> Result<(), &'static str> {
///     let mut logger = VecLogger::default();
///     println!("  Subject: {}", x509.subject());
///     // validate and print warnings and errors to stderr
///     let ok = X509StructureValidator.validate(&x509, &mut logger);
///     print!("Structure validation status: ");
///     if ok {
///         println!("Ok");
///     } else {
///         println!("FAIL");
///     }
///     for warning in logger.warnings() {
///         eprintln!("  [W] {}", warning);
///     }
///     for error in logger.errors() {
///         eprintln!("  [E] {}", error);
///     }
///     println!();
///     if !logger.errors().is_empty() {
///         return Err("validation failed");
///     }
///     Ok(())
/// }
/// ```
pub trait Validator<'a> {
    /// The item to validate
    type Item;

    /// Attempts to validate current item.
    ///
    /// Returns `true` if item was validated.
    ///
    /// Call `l.warn()` if a non-fatal error was encountered, and `l.err()`
    /// if the error is fatal. These functions receive a description of the error.
    fn validate<L: Logger>(&self, item: &'a Self::Item, l: &'_ mut L) -> bool;

    fn chain<V2>(self, v2: V2) -> ChainValidator<'a, Self, V2, Self::Item>
    where
        Self: Sized,
        V2: Validator<'a, Item = Self::Item>,
    {
        ChainValidator {
            v1: self,
            v2,
            _p: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct ChainValidator<'a, A, B, I>
where
    A: Validator<'a, Item = I>,
    B: Validator<'a, Item = I>,
{
    v1: A,
    v2: B,
    _p: PhantomData<&'a ()>,
}

impl<'a, A, B, I> Validator<'a> for ChainValidator<'a, A, B, I>
where
    A: Validator<'a, Item = I>,
    B: Validator<'a, Item = I>,
{
    type Item = I;

    fn validate<L: Logger>(&'_ self, item: &'a Self::Item, l: &'_ mut L) -> bool {
        self.v1.validate(item, l) & self.v2.validate(item, l)
    }
}

#[allow(deprecated)]
#[cfg(test)]
mod tests {
    use crate::validate::*;

    struct V1 {
        a: u32,
    }

    impl Validate for V1 {
        fn validate<W, E>(&self, mut warn: W, _err: E) -> bool
        where
            W: FnMut(&str),
            E: FnMut(&str),
        {
            if self.a > 10 {
                warn("a is greater than 10");
            }
            true
        }
    }

    struct V1Validator;

    impl<'a> Validator<'a> for V1Validator {
        type Item = V1;

        fn validate<L: Logger>(&self, item: &'a Self::Item, l: &'_ mut L) -> bool {
            if item.a > 10 {
                l.warn("a is greater than 10");
            }
            true
        }
    }

    #[test]
    fn validate_warn() {
        let v1 = V1 { a: 1 };
        let (res, warn, err) = v1.validate_to_vec();
        assert!(res);
        assert!(warn.is_empty());
        assert!(err.is_empty());
        // same, with one warning
        let v20 = V1 { a: 20 };
        let (res, warn, err) = v20.validate_to_vec();
        assert!(res);
        assert_eq!(warn, vec!["a is greater than 10".to_string()]);
        assert!(err.is_empty());
    }

    #[test]
    fn validator_warn() {
        let mut logger = VecLogger::default();
        let v1 = V1 { a: 1 };
        let res = V1Validator.validate(&v1, &mut logger);
        assert!(res);
        assert!(logger.warnings().is_empty());
        assert!(logger.errors().is_empty());
        // same, with one warning
        let v20 = V1 { a: 20 };
        let res = V1Validator.validate(&v20, &mut logger);
        assert!(res);
        assert_eq!(logger.warnings(), &["a is greater than 10".to_string()]);
        assert!(logger.errors().is_empty());
    }
}
