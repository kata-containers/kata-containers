//! General purpose combinators

use nom::bytes::streaming::take;
use nom::combinator::map_parser;
use nom::error::{make_error, ErrorKind, ParseError};
use nom::{IResult, Needed, Parser};
use nom::{InputIter, InputTake};
use nom::{InputLength, ToUsize};

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

/// Read the entire slice as a big endian unsigned integer, up to 8 bytes
#[inline]
pub fn be_var_u64<'a, E: ParseError<&'a [u8]>>(input: &'a [u8]) -> IResult<&'a [u8], u64, E> {
    if input.is_empty() {
        return Err(nom::Err::Incomplete(Needed::new(1)));
    }
    if input.len() > 8 {
        return Err(nom::Err::Error(make_error(input, ErrorKind::TooLarge)));
    }
    let mut res = 0u64;
    for byte in input {
        res = (res << 8) + *byte as u64;
    }

    Ok((&b""[..], res))
}

/// Read the entire slice as a little endian unsigned integer, up to 8 bytes
#[inline]
pub fn le_var_u64<'a, E: ParseError<&'a [u8]>>(input: &'a [u8]) -> IResult<&'a [u8], u64, E> {
    if input.is_empty() {
        return Err(nom::Err::Incomplete(Needed::new(1)));
    }
    if input.len() > 8 {
        return Err(nom::Err::Error(make_error(input, ErrorKind::TooLarge)));
    }
    let mut res = 0u64;
    for byte in input.iter().rev() {
        res = (res << 8) + *byte as u64;
    }

    Ok((&b""[..], res))
}

/// Read a slice as a big-endian value.
#[inline]
pub fn parse_hex_to_u64<S>(i: &[u8], size: S) -> IResult<&[u8], u64>
where
    S: ToUsize + Copy,
{
    map_parser(take(size.to_usize()), be_var_u64)(i)
}

/// Apply combinator, automatically converts between errors if the underlying type supports it
pub fn upgrade_error<I, O, E1: ParseError<I>, E2: ParseError<I>, F>(
    mut f: F,
) -> impl FnMut(I) -> IResult<I, O, E2>
where
    F: FnMut(I) -> IResult<I, O, E1>,
    E2: From<E1>,
{
    move |i| f(i).map_err(nom::Err::convert)
}

/// Create a combinator that returns the provided value, and input unchanged
pub fn pure<I, O, E: ParseError<I>>(val: O) -> impl Fn(I) -> IResult<I, O, E>
where
    O: Clone,
{
    move |input: I| Ok((input, val.clone()))
}

/// Return a closure that takes `len` bytes from input, and applies `parser`.
pub fn flat_take<I, C, O, E: ParseError<I>, F>(
    len: C,
    mut parser: F,
) -> impl FnMut(I) -> IResult<I, O, E>
where
    I: InputTake + InputLength + InputIter,
    C: ToUsize + Copy,
    F: Parser<I, O, E>,
{
    // Note: this is the same as `map_parser(take(len), parser)`
    move |input: I| {
        let (input, o1) = take(len.to_usize())(input)?;
        let (_, o2) = parser.parse(o1)?;
        Ok((input, o2))
    }
}

/// Take `len` bytes from `input`, and apply `parser`.
pub fn flat_takec<I, O, E: ParseError<I>, C, F>(input: I, len: C, parser: F) -> IResult<I, O, E>
where
    C: ToUsize + Copy,
    F: Parser<I, O, E>,
    I: InputTake + InputLength + InputIter,
    O: InputLength,
{
    flat_take(len, parser)(input)
}

/// Helper macro for nom parsers: run first parser if condition is true, else second parser
pub fn cond_else<I, O, E: ParseError<I>, C, F, G>(
    cond: C,
    mut first: F,
    mut second: G,
) -> impl FnMut(I) -> IResult<I, O, E>
where
    C: Fn() -> bool,
    F: Parser<I, O, E>,
    G: Parser<I, O, E>,
{
    move |input: I| {
        if cond() {
            first.parse(input)
        } else {
            second.parse(input)
        }
    }
}

/// Align input value to the next multiple of n bytes
/// Valid only if n is a power of 2
pub const fn align_n2(x: usize, n: usize) -> usize {
    (x + (n - 1)) & !(n - 1)
}

/// Align input value to the next multiple of 4 bytes
pub const fn align32(x: usize) -> usize {
    (x + 3) & !3
}

#[cfg(test)]
mod tests {
    use super::{align32, be_var_u64, cond_else, flat_take, pure};
    use nom::bytes::streaming::take;
    use nom::number::streaming::{be_u16, be_u32, be_u8};
    use nom::{Err, IResult, Needed};

    #[test]
    fn test_be_var_u64() {
        let res: IResult<&[u8], u64> = be_var_u64(b"\x12\x34\x56");
        let (_, v) = res.expect("be_var_u64 failed");
        assert_eq!(v, 0x123456);
    }

    #[test]
    fn test_flat_take() {
        let input = &[0x00, 0x01, 0xff];
        // read first 2 bytes and use correct combinator: OK
        let res: IResult<&[u8], u16> = flat_take(2u8, be_u16)(input);
        assert_eq!(res, Ok((&input[2..], 0x0001)));
        // read 3 bytes and use 2: OK (some input is just lost)
        let res: IResult<&[u8], u16> = flat_take(3u8, be_u16)(input);
        assert_eq!(res, Ok((&b""[..], 0x0001)));
        // read 2 bytes and a combinator requiring more bytes
        let res: IResult<&[u8], u32> = flat_take(2u8, be_u32)(input);
        assert_eq!(res, Err(Err::Incomplete(Needed::new(2))));
    }

    #[test]
    fn test_flat_take_str() {
        let input = "abcdef";
        // read first 2 bytes and use correct combinator: OK
        let res: IResult<&str, &str> = flat_take(2u8, take(2u8))(input);
        assert_eq!(res, Ok(("cdef", "ab")));
        // read 3 bytes and use 2: OK (some input is just lost)
        let res: IResult<&str, &str> = flat_take(3u8, take(2u8))(input);
        assert_eq!(res, Ok(("def", "ab")));
        // read 2 bytes and a use combinator requiring more bytes
        let res: IResult<&str, &str> = flat_take(2u8, take(4u8))(input);
        assert_eq!(res, Err(Err::Incomplete(Needed::Unknown)));
    }

    #[test]
    fn test_cond_else() {
        let input = &[0x01][..];
        let empty = &b""[..];
        let a = 1;
        fn parse_u8(i: &[u8]) -> IResult<&[u8], u8> {
            be_u8(i)
        }
        assert_eq!(
            cond_else(|| a == 1, parse_u8, pure(0x02))(input),
            Ok((empty, 0x01))
        );
        assert_eq!(
            cond_else(|| a == 1, parse_u8, pure(0x02))(input),
            Ok((empty, 0x01))
        );
        assert_eq!(
            cond_else(|| a == 2, parse_u8, pure(0x02))(input),
            Ok((input, 0x02))
        );
        assert_eq!(
            cond_else(|| a == 1, pure(0x02), parse_u8)(input),
            Ok((input, 0x02))
        );
        let res: IResult<&[u8], u8> = cond_else(|| a == 1, parse_u8, parse_u8)(input);
        assert_eq!(res, Ok((empty, 0x01)));
    }

    #[test]
    fn test_align32() {
        assert_eq!(align32(3), 4);
        assert_eq!(align32(4), 4);
        assert_eq!(align32(5), 8);
        assert_eq!(align32(5usize), 8);
    }
}
