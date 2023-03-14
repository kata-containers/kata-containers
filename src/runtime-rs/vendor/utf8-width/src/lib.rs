/*!
# UTF-8 Width

To determine the width of a UTF-8 character by providing its first byte.

References: https://tools.ietf.org/html/rfc3629

## Examples

```rust
assert_eq!(1, utf8_width::get_width(b'1'));
assert_eq!(3, utf8_width::get_width("中".as_bytes()[0]));
```

## Benchmark

```bash
cargo bench
```

*/

#![no_std]

pub const MIN_0_1: u8 = 0x80;
pub const MAX_0_1: u8 = 0xC1;
pub const MIN_0_2: u8 = 0xF5;
pub const MAX_0_2: u8 = 0xFF;
pub const MIN_1: u8 = 0x00;
pub const MAX_1: u8 = 0x7F;
pub const MIN_2: u8 = 0xC2;
pub const MAX_2: u8 = 0xDF;
pub const MIN_3: u8 = 0xE0;
pub const MAX_3: u8 = 0xEF;
pub const MIN_4: u8 = 0xF0;
pub const MAX_4: u8 = 0xF4;

#[inline]
pub fn is_width_1(byte: u8) -> bool {
    byte <= MAX_1 // no need to check `MIN_1 <= byte`
}

#[inline]
pub fn is_width_2(byte: u8) -> bool {
    (MIN_2..=MAX_2).contains(&byte)
}

#[inline]
pub fn is_width_3(byte: u8) -> bool {
    (MIN_3..=MAX_3).contains(&byte)
}

#[inline]
pub fn is_width_4(byte: u8) -> bool {
    (MIN_4..=MAX_4).contains(&byte)
}

#[inline]
pub fn is_width_0(byte: u8) -> bool {
    (MIN_0_1..=MAX_0_1).contains(&byte) || MIN_0_2 <= byte // no need to check `byte <= MAX_0_2`
}

/// Given a first byte, determines how many bytes are in this UTF-8 character. If the UTF-8 character is invalid, returns `0`, otherwise returns `1` ~ `4`,
#[inline]
pub fn get_width(byte: u8) -> usize {
    if is_width_1(byte) {
        1
    } else if is_width_2(byte) {
        2
    } else if byte <= MAX_3 {
        // no need to check `MIN_3 <= byte`
        3
    } else if byte <= MAX_4 {
        // no need to check `MIN_4 <= byte`
        4
    } else {
        0
    }
}

#[allow(clippy::missing_safety_doc)]
/// *Assume the input first byte is from a valid UTF-8 character.* Given a first byte, determines how many bytes are in this UTF-8 character. It returns `1` ~ `4`,
#[inline]
pub unsafe fn get_width_assume_valid(byte: u8) -> usize {
    if byte <= MAX_1 {
        1
    } else if byte <= MAX_2 {
        2
    } else if byte <= MAX_3 {
        3
    } else {
        4
    }
}
