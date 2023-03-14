use cfg_if::cfg_if;
use std::borrow::Cow;
use std::ffi::OsStr;

#[derive(Debug, PartialEq)]
pub(crate) enum FlagResult<'a> {
    Argument,
    EndFlags,
    BadFlag,
    Flag {
        num_minuses: usize,
        name: Cow<'a, OsStr>,
        value: Option<Cow<'a, OsStr>>,
    },
}

pub(crate) fn parse_one(s: &OsStr) -> FlagResult<'_> {
    let s = if let Some(s) = s.to_str() {
        s
    } else {
        return parse_one_fallback(s);
    };

    if s.len() < 2 || !s.starts_with("-") {
        // Empty string, `-` and something other than `/-.*/` is a non-flag.
        return FlagResult::Argument;
    }
    if s == "--" {
        // `--` terminates flags.
        return FlagResult::EndFlags;
    }
    let (num_minuses, nv) = if s.starts_with("--") {
        (2, &s[2..])
    } else {
        (1, &s[1..])
    };
    if nv.len() == 0 || nv.starts_with("-") || nv.starts_with("=") {
        return FlagResult::BadFlag;
    }
    let equal_pos = nv.find('=');
    let (name, value) = if let Some(equal_pos) = equal_pos {
        (&nv[..equal_pos], Some(&nv[equal_pos + 1..]))
    } else {
        (nv, None)
    };
    FlagResult::Flag {
        num_minuses,
        name: Cow::from(OsStr::new(name)),
        value: value.map(|value| Cow::from(OsStr::new(value))),
    }
}

cfg_if! {
    if #[cfg(any(unix, target_os = "redox"))] {
        fn parse_one_fallback(s: &OsStr) -> FlagResult<'_> {
            use std::os::unix::ffi::OsStrExt;

            let s = s.as_bytes();
            if s.len() < 2 || !s.starts_with(b"-") {
                // Empty string, `-` and something other than `/-.*/` is a non-flag.
                return FlagResult::Argument;
            }
            if s == b"--" {
                // `--` terminates flags.
                return FlagResult::EndFlags;
            }
            let (num_minuses, nv) = if s.starts_with(b"--") {
                (2, &s[2..])
            } else {
                (1, &s[1..])
            };
            if nv.len() == 0 || nv.starts_with(b"-") || nv.starts_with(b"=") {
                return FlagResult::BadFlag;
            }
            let equal_pos = nv.find(b'=');
            let (name, value) = if let Some(equal_pos) = equal_pos {
                (&nv[..equal_pos], Some(&nv[equal_pos + 1..]))
            } else {
                (nv, None)
            };
            FlagResult::Flag {
                num_minuses,
                name: Cow::from(OsStr::from_bytes(name)),
                value: value.map(|value| Cow::from(OsStr::from_bytes(value))),
            }
        }
    } else if #[cfg(windows)] {
        fn parse_one_fallback(s: &OsStr) -> FlagResult<'_> {
            use std::ffi::OsString;
            use std::os::windows::ffi::{OsStrExt, OsStringExt};

            let s = s.encode_wide().collect::<Vec<_>>();
            if s.len() < 2 || !s.starts_with(&[b'-' as u16]) {
                // Empty string, `-` and something other than `/-.*/` is a non-flag.
                return FlagResult::Argument;
            }
            if s == [b'-' as u16, b'-' as u16] {
                // `--` terminates flags.
                return FlagResult::EndFlags;
            }
            let (num_minuses, nv) = if s.starts_with(&[b'-' as u16, b'-' as u16]) {
                (2, &s[2..])
            } else {
                (1, &s[1..])
            };
            if nv.len() == 0 || nv.starts_with(&[b'-' as u16]) || nv.starts_with(&[b'=' as u16]) {
                return FlagResult::BadFlag;
            }
            let equal_pos = nv.find(b'=' as u16);
            let (name, value) = if let Some(equal_pos) = equal_pos {
                (&nv[..equal_pos], Some(&nv[equal_pos + 1..]))
            } else {
                (nv, None)
            };
            FlagResult::Flag {
                num_minuses,
                name: Cow::from(OsString::from_wide(name)),
                value: value.map(|value| Cow::from(OsString::from_wide(value))),
            }
        }
    } else {
        compile_error!("TODO: implement for cfg(not(any(unix, target_os = \"redox\", windows))) case");
    }
}

