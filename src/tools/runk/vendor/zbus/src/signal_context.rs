use crate::{zvariant::ObjectPath, Connection, Error, Result};
use std::convert::TryInto;

/// A signal emission context.
///
/// For signal emission using the high-level API, you'll need instances of this type.
///
/// See [`crate::InterfaceRef::signal_context`] and [`crate::dbus_interface`]
/// documentation for details and examples of this type in use.
#[derive(Clone, Debug)]
pub struct SignalContext<'s> {
    conn: Connection,
    path: ObjectPath<'s>,
}

impl<'s> SignalContext<'s> {
    /// Create a new signal context for the given connection and object path.
    pub fn new<P>(conn: &Connection, path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'s>>,
        P::Error: Into<Error>,
    {
        path.try_into()
            .map(|p| Self {
                conn: conn.clone(),
                path: p,
            })
            .map_err(Into::into)
    }

    /// Create a new signal context for the given connection and object path.
    pub fn from_parts(conn: Connection, path: ObjectPath<'s>) -> Self {
        Self { conn, path }
    }

    /// Get a reference to the associated connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get a reference to the associated object path.
    pub fn path(&self) -> &ObjectPath<'s> {
        &self.path
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> SignalContext<'static> {
        SignalContext {
            conn: self.conn.clone(),
            path: self.path.to_owned(),
        }
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> SignalContext<'static> {
        SignalContext {
            conn: self.conn,
            path: self.path.into_owned(),
        }
    }
}
