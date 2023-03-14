use std::{collections::HashSet, convert::TryInto, marker::PhantomData, sync::Arc};

use static_assertions::assert_impl_all;
use zbus_names::{BusName, InterfaceName};
use zvariant::{ObjectPath, Str};

use crate::{Connection, Error, Proxy, ProxyInner, Result};

/// The properties caching mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheProperties {
    /// Cache properties. The properties will be cached upfront as part of the proxy
    /// creation.
    Yes,
    /// Don't cache properties.
    No,
    /// Cache properties but only populate the cache on the first read of a property (default).
    Lazily,
}

impl Default for CacheProperties {
    fn default() -> Self {
        CacheProperties::Lazily
    }
}

/// Builder for proxies.
#[derive(Debug)]
pub struct ProxyBuilder<'a, T = ()> {
    conn: Connection,
    destination: Option<BusName<'a>>,
    path: Option<ObjectPath<'a>>,
    interface: Option<InterfaceName<'a>>,
    proxy_type: PhantomData<T>,
    cache: CacheProperties,
    uncached_properties: HashSet<Str<'a>>,
}

impl<'a, T> Clone for ProxyBuilder<'a, T> {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            destination: self.destination.clone(),
            path: self.path.clone(),
            interface: self.interface.clone(),
            cache: self.cache,
            uncached_properties: self.uncached_properties.clone(),
            proxy_type: PhantomData,
        }
    }
}

assert_impl_all!(ProxyBuilder<'_>: Send, Sync, Unpin);

impl<'a, T> ProxyBuilder<'a, T> {
    /// Create a new [`ProxyBuilder`] for the given connection.
    #[must_use]
    pub fn new_bare(conn: &Connection) -> Self {
        Self {
            conn: conn.clone(),
            destination: None,
            path: None,
            interface: None,
            cache: CacheProperties::default(),
            uncached_properties: HashSet::new(),
            proxy_type: PhantomData,
        }
    }
}

impl<'a, T> ProxyBuilder<'a, T> {
    /// Set the proxy destination address.
    pub fn destination<D>(mut self, destination: D) -> Result<Self>
    where
        D: TryInto<BusName<'a>>,
        D::Error: Into<Error>,
    {
        self.destination = Some(destination.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Set the proxy path.
    pub fn path<P>(mut self, path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'a>>,
        P::Error: Into<Error>,
    {
        self.path = Some(path.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Set the proxy interface.
    pub fn interface<I>(mut self, interface: I) -> Result<Self>
    where
        I: TryInto<InterfaceName<'a>>,
        I::Error: Into<Error>,
    {
        self.interface = Some(interface.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Set the properties caching mode.
    #[must_use]
    pub fn cache_properties(mut self, cache: CacheProperties) -> Self {
        self.cache = cache;
        self
    }

    /// Specify a set of properties (by name) which should be excluded from caching.
    #[must_use]
    pub fn uncached_properties(mut self, properties: &[&'a str]) -> Self {
        for prop_name in properties {
            self.uncached_properties.insert(Str::from(*prop_name));
        }
        self
    }

    pub(crate) fn build_internal(self) -> Proxy<'a> {
        let conn = self.conn;
        let destination = self.destination.expect("missing `destination`");
        let path = self.path.expect("missing `path`");
        let interface = self.interface.expect("missing `interface`");
        let cache = self.cache;
        let mut uncached_properties = self.uncached_properties;
        uncached_properties.shrink_to_fit();

        Proxy {
            inner: Arc::new(ProxyInner::new(
                conn,
                destination,
                path,
                interface,
                cache,
                uncached_properties,
            )),
        }
    }

    /// Build a proxy from the builder.
    ///
    /// # Panics
    ///
    /// Panics if the builder is lacking the necessary details to build a proxy.
    pub async fn build(self) -> Result<T>
    where
        T: From<Proxy<'a>>,
    {
        let cache_upfront = self.cache == CacheProperties::Yes;
        let proxy = self.build_internal();

        if cache_upfront {
            proxy
                .get_property_cache()
                .expect("properties cache not initialized")
                .ready()
                .await?;
        }

        Ok(proxy.into())
    }
}

impl<'a, T> ProxyBuilder<'a, T>
where
    T: ProxyDefault,
{
    /// Create a new [`ProxyBuilder`] for the given connection.
    #[must_use]
    pub fn new(conn: &Connection) -> Self {
        Self {
            conn: conn.clone(),
            destination: Some(BusName::from_static_str(T::DESTINATION).expect("invalid bus name")),
            path: Some(ObjectPath::from_static_str(T::PATH).expect("invalid default path")),
            interface: Some(
                InterfaceName::from_static_str(T::INTERFACE).expect("invalid interface name"),
            ),
            cache: CacheProperties::default(),
            uncached_properties: HashSet::new(),
            proxy_type: PhantomData,
        }
    }
}

/// Trait for the default associated values of a proxy.
///
/// The trait is automatically implemented by the [`dbus_proxy`] macro on your behalf, and may be
/// later used to retrieve the associated constants.
///
/// [`dbus_proxy`]: attr.dbus_proxy.html
pub trait ProxyDefault {
    const INTERFACE: &'static str;
    const DESTINATION: &'static str;
    const PATH: &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    #[ntest::timeout(15000)]
    fn builder() {
        crate::utils::block_on(builder_async());
    }

    async fn builder_async() {
        let conn = Connection::session().await.unwrap();

        let builder = ProxyBuilder::<Proxy<'_>>::new_bare(&conn)
            .destination("org.freedesktop.DBus")
            .unwrap()
            .path("/some/path")
            .unwrap()
            .interface("org.freedesktop.Interface")
            .unwrap()
            .cache_properties(CacheProperties::No);
        assert!(matches!(
            builder.clone().destination.unwrap(),
            BusName::Unique(_),
        ));
        let proxy = builder.build().await.unwrap();
        assert!(matches!(proxy.inner.destination, BusName::Unique(_)));
    }
}
