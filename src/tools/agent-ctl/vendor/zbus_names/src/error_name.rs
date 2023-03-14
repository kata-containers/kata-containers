use crate::{Error, Result};
use serde::{de, Deserialize, Serialize};
use static_assertions::assert_impl_all;
use std::{
    borrow::Borrow,
    convert::TryFrom,
    fmt::{self, Display, Formatter},
    ops::Deref,
};
use zvariant::{NoneValue, OwnedValue, Str, Type, Value};

/// String that identifies an [error name][en] on the bus.
///
/// Error names have same constraints as error names.
///
/// # Examples
///
/// ```
/// use core::convert::TryFrom;
/// use zbus_names::ErrorName;
///
/// // Valid error names.
/// let name = ErrorName::try_from("org.gnome.Error_for_you").unwrap();
/// assert_eq!(name, "org.gnome.Error_for_you");
/// let name = ErrorName::try_from("a.very.loooooooooooooooooo_ooooooo_0000o0ng.ErrorName").unwrap();
/// assert_eq!(name, "a.very.loooooooooooooooooo_ooooooo_0000o0ng.ErrorName");
///
/// // Invalid error names
/// ErrorName::try_from("").unwrap_err();
/// ErrorName::try_from(":start.with.a.colon").unwrap_err();
/// ErrorName::try_from("double..dots").unwrap_err();
/// ErrorName::try_from(".").unwrap_err();
/// ErrorName::try_from(".start.with.dot").unwrap_err();
/// ErrorName::try_from("no-dots").unwrap_err();
/// ErrorName::try_from("1st.element.starts.with.digit").unwrap_err();
/// ErrorName::try_from("the.2nd.element.starts.with.digit").unwrap_err();
/// ErrorName::try_from("contains.dashes-in.the.name").unwrap_err();
/// ```
///
/// [en]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-error
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct ErrorName<'name>(Str<'name>);

assert_impl_all!(ErrorName<'_>: Send, Sync, Unpin);

impl<'name> ErrorName<'name> {
    /// A borrowed clone (never allocates, unlike clone).
    pub fn as_ref(&self) -> ErrorName<'_> {
        ErrorName(self.0.as_ref())
    }

    /// The error name as string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a new `ErrorName` from the given string.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<&str>` implementation.
    pub fn from_str_unchecked(name: &'name str) -> Self {
        Self(Str::from(name))
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(name: &'static str) -> Result<Self> {
        ensure_correct_error_name(name)?;
        Ok(Self(Str::from_static(name)))
    }

    /// Same as `from_str_unchecked`, except it takes a `&'static str`.
    pub const fn from_static_str_unchecked(name: &'static str) -> Self {
        Self(Str::from_static(name))
    }

    /// Same as `from_str_unchecked`, except it takes an owned `String`.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<String>` implementation.
    pub fn from_string_unchecked(name: String) -> Self {
        Self(Str::from(name))
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> ErrorName<'static> {
        ErrorName(self.0.to_owned())
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> ErrorName<'static> {
        ErrorName(self.0.into_owned())
    }
}

impl Deref for ErrorName<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for ErrorName<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Display for ErrorName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_str(), f)
    }
}

impl PartialEq<str> for ErrorName<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for ErrorName<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<OwnedErrorName> for ErrorName<'_> {
    fn eq(&self, other: &OwnedErrorName) -> bool {
        *self == other.0
    }
}

impl<'de: 'name, 'name> Deserialize<'de> for ErrorName<'name> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = <&str>::deserialize(deserializer)?;

        Self::try_from(name).map_err(|e| de::Error::custom(e.to_string()))
    }
}

/// Try to create an `ErrorName` from a string.
impl<'s> TryFrom<&'s str> for ErrorName<'s> {
    type Error = Error;

    fn try_from(value: &'s str) -> Result<Self> {
        ensure_correct_error_name(value)?;

        Ok(Self::from_str_unchecked(value))
    }
}

impl TryFrom<String> for ErrorName<'_> {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        ensure_correct_error_name(&value)?;

        Ok(Self::from_string_unchecked(value))
    }
}

