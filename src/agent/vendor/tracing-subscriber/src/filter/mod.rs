//! [`Layer`]s that control which spans and events are enabled by the wrapped
//! subscriber.
//!
//! This module contains a number of types that provide implementations of
//! various strategies for filtering which spans and events are enabled. For
//! details on filtering spans and events using [`Layer`]s, see the
//! [`layer` module's documentation].
//!
//! [`layer` module's documentation]: crate::layer#filtering-with-layers
//! [`Layer`]: crate::layer
mod directive;
#[cfg(feature = "env-filter")]
mod env;
mod filter_fn;
#[cfg(feature = "registry")]
mod layer_filters;
mod level;
pub mod targets;

pub use self::directive::ParseError;
pub use self::filter_fn::*;
#[cfg(not(feature = "registry"))]
pub(crate) use self::has_plf_stubs::*;

#[cfg(feature = "registry")]
#[cfg_attr(docsrs, doc(cfg(feature = "registry")))]
pub use self::layer_filters::*;
pub use self::level::{LevelFilter, ParseError as LevelParseError};

#[cfg(feature = "env-filter")]
#[cfg_attr(docsrs, doc(cfg(feature = "env-filter")))]
pub use self::env::*;

pub use self::targets::Targets;

/// Stub implementations of the per-layer-fitler detection functions for when the
/// `registry` feature is disabled.
#[cfg(not(feature = "registry"))]
mod has_plf_stubs {
    pub(crate) fn is_plf_downcast_marker(_: std::any::TypeId) -> bool {
        false
    }

    /// Does a type implementing `Subscriber` contain any per-layer filters?
    pub(crate) fn subscriber_has_plf<S>(_: &S) -> bool
    where
        S: tracing_core::Subscriber,
    {
        false
    }

    /// Does a type implementing `Layer` contain any per-layer filters?
    pub(crate) fn layer_has_plf<L, S>(_: &L) -> bool
    where
        L: crate::Layer<S>,
        S: tracing_core::Subscriber,
    {
        false
    }
}