#[allow(unused)]
trait BytesExt {
    type Unit;
    fn starts_with(&self, s: &Self) -> bool;
    fn find(&self, ch: Self::Unit) -> Option<usize>;
}

impl BytesExt for [u8] {
    type Unit = u8;
    fn starts_with(&self, s: &Self) -> bool {
        self.len() >= s.len() && &self[..s.len()] == s
    }
    fn find(&self, ch: Self::Unit) -> Option<usize> {
        self.iter().position(|&c| c == ch)
    }
}

impl BytesExt for [u16] {
    type Unit = u16;
    fn starts_with(&self, s: &Self) -> bool {
        self.len() >= s.len() && &self[..s.len()] == s
    }
    fn find(&self, ch: Self::Unit) -> Option<usize> {
        self.iter().position(|&c| c == ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_one() {
        assert_eq!(parse_one("".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z-y".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("=".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("=x".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z=".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z=x".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z-y=".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z-y=x".as_ref()), FlagResult::Argument);

        assert_eq!(parse_one("-".as_ref()), FlagResult::Argument);
        assert_eq!(
            parse_one("-z".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z")),
                value: None,
            }
        );
        assert_eq!(
            parse_one("-z-y".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z-y")),
                value: None,
            }
        );
        assert_eq!(parse_one("-=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("-=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(
            parse_one("-z=".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("-z=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );
        assert_eq!(
            parse_one("-z-y=".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("-z-y=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );

        assert_eq!(parse_one("--".as_ref()), FlagResult::EndFlags);
        assert_eq!(
            parse_one("--z".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z")),
                value: None,
            }
        );
        assert_eq!(
            parse_one("--z-y".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z-y")),
                value: None,
            }
        );
        assert_eq!(parse_one("--=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("--=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(
            parse_one("--z=".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("--z=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );
        assert_eq!(
            parse_one("--z-y=".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("--z-y=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );

        assert_eq!(parse_one("---".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z-y".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z-y=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z-y=x".as_ref()), FlagResult::BadFlag);
    }

    #[test]
    #[cfg(any(unix, target_os = "redox", windows))]
    fn test_parse_one_fallback() {
        use parse_one_fallback as parse_one;

        assert_eq!(parse_one("".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z-y".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("=".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("=x".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z=".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z=x".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z-y=".as_ref()), FlagResult::Argument);
        assert_eq!(parse_one("z-y=x".as_ref()), FlagResult::Argument);

        assert_eq!(parse_one("-".as_ref()), FlagResult::Argument);
        assert_eq!(
            parse_one("-z".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z")),
                value: None,
            }
        );
        assert_eq!(
            parse_one("-z-y".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z-y")),
                value: None,
            }
        );
        assert_eq!(parse_one("-=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("-=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(
            parse_one("-z=".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("-z=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );
        assert_eq!(
            parse_one("-z-y=".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("-z-y=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );

        assert_eq!(parse_one("--".as_ref()), FlagResult::EndFlags);
        assert_eq!(
            parse_one("--z".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z")),
                value: None,
            }
        );
        assert_eq!(
            parse_one("--z-y".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z-y")),
                value: None,
            }
        );
        assert_eq!(parse_one("--=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("--=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(
            parse_one("--z=".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("--z=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );
        assert_eq!(
            parse_one("--z-y=".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("")))
            }
        );
        assert_eq!(
            parse_one("--z-y=x".as_ref()),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::new("z-y")),
                value: Some(Cow::from(OsStr::new("x")))
            }
        );

        assert_eq!(parse_one("---".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z-y".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z=x".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z-y=".as_ref()), FlagResult::BadFlag);
        assert_eq!(parse_one("---z-y=x".as_ref()), FlagResult::BadFlag);
    }

    #[test]
    #[cfg(any(unix, target_os = "redox"))]
    fn test_parse_one_unix() {
        use std::os::unix::ffi::OsStrExt;

        assert_eq!(parse_one(OsStr::from_bytes(b"")), FlagResult::Argument);
        assert_eq!(parse_one(OsStr::from_bytes(b"z\xA0")), FlagResult::Argument);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"z\xA0-y\xB0")),
            FlagResult::Argument
        );
        assert_eq!(parse_one(OsStr::from_bytes(b"=")), FlagResult::Argument);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"=x\xC0")),
            FlagResult::Argument
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"z\xA0=")),
            FlagResult::Argument
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"z\xA0=x\xC0")),
            FlagResult::Argument
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"z\xA0-y\xB0=")),
            FlagResult::Argument
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"z\xA0-y\xB0=x\xC0")),
            FlagResult::Argument
        );

        assert_eq!(parse_one(OsStr::from_bytes(b"-")), FlagResult::Argument);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"-z\xA0")),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::from_bytes(b"z\xA0")),
                value: None,
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"-z\xA0-y\xB0")),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::from_bytes(b"z\xA0-y\xB0")),
                value: None,
            }
        );
        assert_eq!(parse_one(OsStr::from_bytes(b"-=")), FlagResult::BadFlag);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"-=x\xC0")),
            FlagResult::BadFlag
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"-z\xA0=")),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::from_bytes(b"z\xA0")),
                value: Some(Cow::from(OsStr::from_bytes(b"")))
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"-z\xA0=x\xC0")),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::from_bytes(b"z\xA0")),
                value: Some(Cow::from(OsStr::from_bytes(b"x\xC0")))
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"-z\xA0-y\xB0=")),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::from_bytes(b"z\xA0-y\xB0")),
                value: Some(Cow::from(OsStr::from_bytes(b"")))
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"-z\xA0-y\xB0=x\xC0")),
            FlagResult::Flag {
                num_minuses: 1,
                name: Cow::from(OsStr::from_bytes(b"z\xA0-y\xB0")),
                value: Some(Cow::from(OsStr::from_bytes(b"x\xC0")))
            }
        );

        assert_eq!(parse_one(OsStr::from_bytes(b"--")), FlagResult::EndFlags);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"--z\xA0")),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::from_bytes(b"z\xA0")),
                value: None,
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"--z\xA0-y\xB0")),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::from_bytes(b"z\xA0-y\xB0")),
                value: None,
            }
        );
        assert_eq!(parse_one(OsStr::from_bytes(b"--=")), FlagResult::BadFlag);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"--=x\xC0")),
            FlagResult::BadFlag
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"--z\xA0=")),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::from_bytes(b"z\xA0")),
                value: Some(Cow::from(OsStr::from_bytes(b"")))
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"--z\xA0=x\xC0")),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::from_bytes(b"z\xA0")),
                value: Some(Cow::from(OsStr::from_bytes(b"x\xC0")))
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"--z\xA0-y\xB0=")),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::from_bytes(b"z\xA0-y\xB0")),
                value: Some(Cow::from(OsStr::from_bytes(b"")))
            }
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"--z\xA0-y\xB0=x\xC0")),
            FlagResult::Flag {
                num_minuses: 2,
                name: Cow::from(OsStr::from_bytes(b"z\xA0-y\xB0")),
                value: Some(Cow::from(OsStr::from_bytes(b"x\xC0")))
            }
        );

        assert_eq!(parse_one(OsStr::from_bytes(b"---")), FlagResult::BadFlag);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"---z\xA0")),
            FlagResult::BadFlag
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"---z\xA0-y\xB0")),
            FlagResult::BadFlag
        );
        assert_eq!(parse_one(OsStr::from_bytes(b"---=")), FlagResult::BadFlag);
        assert_eq!(
            parse_one(OsStr::from_bytes(b"---=x\xC0")),
            FlagResult::BadFlag
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"---z\xA0=")),
            FlagResult::BadFlag
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"---z\xA0=x\xC0")),
            FlagResult::BadFlag
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"---z\xA0-y\xB0=")),
            FlagResult::BadFlag
        );
        assert_eq!(
            parse_one(OsStr::from_bytes(b"---z\xA0-y\xB0=x\xC0")),
            FlagResult::BadFlag
        );
    }
}
