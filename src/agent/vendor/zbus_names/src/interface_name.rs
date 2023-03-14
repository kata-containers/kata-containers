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

/// String that identifies an [interface name][in] on the bus.
///
/// # Examples
///
/// ```
/// use core::convert::TryFrom;
/// use zbus_names::InterfaceName;
///
/// // Valid interface names.
/// let name = InterfaceName::try_from("org.gnome.Interface_for_you").unwrap();
/// assert_eq!(name, "org.gnome.Interface_for_you");
/// let name = InterfaceName::try_from("a.very.loooooooooooooooooo_ooooooo_0000o0ng.Name").unwrap();
/// assert_eq!(name, "a.very.loooooooooooooooooo_ooooooo_0000o0ng.Name");
///
/// // Invalid interface names
/// InterfaceName::try_from("").unwrap_err();
/// InterfaceName::try_from(":start.with.a.colon").unwrap_err();
/// InterfaceName::try_from("double..dots").unwrap_err();
/// InterfaceName::try_from(".").unwrap_err();
/// InterfaceName::try_from(".start.with.dot").unwrap_err();
/// InterfaceName::try_from("no-dots").unwrap_err();
/// InterfaceName::try_from("1st.element.starts.with.digit").unwrap_err();
/// InterfaceName::try_from("the.2nd.element.starts.with.digit").unwrap_err();
/// InterfaceName::try_from("contains.dashes-in.the.name").unwrap_err();
/// ```
///
/// [in]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-interface
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct InterfaceName<'name>(Str<'name>);

assert_impl_all!(InterfaceName<'_>: Send, Sync, Unpin);

impl<'name> InterfaceName<'name> {
    /// A borrowed clone (never allocates, unlike clone).
    pub fn as_ref(&self) -> InterfaceName<'_> {
        InterfaceName(self.0.as_ref())
    }

    /// The interface name as string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Create a new `InterfaceName` from the given string.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<&str>` implementation.
    pub fn from_str_unchecked(name: &'name str) -> Self {
        Self(Str::from(name))
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(name: &'static str) -> Result<Self> {
        ensure_correct_interface_name(name)?;
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
    pub fn to_owned(&self) -> InterfaceName<'static> {
        InterfaceName(self.0.to_owned())
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> InterfaceName<'static> {
        InterfaceName(self.0.into_owned())
    }
}

impl Deref for InterfaceName<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Borrow<str> for InterfaceName<'_> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Display for InterfaceName<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_str(), f)
    }
}

impl PartialEq<str> for InterfaceName<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for InterfaceName<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<OwnedInterfaceName> for InterfaceName<'_> {
    fn eq(&self, other: &OwnedInterfaceName) -> bool {
        *self == other.0
    }
}

impl<'de: 'name, 'name> Deserialize<'de> for InterfaceName<'name> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = <Cow<'name, str>>::deserialize(deserializer)?;

        Self::try_from(name).map_err(|e| de::Error::custom(e.to_string()))
    }
}

/// Try to create an `InterfaceName` from a string.
impl<'s> TryFrom<&'s str> for InterfaceName<'s> {
    type Error = Error;

    fn try_from(value: &'s str) -> Result<Self> {
        ensure_correct_interface_name(value)?;

        Ok(Self::from_str_unchecked(value))
    }
}

impl TryFrom<String> for InterfaceName<'_> {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        ensure_correct_interface_name(&value)?;

        Ok(Self::from_string_unchecked(value))
    }
}

impl TryFrom<Arc<str>> for InterfaceName<'_> {
    type Error = Error;

    fn try_from(value: Arc<str>) -> Result<Self> {
        ensure_correct_interface_name(&value)?;

        Ok(Self(Str::from(value)))
    }
}

impl<'name> TryFrom<Cow<'name, str>> for InterfaceName<'name> {
    type Error = Error;

    fn try_from(value: Cow<'name, str>) -> Result<Self> {
        match value {
            Cow::Borrowed(s) => Self::try_from(s),
            Cow::Owned(s) => Self::try_from(s),
        }
    }
}

