use futures_util::StreamExt;
use static_assertions::assert_impl_all;
use std::{
    convert::{TryFrom, TryInto},
    ops::Deref,
    sync::Arc,
};
use zbus_names::{BusName, InterfaceName, MemberName, UniqueName};
use zvariant::{ObjectPath, OwnedValue, Value};

use crate::{blocking::Connection, utils::block_on, Error, Message, Result};

use crate::fdo;

/// A blocking wrapper of [`crate::Proxy`].
///
/// This API is mostly the same as [`crate::Proxy`], except that all its methods block to
/// completion.
///
/// # Example
///
/// ```
/// use std::result::Result;
/// use std::error::Error;
/// use zbus::blocking::{Connection, Proxy};
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let connection = Connection::session()?;
///     let p = Proxy::new(
///         &connection,
///         "org.freedesktop.DBus",
///         "/org/freedesktop/DBus",
///         "org.freedesktop.DBus",
///     )?;
///     // owned return value
///     let _id: String = p.call("GetId", &())?;
///     // borrowed return value
///     let _id: &str = p.call_method("GetId", &())?.body()?;
///     Ok(())
/// }
/// ```
///
/// # Note
///
/// It is recommended to use the [`dbus_proxy`] macro, which provides a more convenient and
/// type-safe *façade* `Proxy` derived from a Rust trait.
///
/// ## Current limitations:
///
/// At the moment, `Proxy` doesn't prevent [auto-launching][al].
///
/// [`dbus_proxy`]: attr.dbus_proxy.html
/// [al]: https://gitlab.freedesktop.org/dbus/zbus/-/issues/54
#[derive(derivative::Derivative)]
#[derivative(Clone, Debug)]
pub struct Proxy<'a> {
    #[derivative(Debug = "ignore")]
    conn: Connection,
    azync: crate::Proxy<'a>,
}

