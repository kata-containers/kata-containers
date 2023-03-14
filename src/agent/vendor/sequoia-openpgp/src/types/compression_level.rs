//! Common code for the compression writers.

use crate::{
    Error,
    Result,
};

/// Compression level.
///
/// This value is used by the encoders to tune their compression
/// strategy.  The level is restricted to levels commonly used by
/// compression libraries, `0` to `9`, where `0` means no compression,
/// `1` means fastest compression, `6` being a good default, and
/// meaning `9` best compression.
///
/// Note that compression is [dangerous when used naively].
///
/// [dangerous when used naively]: https://mailarchive.ietf.org/arch/msg/openpgp/2FQUVt6Dw8XAsaMELyo5BNlh2pM
#[cfg_attr(feature = "compression-deflate", doc = r##"
To mitigate some of these issues messages should [use padding].

[use padding]: crate::serialize::stream::padding

# Examples

Write a message using the given [CompressionAlgorithm]:

[CompressionAlgorithm]: super::CompressionAlgorithm

```
use sequoia_openpgp as openpgp;
# fn main() -> openpgp::Result<()> {
use std::io::Write;
use openpgp::serialize::stream::{Message, Compressor, LiteralWriter};
use openpgp::serialize::stream::padding::Padder;
use openpgp::types::{CompressionAlgorithm, CompressionLevel};

let mut sink = Vec::new();
let message = Message::new(&mut sink);
let message = Compressor::new(message)
    .algo(CompressionAlgorithm::Zlib)
#   .algo(CompressionAlgorithm::Uncompressed)
    .level(CompressionLevel::fastest())
    .build()?;

let message = Padder::new(message).build()?;

let mut message = LiteralWriter::new(message).build()?;
message.write_all(b"Hello world.")?;
message.finalize()?;
# Ok(()) }
```
"##)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompressionLevel(u8);
assert_send_and_sync!(CompressionLevel);

impl Default for CompressionLevel {
    fn default() -> Self {
        Self(6)
    }
}

impl CompressionLevel {
    /// Creates a new compression level.
    ///
    /// `level` must be in range `0..10`, where `0` means no
    /// compression, `1` means fastest compression, `6` being a good
    /// default, and meaning `9` best compression.
    pub fn new(level: u8) -> Result<CompressionLevel> {
        if level < 10 {
            Ok(Self(level))
        } else {
            Err(Error::InvalidArgument(
                format!("compression level out of range: {}", level)).into())
        }
    }

    /// No compression.
    pub fn none() -> CompressionLevel {
        Self(0)
    }

    /// Fastest compression.
    pub fn fastest() -> CompressionLevel {
        Self(1)
    }
    /// Best compression.
    pub fn best() -> CompressionLevel {
        Self(9)
    }
}

#[cfg(feature = "compression-deflate")]
mod into_deflate_compression {
    use flate2::Compression;
    use super::*;

    impl From<CompressionLevel> for Compression {
        fn from(l: CompressionLevel) -> Self {
            Compression::new(l.0 as u32)
        }
    }
}

#[cfg(feature = "compression-bzip2")]
mod into_bzip2_compression {
    use bzip2::Compression;
    use super::*;

    impl From<CompressionLevel> for Compression {
        fn from(l: CompressionLevel) -> Self {
            Compression::new(l.0 as u32)
        }
    }
}