fn ensure_correct_error_name(name: &str) -> Result<()> {
    // Rules
    //
    // * Only ASCII alphanumeric or `_`.
    // * Must not begin with a `.`.
    // * Must contain at least one `.`.
    // * Each element must:
    //   * not begin with a digit.
    //   * be 1 character (so name must be minimum 3 characters long).
    // * <= 255 characters.
    if name.len() < 3 {
        return Err(Error::InvalidErrorName(format!(
            "`{}` is {} characters long, which is smaller than minimum allowed (3)",
            name,
            name.len(),
        )));
    } else if name.len() > 255 {
        return Err(Error::InvalidErrorName(format!(
            "`{}` is {} characters long, which is longer than maximum allowed (255)",
            name,
            name.len(),
        )));
    }

    let mut prev = None;
    let mut no_dot = true;
    for c in name.chars() {
        if c == '.' {
            if prev.is_none() || prev == Some('.') {
                return Err(Error::InvalidErrorName(String::from(
                    "must not contain a double `.`",
                )));
            }

            if no_dot {
                no_dot = false;
            }
        } else if c.is_ascii_digit() && (prev.is_none() || prev == Some('.')) {
            return Err(Error::InvalidErrorName(String::from(
                "each element must not start with a digit",
            )));
        } else if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(Error::InvalidErrorName(format!(
                "`{}` character not allowed",
                c
            )));
        }

        prev = Some(c);
    }

    if no_dot {
        return Err(Error::InvalidErrorName(String::from(
            "must contain at least 1 `.`",
        )));
    }

    Ok(())
}

/// This never succeeds but is provided so it's easier to pass `Option::None` values for API
/// requiring `Option<TryInto<impl BusName>>`, since type inference won't work here.
impl TryFrom<()> for ErrorName<'_> {
    type Error = Error;

    fn try_from(_value: ()) -> Result<Self> {
        unreachable!("Conversion from `()` is not meant to actually work");
    }
}

impl<'name> From<&ErrorName<'name>> for ErrorName<'name> {
    fn from(name: &ErrorName<'name>) -> Self {
        name.clone()
    }
}

impl<'name> NoneValue for ErrorName<'name> {
    type NoneType = &'name str;

    fn null_value() -> Self::NoneType {
        <&str>::default()
    }
}

/// Owned sibling of [`ErrorName`].
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct OwnedErrorName(#[serde(borrow)] ErrorName<'static>);

assert_impl_all!(OwnedErrorName: Send, Sync, Unpin);

impl OwnedErrorName {
    /// Convert to the inner `ErrorName`, consuming `self`.
    pub fn into_inner(self) -> ErrorName<'static> {
        self.0
    }
}

impl Deref for OwnedErrorName {
    type Target = ErrorName<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for OwnedErrorName {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl From<OwnedErrorName> for ErrorName<'static> {
    fn from(o: OwnedErrorName) -> Self {
        o.into_inner()
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedErrorName> for ErrorName<'unowned> {
    fn from(name: &'owned OwnedErrorName) -> Self {
        ErrorName::from_str_unchecked(name.as_str())
    }
}

impl From<ErrorName<'_>> for OwnedErrorName {
    fn from(name: ErrorName<'_>) -> Self {
        OwnedErrorName(name.into_owned())
    }
}

impl TryFrom<&'_ str> for OwnedErrorName {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Ok(Self::from(ErrorName::try_from(value)?))
    }
}

impl TryFrom<String> for OwnedErrorName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(Self::from(ErrorName::try_from(value)?))
    }
}

impl<'de> Deserialize<'de> for OwnedErrorName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(ErrorName::deserialize(deserializer)?.into())
    }
}

impl PartialEq<&str> for OwnedErrorName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<ErrorName<'_>> for OwnedErrorName {
    fn eq(&self, other: &ErrorName<'_>) -> bool {
        self.0 == *other
    }
}

impl Display for OwnedErrorName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ErrorName::from(self).fmt(f)
    }
}

impl NoneValue for OwnedErrorName {
    type NoneType = <ErrorName<'static> as NoneValue>::NoneType;

    fn null_value() -> Self::NoneType {
        ErrorName::null_value()
    }
}
