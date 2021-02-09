//! Adapted from [`nom`](https://github.com/Geal/nom).

use std::str::{Bytes, CharIndices, Chars};

use unicode_xid::UnicodeXID;

use fallback::LexError;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Cursor<'a> {
    pub rest: &'a str,
    #[cfg(span_locations)]
    pub off: u32,
}

impl<'a> Cursor<'a> {
    #[cfg(not(span_locations))]
    pub fn advance(&self, amt: usize) -> Cursor<'a> {
        Cursor {
            rest: &self.rest[amt..],
        }
    }
    #[cfg(span_locations)]
    pub fn advance(&self, amt: usize) -> Cursor<'a> {
        Cursor {
            rest: &self.rest[amt..],
            off: self.off + (amt as u32),
        }
    }

    pub fn find(&self, p: char) -> Option<usize> {
        self.rest.find(p)
    }

    pub fn starts_with(&self, s: &str) -> bool {
        self.rest.starts_with(s)
    }

    pub fn is_empty(&self) -> bool {
        self.rest.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rest.len()
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.rest.as_bytes()
    }

    pub fn bytes(&self) -> Bytes<'a> {
        self.rest.bytes()
    }

    pub fn chars(&self) -> Chars<'a> {
        self.rest.chars()
    }

    pub fn char_indices(&self) -> CharIndices<'a> {
        self.rest.char_indices()
    }
}

pub type PResult<'a, O> = Result<(Cursor<'a>, O), LexError>;

pub fn whitespace(input: Cursor) -> PResult<()> {
    if input.is_empty() {
        return Err(LexError);
    }

    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let s = input.advance(i);
        if bytes[i] == b'/' {
            if s.starts_with("//")
                && (!s.starts_with("///") || s.starts_with("////"))
                && !s.starts_with("//!")
            {
                if let Some(len) = s.find('\n') {
                    i += len + 1;
                    continue;
                }
                break;
            } else if s.starts_with("/**/") {
                i += 4;
                continue;
            } else if s.starts_with("/*")
                && (!s.starts_with("/**") || s.starts_with("/***"))
                && !s.starts_with("/*!")
            {
                let (_, com) = block_comment(s)?;
                i += com.len();
                continue;
            }
        }
        match bytes[i] {
            b' ' | 0x09...0x0d => {
                i += 1;
                continue;
            }
            b if b <= 0x7f => {}
            _ => {
                let ch = s.chars().next().unwrap();
                if is_whitespace(ch) {
                    i += ch.len_utf8();
                    continue;
                }
            }
        }
        return if i > 0 { Ok((s, ())) } else { Err(LexError) };
    }
    Ok((input.advance(input.len()), ()))
}

pub fn block_comment(input: Cursor) -> PResult<&str> {
    if !input.starts_with("/*") {
        return Err(LexError);
    }

    let mut depth = 0;
    let bytes = input.as_bytes();
    let mut i = 0;
    let upper = bytes.len() - 1;
    while i < upper {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
            depth += 1;
            i += 1; // eat '*'
        } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            depth -= 1;
            if depth == 0 {
                return Ok((input.advance(i + 2), &input.rest[..i + 2]));
            }
            i += 1; // eat '/'
        }
        i += 1;
    }
    Err(LexError)
}

pub fn skip_whitespace(input: Cursor) -> Cursor {
    match whitespace(input) {
        Ok((rest, _)) => rest,
        Err(LexError) => input,
    }
}

fn is_whitespace(ch: char) -> bool {
    // Rust treats left-to-right mark and right-to-left mark as whitespace
    ch.is_whitespace() || ch == '\u{200e}' || ch == '\u{200f}'
}

pub fn word_break(input: Cursor) -> PResult<()> {
    match input.chars().next() {
        Some(ch) if UnicodeXID::is_xid_continue(ch) => Err(LexError),
        Some(_) | None => Ok((input, ())),
    }
}

macro_rules! named {
    ($name:ident -> $o:ty, $submac:ident!( $($args:tt)* )) => {
        fn $name<'a>(i: Cursor<'a>) -> $crate::strnom::PResult<'a, $o> {
            $submac!(i, $($args)*)
        }
    };
}

macro_rules! alt {
    ($i:expr, $e:ident | $($rest:tt)*) => {
        alt!($i, call!($e) | $($rest)*)
    };

    ($i:expr, $subrule:ident!( $($args:tt)*) | $($rest:tt)*) => {
        match $subrule!($i, $($args)*) {
            res @ Ok(_) => res,
            _ => alt!($i, $($rest)*)
        }
    };

    ($i:expr, $subrule:ident!( $($args:tt)* ) => { $gen:expr } | $($rest:tt)+) => {
        match $subrule!($i, $($args)*) {
            Ok((i, o)) => Ok((i, $gen(o))),
            Err(LexError) => alt!($i, $($rest)*)
        }
    };

    ($i:expr, $e:ident => { $gen:expr } | $($rest:tt)*) => {
        alt!($i, call!($e) => { $gen } | $($rest)*)
    };

    ($i:expr, $e:ident => { $gen:expr }) => {
        alt!($i, call!($e) => { $gen })
    };

    ($i:expr, $subrule:ident!( $($args:tt)* ) => { $gen:expr }) => {
        match $subrule!($i, $($args)*) {
            Ok((i, o)) => Ok((i, $gen(o))),
            Err(LexError) => Err(LexError),
        }
    };

    ($i:expr, $e:ident) => {
        alt!($i, call!($e))
    };

    ($i:expr, $subrule:ident!( $($args:tt)*)) => {
        $subrule!($i, $($args)*)
    };
}

