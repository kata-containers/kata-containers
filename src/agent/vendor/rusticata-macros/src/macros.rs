//! Helper macros

use nom::bytes::complete::take;
use nom::combinator::map_res;
use nom::IResult;

#[doc(hidden)]
pub mod export {
    pub use core::{fmt, mem, ptr};
}

/// Helper macro for newtypes: declare associated constants and implement Display trait
#[macro_export]
macro_rules! newtype_enum (
    (@collect_impl, $name:ident, $($key:ident = $val:expr),* $(,)*) => {
        $( pub const $key : $name = $name($val); )*
    };

    (@collect_disp, $name:ident, $f:ident, $m:expr, $($key:ident = $val:expr),* $(,)*) => {
        match $m {
            $( $val => write!($f, stringify!{$key}), )*
            n => write!($f, "{}({} / 0x{:x})", stringify!{$name}, n, n)
        }
    };

    // entry
    (impl $name:ident {$($body:tt)*}) => (
        #[allow(non_upper_case_globals)]
        impl $name {
            newtype_enum!{@collect_impl, $name, $($body)*}
        }
    );

    // entry with display
    (impl display $name:ident {$($body:tt)*}) => (
        newtype_enum!(impl $name { $($body)* });

        impl $crate::export::fmt::Display for $name {
            fn fmt(&self, f: &mut $crate::export::fmt::Formatter) -> $crate::export::fmt::Result {
                newtype_enum!(@collect_disp, $name, f, self.0, $($body)*)
            }
        }
    );

    // entry with display and debug
    (impl debug $name:ident {$($body:tt)*}) => (
        newtype_enum!(impl display $name { $($body)* });

        impl $crate::export::fmt::Debug for $name {
            fn fmt(&self, f: &mut $crate::export::fmt::Formatter) -> $crate::export::fmt::Result {
                write!(f, "{}", self)
            }
        }
    );
);

/// Helper macro for nom parsers: raise error if the condition is true
///
/// This macro is used when using custom errors
#[macro_export]
macro_rules! custom_check (
  ($i:expr, $cond:expr, $err:expr) => (
    {
      if $cond {
        Err(::nom::Err::Error($err))
      } else {
        Ok(($i, ()))
      }
    }
  );
);

/// Helper macro for nom parsers: raise error if the condition is true
///
/// This macro is used when using `ErrorKind`
#[macro_export]
macro_rules! error_if (
  ($i:expr, $cond:expr, $err:expr) => (
    {
      use nom::error_position;
      if $cond {
        Err(::nom::Err::Error(error_position!($i, $err)))
      } else {
        Ok(($i, ()))
      }
    }
  );
);

/// Helper macro for nom parsers: raise error if input is not empty
///
/// Deprecated - use `nom::eof`
#[macro_export]
#[deprecated(since = "2.0.0")]
macro_rules! empty (
  ($i:expr,) => (
    {
      use nom::eof;
      eof!($i,)
    }
  );
);

#[deprecated(since = "3.0.1", note = "please use `be_var_u64` instead")]
/// Read an entire slice as a big-endian value.
///
/// Returns the value as `u64`. This function checks for integer overflows, and returns a
/// `Result::Err` value if the value is too big.
pub fn bytes_to_u64(s: &[u8]) -> Result<u64, &'static str> {
    let mut u: u64 = 0;

    if s.is_empty() {
        return Err("empty");
    };
    if s.len() > 8 {
        return Err("overflow");
    }
    for &c in s {
        let u1 = u << 8;
        u = u1 | (c as u64);
    }

    Ok(u)
}

/// Read a slice as a big-endian value.
#[macro_export]
macro_rules! parse_hex_to_u64 (
    ( $i:expr, $size:expr ) => {
        map_res(take($size as usize), $crate::combinator::be_var_u64)($i)
    };
);

/// Read 3 bytes as an unsigned integer
#[deprecated(since = "0.5.0", note = "please use `be_u24` instead")]
#[allow(deprecated)]
#[inline]
pub fn parse_uint24(i: &[u8]) -> IResult<&[u8], u64> {
    map_res(take(3usize), bytes_to_u64)(i)
}

//named!(parse_hex4<&[u8], u64>, parse_hex_to_u64!(4));

/// Combination and flat_map! and take! as first combinator
#[macro_export]
macro_rules! flat_take (
    ($i:expr, $len:expr, $f:ident) => ({
        if $i.len() < $len { Err(::nom::Err::Incomplete(::nom::Needed::new($len))) }
        else {
            let taken = &$i[0..$len];
            let rem = &$i[$len..];
            match $f(taken) {
                Ok((_,res)) => Ok((rem,res)),
                Err(e)      => Err(e)
            }
        }
    });
    ($i:expr, $len:expr, $submac:ident!( $($args:tt)*)) => ({
        if $i.len() < $len { Err(::nom::Err::Incomplete(::nom::Needed::new($len))) }
        else {
            let taken = &$i[0..$len];
            let rem = &$i[$len..];
            match $submac!(taken, $($args)*) {
                Ok((_,res)) => Ok((rem,res)),
                Err(e)      => Err(e)
            }
        }
    });
);

