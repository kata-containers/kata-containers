#[cfg(feature = "dynamic-keys")]
mod dynamic;
#[cfg(feature = "dynamic-keys")]
pub use self::dynamic::Key;

#[cfg(not(feature = "dynamic-keys"))]
#[path = "static.rs"]
mod static_;
#[cfg(not(feature = "dynamic-keys"))]
pub use self::static_::Key;
