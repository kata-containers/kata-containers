use std::convert::TryInto;

use static_assertions::assert_impl_all;
use zbus_names::{BusName, InterfaceName};
use zvariant::ObjectPath;

use crate::{blocking::Connection, utils::block_on, CacheProperties, Error, Result};

pub use crate::ProxyDefault;

/// Builder for proxies.
#[derive(Debug, Clone)]
pub struct ProxyBuilder<'a, T = ()>(crate::ProxyBuilder<'a, T>);

assert_impl_all!(ProxyBuilder<'_>: Send, Sync, Unpin);

impl<'a, T> ProxyBuilder<'a, T> {
    /// Create a new [`ProxyBuilder`] for the given connection.
    #[must_use]
    pub fn new_bare(conn: &Connection) -> Self {
        Self(crate::ProxyBuilder::new_bare(&conn.clone().into()))
    }
}

impl<'a, T> ProxyBuilder<'a, T> {
    /// Set the proxy destination address.
    pub fn destination<D>(self, destination: D) -> Result<Self>
    where
        D: TryInto<BusName<'a>>,
        D::Error: Into<Error>,
    {
        crate::ProxyBuilder::destination(self.0, destination).map(Self)
    }

    /// Set the proxy path.
    pub fn path<P>(self, path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'a>>,
        P::Error: Into<Error>,
    {
        crate::ProxyBuilder::path(self.0, path).map(Self)
    }

    /// Set the proxy interface.
    pub fn interface<I>(self, interface: I) -> Result<Self>
    where
        I: TryInto<InterfaceName<'a>>,
        I::Error: Into<Error>,
    {
        crate::ProxyBuilder::interface(self.0, interface).map(Self)
    }

    /// Set whether to cache properties.
    #[must_use]
    pub fn cache_properties(self, cache: CacheProperties) -> Self {
        Self(self.0.cache_properties(cache))
    }

    /// Specify a set of properties (by name) which should be excluded from caching.
    #[must_use]
    pub fn uncached_properties(self, properties: &[&'a str]) -> Self {
        Self(self.0.uncached_properties(properties))
    }

    /// Build a proxy from the builder.
    ///
    /// # Panics
    ///
    /// Panics if the builder is lacking the necessary details to build a proxy.
    pub fn build(self) -> Result<T>
    where
        T: From<crate::Proxy<'a>>,
    {
        block_on(self.0.build())
    }
}

impl<'a, T> ProxyBuilder<'a, T>
where
    T: ProxyDefault,
{
    /// Create a new [`ProxyBuilder`] for the given connection.
    #[must_use]
    pub fn new(conn: &Connection) -> Self {
        Self(crate::ProxyBuilder::new(&conn.clone().into()))
    }
}
