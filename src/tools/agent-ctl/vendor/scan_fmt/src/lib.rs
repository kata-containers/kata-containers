// Copyright 2015-2019 Will Lentz.
// Licensed under the MIT license.

//! This crate provides a simple sscanf()-like interface to extract
//! data from strings and stdin.
//!
//! In version 0.2 scan_fmt! changed to return a Result.
//! Use scan_fmt_some! for the 0.1.x behavior.
//!
//! To use this crate, do:
//!
//! ```ignore
//! #[macro_use] extern crate scan_fmt;
//! ```
//!
//! Example to read from a string:
//!
//! ```rust
//! # #[macro_use] extern crate scan_fmt;
//! # fn main() {
//!   if let Ok((a,b)) = scan_fmt!( "-11 0x22", // input string
//!                                 "{d} {x}",  // format
//!                                 i8, [hex u8]) { // types
//!     assert_eq!( a, -11 ) ;
//!     assert_eq!( b, 0x22 ) ;
//!   }
//!
//!   let (a,b,c) = scan_fmt_some!( "hello 12 345 bye", // input string
//!                                 "hello {} {d} {}",  // format
//!                                 u8, i32, String);   // type of a-c Options
//!   assert_eq!( a, Some(12) ) ;
//!   assert_eq!( b, Some(345) ) ;
//!   assert_eq!( c, Some("bye".into()) ) ;
//! # }
//! ```
//!
//! Special format_string tokens:
//! <pre class="rust">
//!   {{ = escape for '{'
//!   }} = escape for '}'
//!   {} = return any value (until next whitespace)
//!   {d} = return base-10 decimal
//!   {x} = return hex (0xab or ab)
//!       = you must wrap the type in [hex type], e.g. "[hex u32]"
//!   {f} = return float
//!   {*d} = "*" as the first character means "match but don't return"
//!   {2d} or {2x} or {2f} = limit the maximum width to 2.  Any positive integer works.
//!   {[...]} = return pattern.
//!     ^ inverts if it is the first character
//!     - is for ranges.  For a literal - put it at the start or end.
//!     To add a literal ] do "[]abc]"
//!   {e} = doesn't return a value, but matches end of line.  Use this if you
//!         don't want to ignore potential extra characters at end of input.
//!   Examples:
//!     {[0-9ab]} = match 0-9 or a or b
//!     {[^,.]} = match anything but , or .
//!     {/.../} = return regex inside of `//`. (if regex feature is installed)
//!      If there is a single capture group inside of the slashes then
//!      that group will make up the pattern.
//!   Examples:
//!     {/[0-9ab]/} = same as {[0-9ab]}, above
//!     {/a+/} = matches at least one `a`, greedily
//!     {/jj(a*)jj/} = matches any number of `a`s, but only if
//!       they're surrounded by two `j`s
//! </pre>
//!
//! Example to read from stdin:
//!
//! ```ignore
//! # #[macro_use] extern crate scan_fmt;
//! # use std::error::Error ;
//! # fn main() -> Result<(),Box<dyn Error>> {
//!     let (a,b) = scanln_fmt!( "{}-{}", u16, u8) ? ;
//!     println!("Got {} and {}",a,b);
//!
//!     let (a,b) = scanln_fmt_some!( "{}-{}",   // format
//!                                  u16, u8);    // type of a&b Options
//!     match (a,b) {
//!       (Some(aa),Some(bb)) => println!("Got {} and {}",aa,bb),
//!       _ => println!("input error")
//!     }
//!     Ok(())
//! # }
//! ```
//!
//! ## LIMITATIONS:
//! There are no compile-time checks to make sure the format
//! strings matches the number of return arguments.  Extra
//! return values will be None or cause a Result error.
//!
//! Like sscanf(), whitespace (including \n) is largely ignored.
//!
//! Conversion to output values is done using parse::<T>().

#![no_std]

#[cfg(feature = "regex")]
extern crate regex;

#[cfg(any(test, doctest, feature = "std"))]
extern crate std;

#[macro_use]
extern crate alloc;

pub mod parse;

