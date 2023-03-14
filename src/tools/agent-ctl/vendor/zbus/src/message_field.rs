use std::convert::TryFrom;

use serde::{
    de::{Deserialize, Deserializer, Error},
    ser::{Serialize, Serializer},
};
use serde_repr::{Deserialize_repr, Serialize_repr};

use static_assertions::assert_impl_all;
use zbus_names::{BusName, ErrorName, InterfaceName, MemberName, UniqueName};
use zvariant::{ObjectPath, Signature, Type, Value};

/// The message field code.
///
/// Every [`MessageField`] has an associated code. This is mostly an internal D-Bus protocol detail
/// that you would not need to ever care about when using the high-level API. When using the
/// low-level API, this is how you can [retrieve a specific field] from [`MessageFields`].
///
/// [`MessageField`]: enum.MessageField.html
/// [retrieve a specific field]: struct.MessageFields.html#method.get_field
/// [`MessageFields`]: struct.MessageFields.html
#[repr(u8)]
#[derive(Copy, Clone, Debug, Deserialize_repr, PartialEq, Eq, Serialize_repr, Type)]
pub enum MessageFieldCode {
    /// Code for [`MessageField::Invalid`](enum.MessageField.html#variant.Invalid)
    Invalid = 0,
    /// Code for [`MessageField::Path`](enum.MessageField.html#variant.Path)
    Path = 1,
    /// Code for [`MessageField::Interface`](enum.MessageField.html#variant.Interface)
    Interface = 2,
    /// Code for [`MessageField::Member`](enum.MessageField.html#variant.Member)
    Member = 3,
    /// Code for [`MessageField::ErrorName`](enum.MessageField.html#variant.ErrorName)
    ErrorName = 4,
    /// Code for [`MessageField::ReplySerial`](enum.MessageField.html#variant.ReplySerial)
    ReplySerial = 5,
    /// Code for [`MessageField::Destinatione`](enum.MessageField.html#variant.Destination)
    Destination = 6,
    /// Code for [`MessageField::Sender`](enum.MessageField.html#variant.Sender)
    Sender = 7,
    /// Code for [`MessageField::Signature`](enum.MessageField.html#variant.Signature)
    Signature = 8,
    /// Code for [`MessageField::UnixFDs`](enum.MessageField.html#variant.UnixFDs)
    UnixFDs = 9,
}

assert_impl_all!(MessageFieldCode: Send, Sync, Unpin);

impl From<u8> for MessageFieldCode {
    fn from(val: u8) -> MessageFieldCode {
        match val {
            1 => MessageFieldCode::Path,
            2 => MessageFieldCode::Interface,
            3 => MessageFieldCode::Member,
            4 => MessageFieldCode::ErrorName,
            5 => MessageFieldCode::ReplySerial,
            6 => MessageFieldCode::Destination,
            7 => MessageFieldCode::Sender,
            8 => MessageFieldCode::Signature,
            9 => MessageFieldCode::UnixFDs,
            _ => MessageFieldCode::Invalid,
        }
    }
}

impl<'f> MessageField<'f> {
    /// Get the associated code for this field.
    pub fn code(&self) -> MessageFieldCode {
        match self {
            MessageField::Path(_) => MessageFieldCode::Path,
            MessageField::Interface(_) => MessageFieldCode::Interface,
            MessageField::Member(_) => MessageFieldCode::Member,
            MessageField::ErrorName(_) => MessageFieldCode::ErrorName,
            MessageField::ReplySerial(_) => MessageFieldCode::ReplySerial,
            MessageField::Destination(_) => MessageFieldCode::Destination,
            MessageField::Sender(_) => MessageFieldCode::Sender,
            MessageField::Signature(_) => MessageFieldCode::Signature,
            MessageField::UnixFDs(_) => MessageFieldCode::UnixFDs,
            MessageField::Invalid => MessageFieldCode::Invalid,
        }
    }
}

