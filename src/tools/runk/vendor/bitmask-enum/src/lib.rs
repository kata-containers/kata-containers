/*!
# Bitmask-Enum

A bitmask enum attribute macro, to turn an enum into a bitmask.

A bitmask can have (un)signed integer types, the default type is `usize`.

```
use bitmask_enum::bitmask;

#[bitmask] // usize
enum Bitmask { /* ... */ }

#[bitmask(u8)] // u8
enum BitmaskU8 { /* ... */ }
```

## Example

```
use bitmask_enum::bitmask;

#[bitmask(u8)]
enum Bitmask {
    Flag1, // defaults to 0d00000001
    Flag2, // defaults to 0d00000010
    Flag3, // defaults to 0d00000100
}

// It is possible to impl on the bitmask and use its
impl Bitmask {
    fn _set_to(&mut self, val: u8) {
        self.bits = val
    }
}

// bitmask has const bitwise operator methods
const CONST_BM: Bitmask = Bitmask::Flag2.or(Bitmask::Flag3);

println!("{:#010b}", CONST_BM); // 0b00000110

// Bitmask that contains Flag1 and Flag3
let bm = Bitmask::Flag1 | Bitmask::Flag3;

println!("{:#010b}", bm); // 0b00000101

// Does bm intersect one of CONST_BM
println!("{}", bm.intersects(CONST_BM)); // true

// Does bm contain all of CONST_BM
println!("{}", bm.contains(CONST_BM)); // false
```

## Custom Values

You can assign every flag a custom value.

```
use bitmask_enum::bitmask;

#[bitmask(u8)]
enum Bitmask {
    Flag5 = 0b00010000,
    Flag3 = 0b00000100,
    Flag1 = 0b00000001,

    Flag51_1 = 0b00010000 | 0b00000001,
    Flag51_2 = Self::Flag5.or(Self::Flag1).bits,
    Flag51_3 = Self::Flag5.bits | Self::Flag1.bits,

    Flag513 = {
        let flag51 = Self::Flag51_1.bits;
        flag51 | Self::Flag3.bits
    },
}

let bm = Bitmask::Flag5 | Bitmask::Flag1;

println!("{:#010b}", bm); // 0b00010001
println!("{}", bm == Bitmask::Flag51_1); // true

println!("{:#010b}", Bitmask::Flag513); // 0b00010101
```

## Implemented Methods
```ignore
// returns the underlying bits
const fn bits(&self) -> #type {

// contains all values
const fn all() -> Self;

// if self contains all values
const fn is_all(&self) -> bool;

// contains no value
const fn none() -> Self;

// if self contains no value
const fn is_none(&self) -> bool;

// self intersects one of the other
// (self & other) != 0 || other == 0
const fn intersects(&self, other: Self) -> bool;

// self contains all of the other
// (self & other) == other
const fn contains(&self, other: Self) -> bool;

// constant bitwise ops
const fn not(self) -> Self;
const fn and(self, other: Self) -> Self;
const fn or(self, other: Self) -> Self;
const fn xor(self, other: Self) -> Self;
```

## Implemented Traits
```ignore
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]

impl core::ops::Not;

impl core::ops::BitAnd;
impl core::ops::BitAndAssign;

impl core::ops::BitOr;
impl core::ops::BitOrAssign;

impl core::ops::BitXor;
impl core::ops::BitXorAssign;

impl From<#type> for #ident;
impl From<#ident> for #type;

impl PartialEq<#type>;

impl core::fmt::Binary;
impl core::fmt::LowerHex;
impl core::fmt::UpperHex;
impl core::fmt::Octal;
```
*/

mod parser;

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemEnum};

/**
# Bitmask-Enum

A bitmask can have unsigned integer types, the default type is `usize`.

```
use bitmask_enum::bitmask;

#[bitmask] // usize
enum Bitmask { /* ... */ }

#[bitmask(u8)] // u8
enum BitmaskU8 { /* ... */ }
```
*/
#[proc_macro_attribute]
pub fn bitmask(attr: TokenStream, item: TokenStream) -> TokenStream {
    parser::parse(attr, parse_macro_input!(item as ItemEnum))
}
