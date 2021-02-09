// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


#[doc(hidden)]
pub trait NetInt {
    fn from_be(i: Self) -> Self;
    fn to_be(&self) -> Self;
}
macro_rules! doit {
    ($($t:ident)*) => ($(impl NetInt for $t {
        fn from_be(i: Self) -> Self { <$t>::from_be(i) }
        fn to_be(&self) -> Self { <$t>::to_be(*self) }
    })*)
}
doit! { i8 i16 i32 i64 isize u8 u16 u32 u64 usize }

#[doc(hidden)]
pub trait One {
    fn one() -> Self;
}

macro_rules! one {
    ($($t:ident)*) => ($(
        impl One for $t { fn one() -> $t { 1 } }
    )*)
}

one! { i8 i16 i32 i64 isize u8 u16 u32 u64 usize }


#[doc(hidden)]
pub trait Zero {
    fn zero() -> Self;
}

macro_rules! zero {
    ($($t:ident)*) => ($(
        impl Zero for $t { fn zero() -> $t { 0 } }
    )*)
}

zero! { i8 i16 i32 i64 isize u8 u16 u32 u64 usize }

