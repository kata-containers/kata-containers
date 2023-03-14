/*!
# Byte Unit

A library for interaction with units of bytes. The units are **B** for 1 byte, **KB** for 1000 bytes, **KiB** for 1024 bytes, **MB** for 1000000 bytes, **MiB** for 1048576 bytes, etc, and up to **ZiB** which is 1180591620717411303424 bytes.

The data type for storing the size of bytes is `u128` by default, but can also be changed to `u64` by disabling the default features (it will also cause the highest supported unit down to **PiB**).

## Usage

### Macros

There are `n_*_bytes` macros can be used. The star `*` means the unit. For example, `n_gb_bytes` can be used to get a **n-GB** value in bytes.

```rust
#[macro_use] extern crate byte_unit;

let result = n_gb_bytes!(4);

assert_eq!(4000000000, result);
```

You may need to assign a primitive type if the `n` is not an integer.

```rust
#[macro_use] extern crate byte_unit;

let result = n_gb_bytes!(2.5, f64);

assert_eq!(2500000000, result);
```

### Byte

The `Byte` structure can be used for representing a size of bytes.

The `from_str` associated function can parse any **SIZE** string and return a `Byte` instance in common usage. The format of a **SIZE** string is like "123", "123KiB" or "50.84 MB".

```rust
extern crate byte_unit;

use byte_unit::Byte;

let result = Byte::from_str("50.84 MB").unwrap();

assert_eq!(50840000, result.get_bytes());
```

You can also use the `from_bytes` and `from_unit` associated functions to create a `Byte` instance.

```rust
extern crate byte_unit;

use byte_unit::Byte;

let result = Byte::from_bytes(1500000);

assert_eq!(1500000, result.get_bytes());
```

```rust
extern crate byte_unit;

use byte_unit::{Byte, ByteUnit};

let result = Byte::from_unit(1500f64, ByteUnit::KB).unwrap();

assert_eq!(1500000, result.get_bytes());
```

### AdjustedByte

To change the unit of a `Byte` instance, you can use the `get_adjusted_unit` method.

```rust
extern crate byte_unit;

use byte_unit::{Byte, ByteUnit};

let byte = Byte::from_str("123KiB").unwrap();

let adjusted_byte = byte.get_adjusted_unit(ByteUnit::KB);

assert_eq!("125.95 KB", adjusted_byte.to_string());
```

To change the unit of a `Byte` instance automatically and appropriately, you can use the `get_appropriate_unit` method.

```rust
extern crate byte_unit;

use byte_unit::Byte;

let byte = Byte::from_bytes(1500000);

let adjusted_byte = byte.get_appropriate_unit(false);

assert_eq!("1.50 MB", adjusted_byte.to_string());
```

```rust
extern crate byte_unit;

use byte_unit::Byte;

let byte = Byte::from_bytes(1500000);

let adjusted_byte = byte.get_appropriate_unit(true);

assert_eq!("1.43 MiB", adjusted_byte.to_string());
```

The number of fractional digits created by the `to_string` method of a `AdjustedByte` instance is `2` unless the `ByteUnit` is `B`.

To change the number of fractional digits in the formatted string, you can use the `format` method instead.

```rust
extern crate byte_unit;

use byte_unit::Byte;

let byte = Byte::from_bytes(1500000);

let adjusted_byte = byte.get_appropriate_unit(false);

assert_eq!("1.5 MB", adjusted_byte.format(1));
```
*/

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

#[cfg(feature = "u128")]
#[macro_use]
mod u128;

#[cfg(not(feature = "u128"))]
#[macro_use]
mod u64;

#[macro_use]
mod macros;

mod adjusted_byte;
mod byte;
mod byte_error;
mod byte_unit;

#[cfg(feature = "u128")]
pub use self::u128::*;

#[cfg(not(feature = "u128"))]
pub use self::u64::*;

pub use self::byte_unit::*;
pub use adjusted_byte::*;
pub use byte::*;
pub use byte_error::*;
