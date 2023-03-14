#[cfg(feature = "futures-01")]
mod futures_01;
#[cfg(feature = "futures-01")]
pub use self::futures_01::*;

#[cfg(feature = "futures_preview")]
mod futures_preview;
#[cfg(feature = "futures_preview")]
pub use self::futures_preview::*;

#[cfg(feature = "futures-03")]
mod futures_03;
#[cfg(feature = "futures-03")]
pub use self::futures_03::*;
