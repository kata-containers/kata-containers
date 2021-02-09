use alloc::string::String;

#[cfg(windows)]
const FILE_SEPARATOR: char = '\\';

#[cfg(not(windows))]
const FILE_SEPARATOR: char = '/';

/// Delete an ending FILE_SEPARATOR in a string except for the FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// if cfg!(windows) {
///     assert_eq!("path", slash_formatter::delete_end_file_separator(r"path\"));
/// } else {
///     assert_eq!("path", slash_formatter::delete_end_file_separator("path/"));
/// }
/// ```
#[inline]
pub fn delete_end_file_separator<'a, S: ?Sized + AsRef<str> + 'a>(s: &'a S) -> &'a str {
    let s = s.as_ref();

    let length = s.len();

    if length > 1 && s.ends_with(FILE_SEPARATOR) {
        &s[..length - 1]
    } else {
        s
    }
}

/// Delete an ending FILE_SEPARATOR in a string except for the FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// let s = if cfg!(windows) {
///     String::from(r"path\")
/// } else {
///     String::from("path/")
/// };
///
/// let s = slash_formatter::delete_end_file_separator_owned(s);
///
/// assert_eq!("path", s);
/// ```
#[inline]
pub fn delete_end_file_separator_owned<S: Into<String>>(s: S) -> String {
    let mut s = s.into();

    let length = s.len();

    if length > 1 && s.ends_with(FILE_SEPARATOR) {
        s.remove(length - 1);
    }

    s
}

/// Delete an ending FILE_SEPARATOR in a string except for the FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// let mut s = if cfg!(windows) {
///     String::from(r"path\")
/// } else {
///     String::from("path/")
/// };
///
/// slash_formatter::delete_end_file_separator_mut(&mut s);
///
/// assert_eq!("path", s);
/// ```
#[inline]
pub fn delete_end_file_separator_mut(s: &mut String) {
    let length = s.len();

    if length > 1 && s.ends_with(FILE_SEPARATOR) {
        s.remove(length - 1);
    }
}

/// Delete a starting FILE_SEPARATOR in a string except for the FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// if cfg!(windows) {
///     assert_eq!("path", slash_formatter::delete_start_file_separator(r"\path"));
/// } else {
///     assert_eq!("path", slash_formatter::delete_start_file_separator("/path"));
/// }
/// ```
#[inline]
pub fn delete_start_file_separator<'a, S: ?Sized + AsRef<str> + 'a>(s: &'a S) -> &'a str {
    let s = s.as_ref();

    let length = s.len();

    if length > 1 && s.starts_with(FILE_SEPARATOR) {
        &s[1..]
    } else {
        s
    }
}

/// Delete a starting FILE_SEPARATOR in a string except for the FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// let s = if cfg!(windows) {
///     String::from(r"\path")
/// } else {
///     String::from("/path")
/// };
///
/// let s = slash_formatter::delete_start_file_separator_owned(s);
///
/// assert_eq!("path", s);
/// ```
#[inline]
pub fn delete_start_file_separator_owned<S: Into<String>>(s: S) -> String {
    let mut s = s.into();

    let length = s.len();

    if length > 1 && s.starts_with(FILE_SEPARATOR) {
        s.remove(0);
    }

    s
}

/// Delete a starting FILE_SEPARATOR in a string except for the FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// let mut s = if cfg!(windows) {
///     String::from(r"\path")
/// } else {
///     String::from("/path")
/// };
///
/// slash_formatter::delete_start_file_separator_mut(&mut s);
///
/// assert_eq!("path", s);
/// ```
#[inline]
pub fn delete_start_file_separator_mut(s: &mut String) {
    let length = s.len();

    if length > 1 && s.starts_with(FILE_SEPARATOR) {
        s.remove(0);
    }
}

/// Add a starting FILE_SEPARATOR into a string.
///
/// ```
/// extern crate slash_formatter;
///
/// if cfg!(windows) {
///     assert_eq!(r"\path", slash_formatter::add_start_file_separator("path"));
/// } else {
///     assert_eq!("/path", slash_formatter::add_start_file_separator("path"));
/// }
/// ```
#[inline]
pub fn add_start_file_separator<S: AsRef<str>>(s: S) -> String {
    add_start_file_separator_owned(s.as_ref())
}

/// Add a starting FILE_SEPARATOR into a string.
///
/// ```
/// extern crate slash_formatter;
///
/// let s = String::from("path");
///
/// let s = slash_formatter::add_start_file_separator_owned(s);
///
/// if cfg!(windows) {
///     assert_eq!(r"\path", s);
/// } else {
///     assert_eq!("/path", s);
/// }
/// ```
#[inline]
pub fn add_start_file_separator_owned<S: Into<String>>(s: S) -> String {
    let mut s = s.into();

    if !s.starts_with(FILE_SEPARATOR) {
        s.insert(0, FILE_SEPARATOR);
    }

    s
}

/// Add a starting FILE_SEPARATOR into a string.
///
/// ```
/// extern crate slash_formatter;
///
/// let mut s = String::from("path");
///
/// slash_formatter::add_start_file_separator_mut(&mut s);
///
/// if cfg!(windows) {
///     assert_eq!(r"\path", s);
/// } else {
///     assert_eq!("/path", s);
/// }
/// ```
#[inline]
pub fn add_start_file_separator_mut(s: &mut String) {
    if !s.starts_with(FILE_SEPARATOR) {
        s.insert(0, FILE_SEPARATOR);
    }
}

/// Add an ending FILE_SEPARATOR into a string.
///
/// ```
/// extern crate slash_formatter;
///
/// if cfg!(windows) {
///     assert_eq!(r"path\", slash_formatter::add_end_file_separator("path"));
/// } else {
///     assert_eq!("path/", slash_formatter::add_end_file_separator("path"));
/// }
/// ```
#[inline]
pub fn add_end_file_separator<S: AsRef<str>>(s: S) -> String {
    add_end_file_separator_owned(s.as_ref())
}

/// Add an ending FILE_SEPARATOR into a string.
///
/// ```
/// extern crate slash_formatter;
///
/// let s = String::from("path");
///
/// let s = slash_formatter::add_end_file_separator_owned(s);
///
/// if cfg!(windows) {
///     assert_eq!(r"path\", s);
/// } else {
///     assert_eq!("path/", s);
/// }
/// ```
#[inline]
pub fn add_end_file_separator_owned<S: Into<String>>(s: S) -> String {
    let mut s = s.into();

    if !s.ends_with(FILE_SEPARATOR) {
        s.push(FILE_SEPARATOR);
    }

    s
}

/// Add an ending FILE_SEPARATOR into a string.
///
/// ```
/// extern crate slash_formatter;
///
/// let mut s = String::from("path");
///
/// slash_formatter::add_end_file_separator_mut(&mut s);
///
/// if cfg!(windows) {
///     assert_eq!(r"path\", s);
/// } else {
///     assert_eq!("path/", s);
/// }
/// ```
#[inline]
pub fn add_end_file_separator_mut(s: &mut String) {
    if !s.ends_with(FILE_SEPARATOR) {
        s.push(FILE_SEPARATOR);
    }
}

/// Concatenate two strings with a FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// if cfg!(windows) {
///     assert_eq!(r"path\to", slash_formatter::concat_with_file_separator("path", r"to\"));
/// } else {
///     assert_eq!("path/to", slash_formatter::concat_with_file_separator("path", "to/"));
/// }
/// ```
#[inline]
pub fn concat_with_file_separator<S1: AsRef<str>, S2: AsRef<str>>(s1: S1, s2: S2) -> String {
    concat_with_file_separator_owned(s1.as_ref(), s2)
}

/// Concatenate two strings with a FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// let s = String::from("path");
///
/// if cfg!(windows) {
///     let s = slash_formatter::concat_with_file_separator_owned(s, r"to\");
///
///     assert_eq!(r"path\to", s);
/// } else {
///     let s = slash_formatter::concat_with_file_separator_owned(s, "to/");
///
///     assert_eq!("path/to", s);
/// }
/// ```
#[inline]
pub fn concat_with_file_separator_owned<S1: Into<String>, S2: AsRef<str>>(
    s1: S1,
    s2: S2,
) -> String {
    delete_end_file_separator_owned(
        add_end_file_separator_owned(s1) + delete_start_file_separator(s2.as_ref()),
    )
}

/// Concatenate two strings with a FILE_SEPARATOR.
///
/// ```
/// extern crate slash_formatter;
///
/// let mut s = String::from("path");
///
/// if cfg!(windows) {
///     slash_formatter::concat_with_file_separator_mut(&mut s, r"to\");
///
///     assert_eq!(r"path\to", s);
/// } else {
///     slash_formatter::concat_with_file_separator_mut(&mut s, "to/");
///
///     assert_eq!("path/to", s);
/// }
/// ```
#[inline]
pub fn concat_with_file_separator_mut<S2: AsRef<str>>(s1: &mut String, s2: S2) {
    add_end_file_separator_mut(s1);
    s1.push_str(delete_start_file_separator(s2.as_ref()));
    delete_end_file_separator_mut(s1);
}

/**
Concatenate multiple strings with FILE_SEPARATORs.

```
#[macro_use] extern crate slash_formatter;

if cfg!(windows) {
    assert_eq!(r"path\to\file", concat_with_file_separator!("path", r"to\", r"\file\"));

    let s = String::from("path");

    let s = concat_with_file_separator!(s, r"to\", r"\file\");

    assert_eq!(r"path\to\file", s);
} else {
    assert_eq!("path/to/file", concat_with_file_separator!("path", "to/", "/file/"));

    let s = String::from("path");

    let s = concat_with_file_separator!(s, "to/", "/file/");

    assert_eq!("path/to/file", s);
}
```
*/
#[macro_export]
macro_rules! concat_with_file_separator {
    ($s:expr, $($sc:expr), *) => {
        {
            let mut s = $s.to_owned();

            $(
                $crate::concat_with_file_separator_mut(&mut s, $sc);
            )*

            s
        }
    };
}

/**
Concatenate multiple strings with FILE_SEPARATORs.

```
#[macro_use] extern crate slash_formatter;

if cfg!(windows) {
    let mut s = String::from("path");

    concat_with_file_separator_mut!(&mut s, r"to\", r"\file\");

    assert_eq!(r"path\to\file", s);
} else {
    let mut s = String::from("path");

    concat_with_file_separator_mut!(&mut s, "to/", "/file/");

    assert_eq!("path/to/file", s);
}
```
*/
#[macro_export]
macro_rules! concat_with_file_separator_mut {
    ($s:expr, $($sc:expr), *) => {
        {
            $(
                $crate::concat_with_file_separator_mut($s, $sc);
            )*
        }
    };
}