macro_rules! do_parse {
    ($i:expr, ( $($rest:expr),* )) => {
        Ok(($i, ( $($rest),* )))
    };

    ($i:expr, $e:ident >> $($rest:tt)*) => {
        do_parse!($i, call!($e) >> $($rest)*)
    };

    ($i:expr, $submac:ident!( $($args:tt)* ) >> $($rest:tt)*) => {
        match $submac!($i, $($args)*) {
            Err(LexError) => Err(LexError),
            Ok((i, _)) => do_parse!(i, $($rest)*),
        }
    };

    ($i:expr, $field:ident : $e:ident >> $($rest:tt)*) => {
        do_parse!($i, $field: call!($e) >> $($rest)*)
    };

    ($i:expr, $field:ident : $submac:ident!( $($args:tt)* ) >> $($rest:tt)*) => {
        match $submac!($i, $($args)*) {
            Err(LexError) => Err(LexError),
            Ok((i, o)) => {
                let $field = o;
                do_parse!(i, $($rest)*)
            },
        }
    };
}

macro_rules! peek {
    ($i:expr, $submac:ident!( $($args:tt)* )) => {
        match $submac!($i, $($args)*) {
            Ok((_, o)) => Ok(($i, o)),
            Err(LexError) => Err(LexError),
        }
    };
}

macro_rules! call {
    ($i:expr, $fun:expr $(, $args:expr)*) => {
        $fun($i $(, $args)*)
    };
}

macro_rules! option {
    ($i:expr, $f:expr) => {
        match $f($i) {
            Ok((i, o)) => Ok((i, Some(o))),
            Err(LexError) => Ok(($i, None)),
        }
    };
}

macro_rules! take_until_newline_or_eof {
    ($i:expr,) => {{
        if $i.len() == 0 {
            Ok(($i, ""))
        } else {
            match $i.find('\n') {
                Some(i) => Ok(($i.advance(i), &$i.rest[..i])),
                None => Ok(($i.advance($i.len()), &$i.rest[..$i.len()])),
            }
        }
    }};
}

macro_rules! tuple {
    ($i:expr, $($rest:tt)*) => {
        tuple_parser!($i, (), $($rest)*)
    };
}

/// Do not use directly. Use `tuple!`.
macro_rules! tuple_parser {
    ($i:expr, ($($parsed:tt),*), $e:ident, $($rest:tt)*) => {
        tuple_parser!($i, ($($parsed),*), call!($e), $($rest)*)
    };

    ($i:expr, (), $submac:ident!( $($args:tt)* ), $($rest:tt)*) => {
        match $submac!($i, $($args)*) {
            Err(LexError) => Err(LexError),
            Ok((i, o)) => tuple_parser!(i, (o), $($rest)*),
        }
    };

    ($i:expr, ($($parsed:tt)*), $submac:ident!( $($args:tt)* ), $($rest:tt)*) => {
        match $submac!($i, $($args)*) {
            Err(LexError) => Err(LexError),
            Ok((i, o)) => tuple_parser!(i, ($($parsed)* , o), $($rest)*),
        }
    };

    ($i:expr, ($($parsed:tt),*), $e:ident) => {
        tuple_parser!($i, ($($parsed),*), call!($e))
    };

    ($i:expr, (), $submac:ident!( $($args:tt)* )) => {
        $submac!($i, $($args)*)
    };

    ($i:expr, ($($parsed:expr),*), $submac:ident!( $($args:tt)* )) => {
        match $submac!($i, $($args)*) {
            Err(LexError) => Err(LexError),
            Ok((i, o)) => Ok((i, ($($parsed),*, o)))
        }
    };

    ($i:expr, ($($parsed:expr),*)) => {
        Ok(($i, ($($parsed),*)))
    };
}

macro_rules! not {
    ($i:expr, $submac:ident!( $($args:tt)* )) => {
        match $submac!($i, $($args)*) {
            Ok((_, _)) => Err(LexError),
            Err(LexError) => Ok(($i, ())),
        }
    };
}

macro_rules! tag {
    ($i:expr, $tag:expr) => {
        if $i.starts_with($tag) {
            Ok(($i.advance($tag.len()), &$i.rest[..$tag.len()]))
        } else {
            Err(LexError)
        }
    };
}

macro_rules! punct {
    ($i:expr, $punct:expr) => {
        $crate::strnom::punct($i, $punct)
    };
}

/// Do not use directly. Use `punct!`.
pub fn punct<'a>(input: Cursor<'a>, token: &'static str) -> PResult<'a, &'a str> {
    let input = skip_whitespace(input);
    if input.starts_with(token) {
        Ok((input.advance(token.len()), token))
    } else {
        Err(LexError)
    }
}

macro_rules! preceded {
    ($i:expr, $submac:ident!( $($args:tt)* ), $submac2:ident!( $($args2:tt)* )) => {
        match tuple!($i, $submac!($($args)*), $submac2!($($args2)*)) {
            Ok((remaining, (_, o))) => Ok((remaining, o)),
            Err(LexError) => Err(LexError),
        }
    };

    ($i:expr, $submac:ident!( $($args:tt)* ), $g:expr) => {
        preceded!($i, $submac!($($args)*), call!($g))
    };
}

macro_rules! delimited {
    ($i:expr, $submac:ident!( $($args:tt)* ), $($rest:tt)+) => {
        match tuple_parser!($i, (), $submac!($($args)*), $($rest)*) {
            Err(LexError) => Err(LexError),
            Ok((i1, (_, o, _))) => Ok((i1, o))
        }
    };
}

macro_rules! map {
    ($i:expr, $submac:ident!( $($args:tt)* ), $g:expr) => {
        match $submac!($i, $($args)*) {
            Err(LexError) => Err(LexError),
            Ok((i, o)) => Ok((i, call!(o, $g)))
        }
    };

    ($i:expr, $f:expr, $g:expr) => {
        map!($i, call!($f), $g)
    };
}
