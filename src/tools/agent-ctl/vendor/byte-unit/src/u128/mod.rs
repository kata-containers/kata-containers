mod constants;

pub use constants::*;

#[doc(hidden)]
#[macro_export]
macro_rules! _bytes_as {
    ($x:expr) => {
        $x as u128
    };
}
