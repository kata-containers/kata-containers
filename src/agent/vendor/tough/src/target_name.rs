use crate::error::{self, Result};
use path_absolutize::Absolutize;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use snafu::{ensure, OptionExt, ResultExt};
use std::convert::TryFrom;
use std::path::PathBuf;
use std::str::FromStr;

/// Represents the name of a target in the repository. Path-like constructs are resolved (e.g.
/// `foo/../bar` becomes `bar`). Certain unsafe names are rejected when constructing a `TargetName`.
/// Unsafe names include:
/// - Anything that resolves to an empty string
/// - Anything that resolves to `/`
///
/// `TargetName` intentionally does not impl String-like traits so that we are forced to choose
/// between the resolved name and the raw/original name when we use it as a string.
///
/// Note that `Serialize` writes the `raw`, un-resolved name. You should not use the results of
/// serialization to form file paths.
///
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TargetName {
    /// The name assigned to the target by the repository user.
    raw: String,
    /// If the `raw` name is path-like, and it resolves to a simpler path construct, then the
    /// resolved name is stored here. (As a CPU optimization).
    resolved: Option<String>,
}

impl TargetName {
    /// Construct a new `TargetName`. Unsafe names will return an error.
    pub fn new<S: Into<String>>(raw: S) -> Result<Self> {
        let raw = raw.into();
        let resolved = clean_name(&raw)?;
        if raw == resolved {
            Ok(Self {
                raw,
                resolved: None,
            })
        } else {
            Ok(Self {
                raw,
                resolved: Some(resolved),
            })
        }
    }

    /// Get the original, unchanged name (i.e. which might be something like `foo/../bar` instead of
    /// `bar`).
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Get the resolved name (i.e. which would be `bar` instead of `foo/../bar`).
    pub fn resolved(&self) -> &str {
        match &self.resolved {
            None => self.raw(),
            Some(resolved) => resolved,
        }
    }
}

impl FromStr for TargetName {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::new(s)
    }
}

impl TryFrom<String> for TargetName {
    type Error = crate::error::Error;

    fn try_from(value: String) -> Result<Self> {
        TargetName::new(value)
    }
}

impl TryFrom<&str> for TargetName {
    type Error = crate::error::Error;

    fn try_from(value: &str) -> Result<Self> {
        TargetName::new(value)
    }
}

impl Serialize for TargetName {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.raw())
    }
}

impl<'de> Deserialize<'de> for TargetName {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <String>::deserialize(deserializer)?;
        TargetName::new(s).map_err(|e| D::Error::custom(format!("{}", e)))
    }
}

// Resolves path-like constructs. e.g. `foo/../bar` becomes `bar`.
fn clean_name(name: &str) -> Result<String> {
    // This causes something to panic, so we check for it early.
    ensure!(name != "..", error::UnsafeTargetNameDotDotSnafu);

    // Seems like bad things could happen if the target filename is the empty string.
    ensure!(!name.is_empty(), error::UnsafeTargetNameEmptySnafu { name });

    // If our name starts with absolute, then we need to remember this so we can restore it later.
    let name_path = PathBuf::from(name);
    let absolute = name_path.is_absolute();

    let clean = {
        let proposed = name_path
            .absolutize_from(&PathBuf::from("/"))
            .context(error::TargetNameResolveSnafu { name })?;

        // `absolutize_from` will give us a path that starts with `/`, so we remove it if the
        // original name did not start with `/`
        if absolute {
            // If `name` started with `/`, then we have nothing left to do because absolutize_from
            // returns a rooted path.
            proposed.to_path_buf()
        } else {
            let mut components = proposed.components();
            // If the original name did not start with `/`, we need to remove the leading slash
            // here because absolutize_from will return a rooted path.
            let first_component = components
                .next()
                // If this error occurs then there is a bug or behavior change in absolutize_from.
                .context(error::TargetNameComponentsEmptySnafu { name })?
                .as_os_str();

            // If the first component isn't `/` then there is a bug or behavior change in
            // absolutize_from.
            ensure!(
                first_component == "/",
                error::TargetNameRootMissingSnafu { name }
            );

            components.as_path().to_owned()
        }
    };

    let final_name = clean
        .as_os_str()
        .to_str()
        .context(error::PathUtf8Snafu { path: &clean })?
        .to_string();

    // Check again to make sure we didn't end up with an empty string.
    ensure!(
        !final_name.is_empty(),
        error::UnsafeTargetNameEmptySnafu { name }
    );

    ensure!(
        final_name != "/",
        error::UnsafeTargetNameSlashSnafu { name }
    );

    Ok(final_name)
}

#[test]
fn simple_1() {
    let name = "/absolute/path/is/ok.txt";
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn simple_2() {
    let name = "relative/path/is/ok.txt";
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn simple_3() {
    let name = "not-path-like.txt";
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_1() {
    let name = "/this/../is/ok.txt";
    let actual = clean_name(name).unwrap();
    let expected = "/is/ok.txt";
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_2() {
    let name = "../x";
    let actual = clean_name(name).unwrap();
    let expected = "x";
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_3() {
    let name = "../../x";
    let actual = clean_name(name).unwrap();
    let expected = "x";
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_4() {
    let name = "/../x";
    let actual = clean_name(name).unwrap();
    let expected = "/x";
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_5() {
    let name = "/../../x";
    let actual = clean_name(name).unwrap();
    let expected = "/x";
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_6() {
    let name = "/this/../../../../is/ok.txt";
    let actual = clean_name(name).unwrap();
    let expected = "/is/ok.txt";
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_7() {
    let name = "foo";
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn resolved_8() {
    let name = "/foo";
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn uncleaned_1() {
    let name = r#"~/\.\."#;
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn uncleaned_2() {
    let name = r#"funky\/\.\.\/name"#;
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn uncleaned_3() {
    let name = "/weird/\\..\\/path";
    let actual = clean_name(name).unwrap();
    let expected = name;
    assert_eq!(expected, &actual);
}

#[test]
fn bad_1() {
    let name = "..";
    let error = clean_name(name).err().unwrap();
    assert!(matches!(error, error::Error::UnsafeTargetNameDotDot { .. }));
}

#[test]
fn bad_2() {
    let name = "../";
    let error = clean_name(name).err().unwrap();
    assert!(matches!(error, error::Error::UnsafeTargetNameEmpty { .. }));
}

#[test]
fn bad_3() {
    let name = "/..";
    let error = clean_name(name).err().unwrap();
    assert!(matches!(error, error::Error::UnsafeTargetNameSlash { .. }));
}
