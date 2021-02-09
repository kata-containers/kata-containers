// Re-export the io::Result / Error types for convenience
pub use std::io::{Read, Write, Result, Error, ErrorKind};

// TODO: Delete this
/// A helper trait to provide the map_non_block function on Results.
pub trait MapNonBlock<T> {
    /// Maps a `Result<T>` to a `Result<Option<T>>` by converting
    /// operation-would-block errors into `Ok(None)`.
    fn map_non_block(self) -> Result<Option<T>>;
}

impl<T> MapNonBlock<T> for Result<T> {
    fn map_non_block(self) -> Result<Option<T>> {
        use std::io::ErrorKind::WouldBlock;

        match self {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                if let WouldBlock = err.kind() {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }
}

#[cfg(feature = "with-deprecated")]
pub mod deprecated {
    /// Returns a std `WouldBlock` error without allocating
    pub fn would_block() -> ::std::io::Error {
        ::std::io::ErrorKind::WouldBlock.into()
    }
}