#[macro_export]
macro_rules! scan_fmt_help {
    ( wrap $res:expr, [hex $arg:tt] ) => {
        match $res.next() {
            Some(item) => $arg::from_str_radix(&item, 16).ok(),
            _ => None,
        }
    };
    ( wrap $res:expr , $($arg1:tt)::* ) => {
        match $res.next() {
            Some(item) => item.parse::<$($arg1)::*>().ok(),
            _ => None,
        }
    };
    ( no_wrap $err:ident, $res:expr, [hex $arg:tt] ) => {
        match $res.next() {
            Some(item) => {
                let ret = $arg::from_str_radix(&item, 16);
                if ret.is_err() {
                    $err = "from_str_radix hex";
                }
                ret.unwrap_or(0)
            }
            _ => {
                $err = "internal hex";
                0
            }
        }
    };
    ( no_wrap $err:ident, $res:expr , $($arg1:tt)::* ) => {{
        // We need to return a value of type $($arg1)::* if parsing fails.
        // Is there a better way?
        let mut err = "0".parse::<$($arg1)::*>(); // most types
        if err.is_err() {
           err = "0.0.0.0".parse::<$($arg1)::*>(); // IpAddr
        }
        let err = err.unwrap();
        match $res.next() {
            Some(item) => {
                let ret = item.parse::<$($arg1)::*>();
                if(item == "") {
                    $err = "match::none";
                } else if ret.is_err() {
                    $err = concat!("parse::", stringify!($($arg1)::*));
                }
                ret.unwrap_or(err)
            }
            _ => {
                $err = concat!("internal ", stringify!($($arg1)::*));
                err
            }
        }
    }};
}

#[macro_export]
macro_rules! scan_fmt_some {
    ( $instr:expr, $fmt:expr, $($($args:tt)::*),* ) => {
        {
            let mut res = $crate::parse::scan( $instr, $fmt ) ;
            ($($crate::scan_fmt_help!(wrap res,$($args)::*)),*)
        }
    };
}

#[macro_export]
macro_rules! scan_fmt {
    ( $instr:expr, $fmt:expr, $($($args:tt)::*),* ) => {
        {
            let mut err = "" ;
            let mut res = $crate::parse::scan( $instr, $fmt ) ;
            let result = ($($crate::scan_fmt_help!(no_wrap err,res,$($args)::*)),*) ;
            if err == "" {
                Ok(result)
            } else {
                Err($crate::parse::ScanError(err.into()))
            }
        }
    };
}

#[cfg(feature = "std")]
pub use std_features::*;

#[cfg(feature = "std")]
mod std_features {
    use std::string::String;

    pub fn get_input_unwrap() -> String {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        input
    }

    /// (a,+) = scanln_fmt!( format_string, types,+ )
    /// <p>Same as scan_fmt!(), but reads input string from stdin.</p>
    #[macro_export]
    macro_rules! scanln_fmt {
        ($($arg:tt)*) => {{ scan_fmt!(&$crate::get_input_unwrap(), $($arg)*) }}
    }

    /// (a,+) = scanln_fmt_some!( format_string, types,+ )
    /// <p>Same as scan_fmt_some!(), but reads input string from stdin.</p>
    #[macro_export]
    macro_rules! scanln_fmt_some {
        ($($arg:tt)*) => {{ scan_fmt_some!(&$crate::get_input_unwrap(), $($arg)*) }}
    }
}

#[cfg(test)]
use alloc::string::{String, ToString};
#[cfg(test)]
use parse::ScanError;

#[cfg(test)]
macro_rules! assert_flt_eq {
    ($t:ident, $v1:expr, $v2:expr) => {{
        assert!(($v1 - $v2).abs() <= 2.0 * std::$t::EPSILON);
    }};
}

#[cfg(test)]
fn ret_scan_all() -> Result<(), ScanError> {
    let (a, b) = scan_fmt!("1.2 e","{f} {x}",f32,[hex u32])?;
    assert_flt_eq!(f32, a, 1.2);
    assert_eq!(b, 14);
    Ok(())
}

#[test]
fn test_scan_all() {
    if let Ok(a) = scan_fmt!("hi1 3", "{} {d}", std::string::String, u32) {
        assert_eq!(a, ("hi1".to_string(), 3));
    } else {
        assert!(false, "error 0");
    }
    if let Ok((a, b, c)) = scan_fmt!("hi1 0xf -3","{} {x} {d}",String,[hex u32],i8) {
        assert_eq!(a, "hi1");
        assert_eq!(b, 0xf);
        assert_eq!(c, -3);
    } else {
        assert!(false, "error 1");
    }
    let a = scan_fmt!("hi1 f", "{} {d}", String, i32);
    assert!(a.is_err());
    let a = ret_scan_all();
    std::println!("{:?}", a);
    assert!(a.is_ok());
}

