use std::{
    convert::{TryFrom, TryInto},
    fmt,
    iter::repeat_with,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use static_assertions::assert_impl_all;

/// A D-Bus server GUID.
///
/// See the D-Bus specification [UUIDs chapter] for details.
///
/// You can create a `Guid` from an existing string with [`Guid::try_from::<&str>`][TryFrom].
///
/// [UUIDs chapter]: https://dbus.freedesktop.org/doc/dbus-specification.html#uuids
/// [TryFrom]: #impl-TryFrom%3C%26%27_%20str%3E
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Guid(String);

assert_impl_all!(Guid: Send, Sync, Unpin);

impl Guid {
    /// Generate a D-Bus GUID that can be used with e.g. [`Connection::new_unix_server`].
    ///
    /// [`Connection::new_unix_server`]: struct.Connection.html#method.new_unix_server
    pub fn generate() -> Self {
        let r: Vec<u32> = repeat_with(rand::random::<u32>).take(3).collect();
        let r3 = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(n) => n.as_secs() as u32,
            Err(_) => rand::random::<u32>(),
        };

        let s = format!("{:08x}{:08x}{:08x}{:08x}", r[0], r[1], r[2], r3);
        Self(s)
    }

    /// Returns a string slice for the GUID.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<&str> for Guid {
    type Error = crate::Error;

    /// Creates a GUID from a string with 32 hex digits.
    ///
    /// Returns `Err(`[`Error::InvalidGUID`]`)` if the provided string is not a well-formed GUID.
    ///
    /// [`Error::InvalidGUID`]: enum.Error.html#variant.InvalidGUID
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        if value.as_bytes().len() != 32 || !value.chars().all(|c| char::is_ascii_hexdigit(&c)) {
            Err(crate::Error::InvalidGUID)
        } else {
            Ok(Guid(value.to_string()))
        }
    }
}

impl FromStr for Guid {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.try_into()
    }
}

#[cfg(test)]
mod tests {
    use crate::Guid;
    use test_log::test;

    #[test]
    fn generate() {
        let u1 = Guid::generate();
        let u2 = Guid::generate();
        assert_eq!(u1.as_str().len(), 32);
        assert_eq!(u2.as_str().len(), 32);
        assert_ne!(u1, u2);
        assert_ne!(u1.as_str(), u2.as_str());
    }
}
