//! A general purpose crate for working with timeouts and delays with futures.
//!
//! # Examples
//!
//! ```no_run
//! # #[async_std::main]
//! # async fn main() {
//! use std::time::Duration;
//! use futures_timer::Delay;
//!
//! let now = Delay::new(Duration::from_secs(3)).await;
//! println!("waited for 3 secs");
//! # }
//! ```

#![deny(missing_docs)]
#![warn(missing_debug_implementations)]

#[cfg(not(all(target_arch = "wasm32", feature = "wasm-bindgen")))]
mod native;
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen"))]
mod wasm;

#[cfg(not(all(target_arch = "wasm32", feature = "wasm-bindgen")))]
pub use self::native::Delay;
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen"))]
pub use self::wasm::Delay;