#[test]
fn test_plus_sign() {
    let a = scan_fmt_some!("+42", "{d}", i32);
    assert_eq!(a, Some(42));
    let a = scan_fmt_some!("+42.0", "{f}", f64);
    assert_flt_eq!(f64, a.unwrap(), 42.0);
}

#[test]
fn test_hex() {
    let (a, b, c) =
        scan_fmt_some!("DEV 0xab 0x1234", "{} {x} {x}", std::string::String, [hex u32], [hex u64]);
    assert_eq!(a, Some("DEV".into()));
    assert_eq!(b, Some(0xab));
    assert_eq!(c, Some(0x1234));
}

#[test]
fn test_limited_data_range() {
    let (a, b, c) = scan_fmt_some!(
        "test{\t 1e9 \n bye 257} hi  22.7e-1",
        "test{{ {} bye {d}}} hi {f}",
        f64,
        u8,
        f32
    );
    assert_flt_eq!(f64, a.unwrap(), 1e9);
    assert_eq!(b, None); // 257 doesn't fit into a u8
    assert_flt_eq!(f32, c.unwrap(), 2.27);
}

#[test]
fn test_too_many_outputs() {
    let (a, b, c, d) = scan_fmt_some!("a_aa bb_b c", "{} {s} {}", String, String, String, String);
    assert_eq!(a.unwrap(), "a_aa");
    assert_eq!(b.unwrap(), "bb_b");
    assert_eq!(c.unwrap(), "c");
    assert_eq!(d, None);
}

#[test]
fn test_skip_assign() {
    let (a, b) = scan_fmt_some!("1 2 3, 4 5, 6 7", "{[^,]},{*[^,]},{[^,]}", String, String);
    assert_eq!(a.unwrap(), "1 2 3");
    assert_eq!(b.unwrap(), "6 7");
    let a = scan_fmt!("1 2 3, 4 5, 6 7", "{[^,]},{*[^,]},{[^,]}", String, String).unwrap();
    assert_eq!(a.0, "1 2 3");
    assert_eq!(a.1, "6 7");
}

#[test]
fn test_width_specifier() {
    let a = scan_fmt!("123ab71 2.1234",
                      "{1d}{2d}{3x}{4d}{3f}",
                      u8, u8, [hex u16], u16, f32)
    .unwrap();
    assert_eq!(a.0, 1);
    assert_eq!(a.1, 23);
    assert_eq!(a.2, 0xab7);
    assert_eq!(a.3, 1);
    assert_flt_eq!(f32, a.4, 2.1);
}

#[test]
fn test_err_equals() {
    let a = scan_fmt!("hi 123", "hi {d", u8);
    assert_eq!(a, Err(parse::ScanError("internal u8".to_string())));
}

#[test]
fn test_no_post_match_regex() {
    let a = scan_fmt!("74in", "{d}{/in/}", u8, String);
    assert_eq!(a, Ok((74, String::from("in"))));
    let a = scan_fmt!("74in", "{d}{/cm/}", u8, String);
    assert_eq!(a, Err(parse::ScanError("match::none".to_string())));
}

#[test]
fn test_no_post_match() {
    let a = scan_fmt!("17in", "{d}in", u8);
    assert_eq!(a, Ok(17u8));

    let a = scan_fmt!("17in", "{d}cm", u8);
    assert_eq!(a, Err(parse::ScanError("match::none".to_string())));
}

#[test]
fn test_match_end() {
    let a = scan_fmt!("17in", "{d}in{e}", u8);
    assert_eq!(a, Ok(17u8));
    let a = scan_fmt!("17ink", "{d}in{e}", u8);
    assert_eq!(a, Err(parse::ScanError("match::none".to_string())));
}

#[test]
fn test_ip_addr() {
    let a = scan_fmt!("x 185.187.165.163 y", "x {} y", std::net::IpAddr);
    assert_eq!(
        a.unwrap(),
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(185, 187, 165, 163))
    );
}
