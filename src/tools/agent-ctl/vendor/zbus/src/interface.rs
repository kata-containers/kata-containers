use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Write,
    future::Future,
    pin::Pin,
};

use async_trait::async_trait;
use zbus_names::{InterfaceName, MemberName};
use zvariant::{DynamicType, OwnedValue, Value};

use crate::{fdo, Connection, Message, ObjectServer, Result, SignalContext};

/// A helper type returned by [`Interface`] callbacks.
pub enum DispatchResult<'a> {
    /// This interface does not support the given method
    NotFound,

    /// Retry with [Interface::call_mut].
    ///
    /// This is equivalent to NotFound if returned by call_mut.
    RequiresMut,

    /// The method was found and will be completed by running this Future
    Async(Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>),
}

impl<'a> DispatchResult<'a> {
    /// Helper for creating the Async variant
    pub fn new_async<F, T, E>(conn: &'a Connection, msg: &'a Message, f: F) -> Self
    where
        F: Future<Output = ::std::result::Result<T, E>> + Send + 'a,
        T: serde::Serialize + DynamicType + Send + Sync,
        E: zbus::DBusError + Send,
    {
        DispatchResult::Async(Box::pin(async move {
            let hdr = msg.header()?;
            match f.await {
                Ok(r) => conn.reply(msg, &r).await,
                Err(e) => conn.reply_dbus_error(&hdr, e).await,
            }
            .map(|_seq| ())
        }))
    }
}

/// The trait used to dispatch messages to an interface instance.
///
/// Note: It is not recommended to manually implement this trait. The [`dbus_interface`] macro
/// implements it for you.
///
/// [`dbus_interface`]: attr.dbus_interface.html
#[async_trait]
pub trait Interface: Any + Send + Sync {
    /// Return the name of the interface. Ex: "org.foo.MyInterface"
    fn name() -> InterfaceName<'static>
    where
        Self: Sized;

    /// Get a property value. Returns `None` if the property doesn't exist.
    async fn get(&self, property_name: &str) -> Option<fdo::Result<OwnedValue>>;

    /// Return all the properties.
    async fn get_all(&self) -> HashMap<String, OwnedValue>;

    /// Set a property value.
    ///
    /// Return [`DispatchResult::NotFound`] if the property doesn't exist, or
    /// [`DispatchResult::RequiresMut`] if `set_mut` should be used instead.  The default
    /// implementation just returns `RequiresMut`.
    fn set<'call>(
        &'call self,
        property_name: &'call str,
        value: &'call Value<'_>,
        ctxt: &'call SignalContext<'_>,
    ) -> DispatchResult<'call> {
        let _ = (property_name, value, ctxt);
        DispatchResult::RequiresMut
    }

    /// Set a property value.
    ///
    /// Returns `None` if the property doesn't exist.
    ///
    /// This will only be invoked if `set` returned `RequiresMut`.
    async fn set_mut(
        &mut self,
        property_name: &str,
        value: &Value<'_>,
        ctxt: &SignalContext<'_>,
    ) -> Option<fdo::Result<()>>;

    /// Call a method.
    ///
    /// Return [`DispatchResult::NotFound`] if the method doesn't exist, or
    /// [`DispatchResult::RequiresMut`] if `call_mut` should be used instead.
    ///
    /// It is valid, though inefficient, for this to always return `RequiresMut`.
    fn call<'call>(
        &'call self,
        server: &'call ObjectServer,
        connection: &'call Connection,
        msg: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult<'call>;

    /// Call a `&mut self` method.
    ///
    /// This will only be invoked if `call` returned `RequiresMut`.
    fn call_mut<'call>(
        &'call mut self,
        server: &'call ObjectServer,
        connection: &'call Connection,
        msg: &'call Message,
        name: MemberName<'call>,
    ) -> DispatchResult<'call>;

    /// Write introspection XML to the writer, with the given indentation level.
    fn introspect_to_writer(&self, writer: &mut dyn Write, level: usize);
}

// Note: while it is possible to implement this without `unsafe`, it currently requires a helper
// trait with a blanket impl that creates `dyn Any` refs.  It's simpler (and more performant) to
// just check the type ID and do the downcast ourself.
//
// See https://github.com/rust-lang/rust/issues/65991 for a rustc feature that will make it
// possible to get a `dyn Any` ref directly from a `dyn Interface` ref; once that is stable, we can
// remove this unsafe code.
impl dyn Interface {
    /// Return Any of self
    pub(crate) fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if <dyn Interface as Any>::type_id(self) == TypeId::of::<T>() {
            // SAFETY: If type ID matches, it means object is of type T
            Some(unsafe { &*(self as *const dyn Interface as *const T) })
        } else {
            None
        }
    }

    /// Return Any of self
    pub(crate) fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        if <dyn Interface as Any>::type_id(self) == TypeId::of::<T>() {
            // SAFETY: If type ID matches, it means object is of type T
            Some(unsafe { &mut *(self as *mut dyn Interface as *mut T) })
        } else {
            None
        }
    }
}