assert_impl_all!(Proxy<'_>: Send, Sync, Unpin);

impl<'a> Proxy<'a> {
    /// Create a new `Proxy` for the given destination/path/interface.
    pub fn new<D, P, I>(
        conn: &Connection,
        destination: D,
        path: P,
        interface: I,
    ) -> Result<Proxy<'a>>
    where
        D: TryInto<BusName<'a>>,
        P: TryInto<ObjectPath<'a>>,
        I: TryInto<InterfaceName<'a>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
    {
        let proxy = block_on(crate::Proxy::new(
            conn.inner(),
            destination,
            path,
            interface,
        ))?;

        Ok(Self {
            conn: conn.clone(),
            azync: proxy,
        })
    }

    /// Create a new `Proxy` for the given destination/path/interface, taking ownership of all
    /// passed arguments.
    pub fn new_owned<D, P, I>(
        conn: Connection,
        destination: D,
        path: P,
        interface: I,
    ) -> Result<Proxy<'a>>
    where
        D: TryInto<BusName<'static>>,
        P: TryInto<ObjectPath<'static>>,
        I: TryInto<InterfaceName<'static>>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
    {
        let proxy = block_on(crate::Proxy::new_owned(
            conn.clone().into_inner(),
            destination,
            path,
            interface,
        ))?;

        Ok(Self { conn, azync: proxy })
    }

    /// Get a reference to the associated connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get a reference to the destination service name.
    pub fn destination(&self) -> &BusName<'_> {
        self.azync.destination()
    }

    /// Get a reference to the object path.
    pub fn path(&self) -> &ObjectPath<'_> {
        self.azync.path()
    }

    /// Get a reference to the interface.
    pub fn interface(&self) -> &InterfaceName<'_> {
        self.azync.interface()
    }

    /// Introspect the associated object, and return the XML description.
    ///
    /// See the [xml](xml/index.html) module for parsing the result.
    pub fn introspect(&self) -> fdo::Result<String> {
        block_on(self.azync.introspect())
    }

    /// Get the cached value of the property `property_name`.
    ///
    /// This returns `None` if the property is not in the cache.  This could be because the cache
    /// was invalidated by an update, because caching was disabled for this property or proxy, or
    /// because the cache has not yet been populated.  Use `get_property` to fetch the value from
    /// the peer.
    pub fn cached_property<T>(&self, property_name: &str) -> Result<Option<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>,
    {
        self.azync.cached_property(property_name)
    }

    /// Get the cached value of the property `property_name`.
    ///
    /// Same as `cached_property`, but gives you access to the raw value stored in the cache. This
    /// is useful if you want to avoid allocations and cloning.
    pub fn cached_property_raw<'p>(
        &'p self,
        property_name: &'p str,
    ) -> Option<impl Deref<Target = Value<'static>> + 'p> {
        self.azync.cached_property_raw(property_name)
    }

    /// Get the property `property_name`.
    ///
    /// Get the property value from the cache or call the `Get` method of the
    /// `org.freedesktop.DBus.Properties` interface.
    pub fn get_property<T>(&self, property_name: &str) -> Result<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<Error>,
    {
        block_on(self.azync.get_property(property_name))
    }

    /// Set the property `property_name`.
    ///
    /// Effectively, call the `Set` method of the `org.freedesktop.DBus.Properties` interface.
    pub fn set_property<'t, T: 't>(&self, property_name: &str, value: T) -> fdo::Result<()>
    where
        T: Into<Value<'t>>,
    {
        block_on(self.azync.set_property(property_name, value))
    }

    /// Call a method and return the reply.
    ///
    /// Typically, you would want to use [`call`] method instead. Use this method if you need to
    /// deserialize the reply message manually (this way, you can avoid the memory
    /// allocation/copying, by deserializing the reply to an unowned type).
    ///
    /// [`call`]: struct.Proxy.html#method.call
    pub fn call_method<'m, M, B>(&self, method_name: M, body: &B) -> Result<Arc<Message>>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        block_on(self.azync.call_method(method_name, body))
    }

    /// Call a method and return the reply body.
    ///
    /// Use [`call_method`] instead if you need to deserialize the reply manually/separately.
    ///
    /// [`call_method`]: struct.Proxy.html#method.call_method
    pub fn call<'m, M, B, R>(&self, method_name: M, body: &B) -> Result<R>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
        R: serde::de::DeserializeOwned + zvariant::Type,
    {
        block_on(self.azync.call(method_name, body))
    }

    /// Call a method without expecting a reply
    ///
    /// This sets the `NoReplyExpected` flag on the calling message and does not wait for a reply.
    pub fn call_noreply<'m, M, B>(&self, method_name: M, body: &B) -> Result<()>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + zvariant::DynamicType,
    {
        block_on(self.azync.call_noreply(method_name, body))
    }

    /// Create a stream for signal named `signal_name`.
    ///
    /// # Errors
    ///
    /// Apart from general I/O errors that can result from socket communications, calling this
    /// method will also result in an error if the destination service has not yet registered its
    /// well-known name with the bus (assuming you're using the well-known name as destination).
    pub fn receive_signal<'m: 'a, M>(&self, signal_name: M) -> Result<SignalIterator<'a>>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
    {
        block_on(self.azync.receive_signal(signal_name)).map(SignalIterator)
    }

    /// Create a stream for all signals emitted by this service.
    ///
    /// # Errors
    ///
    /// Apart from general I/O errors that can result from socket communications, calling this
    /// method will also result in an error if the destination service has not yet registered its
    /// well-known name with the bus (assuming you're using the well-known name as destination).
    pub fn receive_all_signals(&self) -> Result<SignalIterator<'a>> {
        block_on(self.azync.receive_all_signals()).map(SignalIterator)
    }

    /// Get an iterator to receive owner changed events.
    ///
    /// If the proxy destination is a unique name, the stream will be notified of the peer
    /// disconnection from the bus (with a `None` value).
    ///
    /// If the proxy destination is a well-known name, the stream will be notified whenever the name
    /// owner is changed, either by a new peer being granted ownership (`Some` value) or when the
    /// name is released (with a `None` value).
    ///
    /// Note that zbus doesn't queue the updates. If the listener is slower than the receiver, it
    /// will only receive the last update.
    pub fn receive_property_changed<'name: 'a, T>(
        &self,
        name: &'name str,
    ) -> PropertyIterator<'a, T> {
        PropertyIterator(block_on(self.azync.receive_property_changed(name)))
    }

    /// Get an iterator to receive property changed events.
    ///
    /// Note that zbus doesn't queue the updates. If the listener is slower than the receiver, it
    /// will only receive the last update.
    pub fn receive_owner_changed(&self) -> Result<OwnerChangedIterator<'_>> {
        block_on(self.azync.receive_owner_changed()).map(OwnerChangedIterator)
    }

    /// Get a reference to the underlying async Proxy.
    pub fn inner(&self) -> &crate::Proxy<'a> {
        &self.azync
    }

    /// Get the underlying async Proxy, consuming `self`.
    pub fn into_inner(self) -> crate::Proxy<'a> {
        self.azync
    }
}