/// Apply combinator, trying to "upgrade" error to next error type (using the `Into` or `From`
/// traits).
#[macro_export]
macro_rules! upgrade_error (
    ($i:expr, $submac:ident!( $($args:tt)*) ) => ({
        upgrade_error!( $submac!( $i, $($args)* ) )
    });
    ($i:expr, $f:expr) => ({
        upgrade_error!( call!($i, $f) )
    });
    ($e:expr) => ({
        match $e {
            Ok(o) => Ok(o),
            Err(::nom::Err::Error(e)) => Err(::nom::Err::Error(e.into())),
            Err(::nom::Err::Failure(e)) => Err(::nom::Err::Failure(e.into())),
            Err(::nom::Err::Incomplete(i)) => Err(::nom::Err::Incomplete(i)),
        }
    });
);

/// Apply combinator, trying to "upgrade" error to next error type (using the `Into` or `From`
/// traits).
#[macro_export]
macro_rules! upgrade_error_to (
    ($i:expr, $ty:ty, $submac:ident!( $($args:tt)*) ) => ({
        upgrade_error_to!( $ty, $submac!( $i, $($args)* ) )
    });
    ($i:expr, $ty:ty, $f:expr) => ({
        upgrade_error_to!( $ty, call!($i, $f) )
    });
    ($ty:ty, $e:expr) => ({
        match $e {
            Ok(o) => Ok(o),
            Err(::nom::Err::Error(e)) => Err(::nom::Err::Error(e.into::<$ty>())),
            Err(::nom::Err::Failure(e)) => Err(::nom::Err::Failure(e.into::<$ty>())),
            Err(::nom::Err::Incomplete(i)) => Err(::nom::Err::Incomplete(i)),
        }
    });
);

/// Nom combinator that returns the given expression unchanged
#[macro_export]
macro_rules! q {
    ($i:expr, $x:expr) => {{
        Ok(($i, $x))
    }};
}

/// Align input value to the next multiple of n bytes
/// Valid only if n is a power of 2
#[macro_export]
macro_rules! align_n2 {
    ($x:expr, $n:expr) => {
        ($x + ($n - 1)) & !($n - 1)
    };
}

/// Align input value to the next multiple of 4 bytes
#[macro_export]
macro_rules! align32 {
    ($x:expr) => {
        $crate::align_n2!($x, 4)
    };
}

#[cfg(test)]
mod tests {
    use nom::error::ErrorKind;
    use nom::number::streaming::{be_u16, be_u32};
    use nom::{error_position, Err, IResult, Needed};

    #[test]
    fn test_error_if() {
        let empty = &b""[..];
        let res: IResult<&[u8], ()> = error_if!(empty, true, ErrorKind::Tag);
        assert_eq!(res, Err(Err::Error(error_position!(empty, ErrorKind::Tag))));
    }

    #[test]
    fn test_newtype_enum() {
        #[derive(Debug, PartialEq, Eq)]
        struct MyType(pub u8);

        newtype_enum! {
            impl display MyType {
                Val1 = 0,
                Val2 = 1
            }
        }

        assert_eq!(MyType(0), MyType::Val1);
        assert_eq!(MyType(1), MyType::Val2);

        assert_eq!(format!("{}", MyType(0)), "Val1");
        assert_eq!(format!("{}", MyType(4)), "MyType(4 / 0x4)");
    }
    #[test]
    fn test_flat_take() {
        let input = &[0x00, 0x01, 0xff];
        // read first 2 bytes and use correct combinator: OK
        let res: IResult<&[u8], u16> = flat_take!(input, 2, be_u16);
        assert_eq!(res, Ok((&input[2..], 0x0001)));
        // read 3 bytes and use 2: OK (some input is just lost)
        let res: IResult<&[u8], u16> = flat_take!(input, 3, be_u16);
        assert_eq!(res, Ok((&b""[..], 0x0001)));
        // read 2 bytes and a combinator requiring more bytes
        let res: IResult<&[u8], u32> = flat_take!(input, 2, be_u32);
        assert_eq!(res, Err(Err::Incomplete(Needed::new(2))));
        // test with macro as sub-combinator
        let res: IResult<&[u8], u16> = flat_take!(input, 2, be_u16);
        assert_eq!(res, Ok((&input[2..], 0x0001)));
    }

    #[test]
    fn test_q() {
        let empty = &b""[..];
        let res: IResult<&[u8], &str, ErrorKind> = q!(empty, "test");
        assert_eq!(res, Ok((empty, "test")));
    }

    #[test]
    fn test_align32() {
        assert_eq!(align32!(3), 4);
        assert_eq!(align32!(4), 4);
        assert_eq!(align32!(5), 8);
        assert_eq!(align32!(5u32), 8);
        assert_eq!(align32!(5i32), 8);
        assert_eq!(align32!(5usize), 8);
    }
}