/// The dynamic message header.
///
/// All D-Bus messages contain a set of metadata [headers]. Some of these headers [are fixed] for
/// all types of messages, while others depend on the type of the message in question. The latter
/// are called message fields.
///
/// Please consult the [Message Format] section of the D-Bus spec for more details.
///
/// [headers]: struct.MessageHeader.html
/// [are fixed]: struct.MessagePrimaryHeader.html
/// [Message Format]: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-messages
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageField<'f> {
    /// Not a valid field.
    Invalid,
    /// The object to send a call to, or the object a signal is emitted from.
    Path(ObjectPath<'f>),
    /// The interface to invoke a method call on, or that a signal is emitted from.
    Interface(InterfaceName<'f>),
    /// The member, either the method name or signal name.
    Member(MemberName<'f>),
    /// The name of the error that occurred, for errors
    ErrorName(ErrorName<'f>),
    /// The serial number of the message this message is a reply to.
    ReplySerial(u32),
    /// The name of the connection this message is intended for.
    Destination(BusName<'f>),
    /// Unique name of the sending connection.
    Sender(UniqueName<'f>),
    /// The signature of the message body.
    Signature(Signature<'f>),
    /// The number of Unix file descriptors that accompany the message.
    UnixFDs(u32),
}

assert_impl_all!(MessageField<'_>: Send, Sync, Unpin);

impl<'f> Type for MessageField<'f> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked("(yv)")
    }
}

impl<'f> Serialize for MessageField<'f> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tuple: (MessageFieldCode, Value<'_>) = match self {
            MessageField::Path(value) => (MessageFieldCode::Path, value.clone().into()),
            MessageField::Interface(value) => (MessageFieldCode::Interface, value.as_str().into()),
            MessageField::Member(value) => (MessageFieldCode::Member, value.as_str().into()),
            MessageField::ErrorName(value) => (MessageFieldCode::ErrorName, value.as_str().into()),
            MessageField::ReplySerial(value) => (MessageFieldCode::ReplySerial, (*value).into()),
            MessageField::Destination(value) => {
                (MessageFieldCode::Destination, value.as_str().into())
            }
            MessageField::Sender(value) => (MessageFieldCode::Sender, value.as_str().into()),
            MessageField::Signature(value) => (MessageFieldCode::Signature, value.clone().into()),
            MessageField::UnixFDs(value) => (MessageFieldCode::UnixFDs, (*value).into()),
            // This is a programmer error
            MessageField::Invalid => panic!("Attempt to serialize invalid MessageField"),
        };

        tuple.serialize(serializer)
    }
}

impl<'de: 'f, 'f> Deserialize<'de> for MessageField<'f> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (code, value) = <(MessageFieldCode, Value<'_>)>::deserialize(deserializer)?;
        Ok(match code {
            MessageFieldCode::Path => {
                MessageField::Path(ObjectPath::try_from(value).map_err(D::Error::custom)?)
            }
            MessageFieldCode::Interface => {
                MessageField::Interface(InterfaceName::try_from(value).map_err(D::Error::custom)?)
            }
            MessageFieldCode::Member => {
                MessageField::Member(MemberName::try_from(value).map_err(D::Error::custom)?)
            }
            MessageFieldCode::ErrorName => MessageField::ErrorName(
                ErrorName::try_from(value)
                    .map(Into::into)
                    .map_err(D::Error::custom)?,
            ),
            MessageFieldCode::ReplySerial => {
                MessageField::ReplySerial(u32::try_from(value).map_err(D::Error::custom)?)
            }
            MessageFieldCode::Destination => MessageField::Destination(
                BusName::try_from(value)
                    .map(Into::into)
                    .map_err(D::Error::custom)?,
            ),
            MessageFieldCode::Sender => MessageField::Sender(
                UniqueName::try_from(value)
                    .map(Into::into)
                    .map_err(D::Error::custom)?,
            ),
            MessageFieldCode::Signature => {
                MessageField::Signature(Signature::try_from(value).map_err(D::Error::custom)?)
            }
            MessageFieldCode::UnixFDs => {
                MessageField::UnixFDs(u32::try_from(value).map_err(D::Error::custom)?)
            }
            MessageFieldCode::Invalid => {
                return Err(Error::invalid_value(
                    serde::de::Unexpected::Unsigned(code as u64),
                    &"A valid D-Bus message field code",
                ));
            }
        })
    }
}