impl<'a> std::convert::AsRef<Proxy<'a>> for Proxy<'a> {
    fn as_ref(&self) -> &Proxy<'a> {
        self
    }
}

impl<'a> From<crate::Proxy<'a>> for Proxy<'a> {
    fn from(proxy: crate::Proxy<'a>) -> Self {
        Self {
            conn: proxy.connection().clone().into(),
            azync: proxy,
        }
    }
}

/// An [`std::iter::Iterator`] implementation that yields signal [messages](`Message`).
///
/// Use [`Proxy::receive_signal`] to create an instance of this type.
#[derive(Debug)]
pub struct SignalIterator<'a>(crate::SignalStream<'a>);

assert_impl_all!(SignalIterator<'_>: Send, Sync, Unpin);

impl std::iter::Iterator for SignalIterator<'_> {
    type Item = Arc<Message>;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(self.0.next())
    }
}

/// An [`std::iter::Iterator`] implementation that yields property change notifications.
///
/// Use [`Proxy::receive_property_changed`] to create an instance of this type.
pub struct PropertyIterator<'a, T>(crate::PropertyStream<'a, T>);

impl<'a, T> std::iter::Iterator for PropertyIterator<'a, T>
where
    T: Unpin,
{
    type Item = PropertyChanged<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(self.0.next()).map(PropertyChanged)
    }
}

/// A property changed event.
///
/// The property changed event generated by [`PropertyIterator`].
pub struct PropertyChanged<'a, T>(crate::PropertyChanged<'a, T>);

// split this out to avoid the trait bound on `name` method
impl<'a, T> PropertyChanged<'a, T> {
    /// Get the name of the property that changed.
    pub fn name(&self) -> &str {
        self.0.name()
    }

    // Get the raw value of the property that changed.
    //
    // If the notification signal contained the new value, it has been cached already and this call
    // will return that value. Otherwise (i-e invalidated property), a D-Bus call is made to fetch
    // and cache the new value.
    pub fn get_raw<'p>(&'p self) -> Result<impl Deref<Target = Value<'static>> + 'p> {
        block_on(self.0.get_raw())
    }
}

impl<'a, T> PropertyChanged<'a, T>
where
    T: TryFrom<zvariant::OwnedValue>,
    T::Error: Into<crate::Error>,
{
    // Get the value of the property that changed.
    //
    // If the notification signal contained the new value, it has been cached already and this call
    // will return that value. Otherwise (i-e invalidated property), a D-Bus call is made to fetch
    // and cache the new value.
    pub fn get(&self) -> Result<T> {
        block_on(self.0.get())
    }
}

/// An [`std::iter::Iterator`] implementation that yields owner change notifications.
///
/// Use [`Proxy::receive_owner_changed`] to create an instance of this type.
pub struct OwnerChangedIterator<'a>(crate::OwnerChangedStream<'a>);

impl OwnerChangedIterator<'_> {
    /// The bus name being tracked.
    pub fn name(&self) -> &BusName<'_> {
        self.0.name()
    }
}

impl<'a> std::iter::Iterator for OwnerChangedIterator<'a> {
    type Item = Option<UniqueName<'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        block_on(self.0.next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocking;
    use ntest::timeout;
    use test_log::test;

    #[test]
    #[timeout(15000)]
    fn signal() {
        // Register a well-known name with the session bus and ensure we get the appropriate
        // signals called for that.
        let conn = Connection::session().unwrap();

        let proxy = blocking::fdo::DBusProxy::new(&conn).unwrap();
        let owner_changed = proxy.receive_name_owner_changed().unwrap();
        let name_acquired = proxy.receive_name_acquired().unwrap();
        let well_known = "org.freedesktop.zbus.ProxySignalTest";
        let unique_name = conn.unique_name().unwrap().to_string();

        blocking::fdo::DBusProxy::new(&conn)
            .unwrap()
            .request_name(
                well_known.try_into().unwrap(),
                fdo::RequestNameFlags::ReplaceExisting.into(),
            )
            .unwrap();

        for signal in owner_changed {
            let args = signal.args().unwrap();
            if args.name() == well_known {
                // Meant for the this testcase.
                assert_eq!(*args.new_owner().as_ref().unwrap(), *unique_name);
                break;
            }
        }
        // `NameAcquired` is emitted twice, first when the unique name is assigned on
        // connection and secondly after we ask for a specific name.
        for signal in name_acquired {
            if signal.args().unwrap().name() == well_known {
                break;
            }
        }
    }
}