fn ensure_correct_interface_name(name: &str) -> Result<()> {
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
        return Err(Error::InvalidInterfaceName(format!(
            "`{}` is {} characters long, which is smaller than minimum allowed (3)",
            name,
            name.len(),
        )));
    } else if name.len() > 255 {
        return Err(Error::InvalidInterfaceName(format!(
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
                return Err(Error::InvalidInterfaceName(String::from(
                    "must not contain a double `.`",
                )));
            }

            if no_dot {
                no_dot = false;
            }
        } else if c.is_ascii_digit() && (prev.is_none() || prev == Some('.')) {
            return Err(Error::InvalidInterfaceName(String::from(
                "each element must not start with a digit",
            )));
        } else if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(Error::InvalidInterfaceName(format!(
                "`{}` character not allowed",
                c
            )));
        }

        prev = Some(c);
    }

    if no_dot {
        return Err(Error::InvalidInterfaceName(String::from(
            "must contain at least 1 `.`",
        )));
    }

    Ok(())
}

/// This never succeeds but is provided so it's easier to pass `Option::None` values for API
/// requiring `Option<TryInto<impl BusName>>`, since type inference won't work here.
impl TryFrom<()> for InterfaceName<'_> {
    type Error = Error;

    fn try_from(_value: ()) -> Result<Self> {
        unreachable!("Conversion from `()` is not meant to actually work");
    }
}

impl<'name> From<&InterfaceName<'name>> for InterfaceName<'name> {
    fn from(name: &InterfaceName<'name>) -> Self {
        name.clone()
    }
}

impl<'name> NoneValue for InterfaceName<'name> {
    type NoneType = &'name str;

    fn null_value() -> Self::NoneType {
        <&str>::default()
    }
}

/// Owned sibling of [`InterfaceName`].
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Serialize, Type, Value, PartialOrd, Ord, OwnedValue,
)]
pub struct OwnedInterfaceName(#[serde(borrow)] InterfaceName<'static>);

assert_impl_all!(OwnedInterfaceName: Send, Sync, Unpin);

impl OwnedInterfaceName {
    /// Convert to the inner `InterfaceName`, consuming `self`.
    pub fn into_inner(self) -> InterfaceName<'static> {
        self.0
    }

    /// Get a reference to the inner `InterfaceName`.
    pub fn inner(&self) -> &InterfaceName<'static> {
        &self.0
    }
}

impl Deref for OwnedInterfaceName {
    type Target = InterfaceName<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for OwnedInterfaceName {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl From<OwnedInterfaceName> for InterfaceName<'static> {
    fn from(o: OwnedInterfaceName) -> Self {
        o.into_inner()
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedInterfaceName> for InterfaceName<'unowned> {
    fn from(name: &'owned OwnedInterfaceName) -> Self {
        InterfaceName::from_str_unchecked(name.as_str())
    }
}

impl From<InterfaceName<'_>> for OwnedInterfaceName {
    fn from(name: InterfaceName<'_>) -> Self {
        OwnedInterfaceName(name.into_owned())
    }
}

impl TryFrom<&'_ str> for OwnedInterfaceName {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Ok(Self::from(InterfaceName::try_from(value)?))
    }
}

impl TryFrom<String> for OwnedInterfaceName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(Self::from(InterfaceName::try_from(value)?))
    }
}

impl TryFrom<Arc<str>> for OwnedInterfaceName {
    type Error = Error;

    fn try_from(value: Arc<str>) -> Result<Self> {
        Ok(Self::from(InterfaceName::try_from(value)?))
    }
}

impl<'de> Deserialize<'de> for OwnedInterfaceName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|n| InterfaceName::try_from(n).map_err(|e| de::Error::custom(e.to_string())))
            .map(Self)
    }
}

impl PartialEq<&str> for OwnedInterfaceName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<InterfaceName<'_>> for OwnedInterfaceName {
    fn eq(&self, other: &InterfaceName<'_>) -> bool {
        self.0 == *other
    }
}

impl Display for OwnedInterfaceName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        InterfaceName::from(self).fmt(f)
    }
}

impl NoneValue for OwnedInterfaceName {
    type NoneType = <InterfaceName<'static> as NoneValue>::NoneType;

    fn null_value() -> Self::NoneType {
        InterfaceName::null_value()
    }
}
