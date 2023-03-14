use crate::{Error, Result};
use serde::{de, Deserialize, Serialize};
use static_assertions::assert_impl_all;
use std::{
    borrow::{Borrow, Cow},
    convert::TryFrom,
    fmt::{self, Display, Formatter},
    ops::Deref,
    sync::Arc,
};
use zvariant::{NoneValue, OwnedValue, Str, Type, Value};

/// String that identifies a [well-known bus name][wbn].
///
/// # Examples
///
/// ```
/// use core::convert::TryFrom;
/// use zbus_names::WellKnownName;
///
/// // Valid well-known names.
/// let name = WellKnownName::try_from("org.gnome.Service-for_you").unwrap();
/// assert_eq!(name, "org.gnome.Service-for_you");
/// let name = WellKnownName::try_from("a.very.loooooooooooooooooo-ooooooo_0000o0ng.Name").unwrap();
/// assert_eq!(name, "a.very.loooooooooooooooooo-ooooooo_0000o0ng.Name");
///
/// // Invalid well-known names
/// WellKnownName::try_from("").unwrap_err();
/// WellKnownName::try_from("double..dots").unwrap_err();
/// WellKnownName::try_from(".").unwrap_err();
/// WellKnownName::try_from(".start.with.dot").unwrap_err();
/// WellKnownName::try_from("1st.element.starts.with.digit").unwrap_err();
/// WellKnownName::try_from("the.2nd.element.starts.with.digit").unwrap_err();
/// WellKnownName::try_from("no-dots").unwrap_err();
/// ```
///
/// [wbn]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct WellKnownName<'name>(Str<'name>);

assert_impl_all!(WellKnownName<'_>: Send, Sync, Unpin);

impl<'name> WellKnownName<'name> {
    /// A borrowed clone (never allocates, unlike clone).
    pub fn as_ref(&self) -> WellKnownName<'_> {
        WellKnownName(self.0.as_ref())
    }

    /// The well-known-name as string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a new `WellKnownName` from the given string.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<&str>` implementation.
    pub fn from_str_unchecked(name: &'name str) -> Self {
        Self(Str::from(name))
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(name: &'static str) -> Result<Self> {
        ensure_correct_well_known_name(name)?;
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
    pub fn to_owned(&self) -> WellKnownName<'static> {
        WellKnownName(self.0.to_owned())
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> WellKnownName<'static> {
        WellKnownName(self.0.into_owned())
    }
}

impl Deref for WellKnownName<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for WellKnownName<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Display for WellKnownName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_str(), f)
    }
}

impl<'a> PartialEq<str> for WellKnownName<'a> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<'a> PartialEq<&str> for WellKnownName<'a> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<OwnedWellKnownName> for WellKnownName<'_> {
    fn eq(&self, other: &OwnedWellKnownName) -> bool {
        *self == other.0
    }
}

impl<'de: 'name, 'name> Deserialize<'de> for WellKnownName<'name> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = <Cow<'name, str>>::deserialize(deserializer)?;

        Self::try_from(name).map_err(|e| de::Error::custom(e.to_string()))
    }
}

/// Try to create an `WellKnownName` from a string.
impl<'name> TryFrom<&'name str> for WellKnownName<'name> {
    type Error = Error;

    fn try_from(value: &'name str) -> Result<Self> {
        ensure_correct_well_known_name(value)?;

        Ok(Self::from_str_unchecked(value))
    }
}

impl TryFrom<String> for WellKnownName<'_> {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        ensure_correct_well_known_name(&value)?;

        Ok(Self::from_string_unchecked(value))
    }
}

impl TryFrom<Arc<str>> for WellKnownName<'_> {
    type Error = Error;

    fn try_from(value: Arc<str>) -> Result<Self> {
        ensure_correct_well_known_name(&value)?;

        Ok(Self(Str::from(value)))
    }
}

impl<'name> TryFrom<Cow<'name, str>> for WellKnownName<'name> {
    type Error = Error;

    fn try_from(value: Cow<'name, str>) -> Result<Self> {
        match value {
            Cow::Borrowed(s) => Self::try_from(s),
            Cow::Owned(s) => Self::try_from(s),
        }
    }
}

fn ensure_correct_well_known_name(name: &str) -> Result<()> {
    // Rules
    //
    // * Only ASCII alphanumeric, `_` or '-'.
    // * Must not begin with a `.`.
    // * Must contain at least one `.`.
    // * Each element must:
    //  * not begin with a digit.
    //  * be 1 character (so name must be minimum 3 characters long).
    // * <= 255 characters.
    if name.is_empty() {
        return Err(Error::InvalidWellKnownName(String::from(
            "must contain at least 3 characters",
        )));
    } else if name.len() < 3 {
        return Err(Error::InvalidWellKnownName(format!(
            "`{}` is {} characters long, which is smaller than minimum allowed (3)",
            name,
            name.len(),
        )));
    } else if name.len() > 255 {
        return Err(Error::InvalidWellKnownName(format!(
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
                return Err(Error::InvalidWellKnownName(String::from(
                    "must not contain a double `.`",
                )));
            }

            if no_dot {
                no_dot = false;
            }
        } else if c.is_ascii_digit() && (prev.is_none() || prev == Some('.')) {
            return Err(Error::InvalidWellKnownName(String::from(
                "each element must not start with a digit",
            )));
        } else if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
            return Err(Error::InvalidWellKnownName(format!(
                "`{}` character not allowed",
                c
            )));
        }

        prev = Some(c);
    }

    if no_dot {
        return Err(Error::InvalidWellKnownName(String::from(
            "must contain at least 1 `.`",
        )));
    }

    Ok(())
}

/// This never succeeds but is provided so it's easier to pass `Option::None` values for API
/// requiring `Option<TryInto<impl BusName>>`, since type inference won't work here.
impl TryFrom<()> for WellKnownName<'_> {
    type Error = Error;

    fn try_from(_value: ()) -> Result<Self> {
        unreachable!("Conversion from `()` is not meant to actually work");
    }
}

impl<'name> From<&WellKnownName<'name>> for WellKnownName<'name> {
    fn from(name: &WellKnownName<'name>) -> Self {
        name.clone()
    }
}

impl<'name> NoneValue for WellKnownName<'name> {
    type NoneType = &'name str;

    fn null_value() -> Self::NoneType {
        <&str>::default()
    }
}

/// Owned sibling of [`WellKnownName`].
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct OwnedWellKnownName(#[serde(borrow)] WellKnownName<'static>);

assert_impl_all!(OwnedWellKnownName: Send, Sync, Unpin);

impl OwnedWellKnownName {
    /// Convert to the inner `WellKnownName`, consuming `self`.
    pub fn into_inner(self) -> WellKnownName<'static> {
        self.0
    }

    /// Get a reference to the inner `WellKnownName`.
    pub fn inner(&self) -> &WellKnownName<'static> {
        &self.0
    }
}

impl Deref for OwnedWellKnownName {
    type Target = WellKnownName<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for OwnedWellKnownName {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for OwnedWellKnownName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for OwnedWellKnownName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        WellKnownName::from(self).fmt(f)
    }
}

impl From<OwnedWellKnownName> for WellKnownName<'static> {
    fn from(name: OwnedWellKnownName) -> Self {
        name.into_inner()
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedWellKnownName> for WellKnownName<'unowned> {
    fn from(name: &'owned OwnedWellKnownName) -> Self {
        WellKnownName::from_str_unchecked(name.as_str())
    }
}

impl From<WellKnownName<'_>> for OwnedWellKnownName {
    fn from(name: WellKnownName<'_>) -> Self {
        OwnedWellKnownName(name.into_owned())
    }
}

impl TryFrom<&'_ str> for OwnedWellKnownName {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Ok(Self::from(WellKnownName::try_from(value)?))
    }
}

impl TryFrom<String> for OwnedWellKnownName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(Self::from(WellKnownName::try_from(value)?))
    }
}

impl TryFrom<Arc<str>> for OwnedWellKnownName {
    type Error = Error;

    fn try_from(value: Arc<str>) -> Result<Self> {
        Ok(Self::from(WellKnownName::try_from(value)?))
    }
}

impl<'de> Deserialize<'de> for OwnedWellKnownName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|n| WellKnownName::try_from(n).map_err(|e| de::Error::custom(e.to_string())))
            .map(Self)
    }
}

impl PartialEq<&str> for OwnedWellKnownName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<WellKnownName<'_>> for OwnedWellKnownName {
    fn eq(&self, other: &WellKnownName<'_>) -> bool {
        self.0 == *other
    }
}

impl NoneValue for OwnedWellKnownName {
    type NoneType = <WellKnownName<'static> as NoneValue>::NoneType;

    fn null_value() -> Self::NoneType {
        WellKnownName::null_value()
    }
}
