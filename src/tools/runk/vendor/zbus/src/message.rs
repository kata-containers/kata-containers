use std::{
    convert::{Into, TryFrom, TryInto},
    fmt,
    io::{Cursor, Write},
};

#[cfg(unix)]
use std::{
    os::unix::io::{AsRawFd, RawFd},
    sync::{Arc, RwLock},
};

use enumflags2::BitFlags;
use static_assertions::assert_impl_all;
use zbus_names::{BusName, ErrorName, InterfaceName, MemberName, UniqueName};
use zvariant::{DynamicType, EncodingContext, ObjectPath, Signature, Type};

#[cfg(unix)]
use crate::OwnedFd;
use crate::{
    utils::padding_for_8_bytes, EndianSig, Error, MessageField, MessageFieldCode, MessageFields,
    MessageFlags, MessageHeader, MessagePrimaryHeader, MessageType, QuickMessageFields, Result,
    MIN_MESSAGE_SIZE, NATIVE_ENDIAN_SIG,
};

#[cfg(unix)]
const LOCK_PANIC_MSG: &str = "lock poisoned";

#[cfg(unix)]
type BuildGenericResult = Vec<RawFd>;

#[cfg(not(unix))]
type BuildGenericResult = ();

macro_rules! dbus_context {
    ($n_bytes_before: expr) => {
        EncodingContext::<byteorder::NativeEndian>::new_dbus($n_bytes_before)
    };
}

/// A builder for [`Message`]
#[derive(Debug)]
pub struct MessageBuilder<'a> {
    header: MessageHeader<'a>,
}

impl<'a> MessageBuilder<'a> {
    fn new(msg_type: MessageType) -> Self {
        let primary = MessagePrimaryHeader::new(msg_type, 0);
        let fields = MessageFields::new();
        let header = MessageHeader::new(primary, fields);
        Self { header }
    }

    /// Create a message of type [`MessageType::MethodCall`].
    pub fn method_call<'p: 'a, 'm: 'a, P, M>(path: P, method_name: M) -> Result<Self>
    where
        P: TryInto<ObjectPath<'p>>,
        M: TryInto<MemberName<'m>>,
        P::Error: Into<Error>,
        M::Error: Into<Error>,
    {
        Self::new(MessageType::MethodCall)
            .path(path)?
            .member(method_name)
    }

    /// Create a message of type [`MessageType::Signal`].
    pub fn signal<'p: 'a, 'i: 'a, 'm: 'a, P, I, M>(path: P, interface: I, name: M) -> Result<Self>
    where
        P: TryInto<ObjectPath<'p>>,
        I: TryInto<InterfaceName<'i>>,
        M: TryInto<MemberName<'m>>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
        M::Error: Into<Error>,
    {
        Self::new(MessageType::Signal)
            .path(path)?
            .interface(interface)?
            .member(name)
    }

    /// Create a message of type [`MessageType::MethodReturn`].
    pub fn method_return(reply_to: &MessageHeader<'_>) -> Result<Self> {
        Self::new(MessageType::MethodReturn).reply_to(reply_to)
    }

    /// Create a message of type [`MessageType::Error`].
    pub fn error<'e: 'a, E>(reply_to: &MessageHeader<'_>, name: E) -> Result<Self>
    where
        E: TryInto<ErrorName<'e>>,
        E::Error: Into<Error>,
    {
        Self::new(MessageType::Error)
            .error_name(name)?
            .reply_to(reply_to)
    }

    /// Add flags to the message.
    ///
    /// See [`MessageFlags`] documentation for the meaning of the flags.
    ///
    /// The function will return an error if invalid flags are given for the message type.
    pub fn with_flags(mut self, flag: MessageFlags) -> Result<Self> {
        if self.header.message_type()? != MessageType::MethodCall
            && BitFlags::from_flag(flag).contains(MessageFlags::NoReplyExpected)
        {
            return Err(Error::InvalidField);
        }
        let flags = self.header.primary().flags() | flag;
        self.header.primary_mut().set_flags(flags);
        Ok(self)
    }

    /// Set the unique name of the sending connection.
    pub fn sender<'s: 'a, S>(mut self, sender: S) -> Result<Self>
    where
        S: TryInto<UniqueName<'s>>,
        S::Error: Into<Error>,
    {
        self.header
            .fields_mut()
            .replace(MessageField::Sender(sender.try_into().map_err(Into::into)?));
        Ok(self)
    }

    /// Set the object to send a call to, or the object a signal is emitted from.
    pub fn path<'p: 'a, P>(mut self, path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<Error>,
    {
        self.header
            .fields_mut()
            .replace(MessageField::Path(path.try_into().map_err(Into::into)?));
        Ok(self)
    }

    /// Set the interface to invoke a method call on, or that a signal is emitted from.
    pub fn interface<'i: 'a, I>(mut self, interface: I) -> Result<Self>
    where
        I: TryInto<InterfaceName<'i>>,
        I::Error: Into<Error>,
    {
        self.header.fields_mut().replace(MessageField::Interface(
            interface.try_into().map_err(Into::into)?,
        ));
        Ok(self)
    }

    /// Set the member, either the method name or signal name.
    pub fn member<'m: 'a, M>(mut self, member: M) -> Result<Self>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
    {
        self.header
            .fields_mut()
            .replace(MessageField::Member(member.try_into().map_err(Into::into)?));
        Ok(self)
    }

    fn error_name<'e: 'a, E>(mut self, error: E) -> Result<Self>
    where
        E: TryInto<ErrorName<'e>>,
        E::Error: Into<Error>,
    {
        self.header.fields_mut().replace(MessageField::ErrorName(
            error.try_into().map_err(Into::into)?,
        ));
        Ok(self)
    }

    /// Set the name of the connection this message is intended for.
    pub fn destination<'d: 'a, D>(mut self, destination: D) -> Result<Self>
    where
        D: TryInto<BusName<'d>>,
        D::Error: Into<Error>,
    {
        self.header.fields_mut().replace(MessageField::Destination(
            destination.try_into().map_err(Into::into)?,
        ));
        Ok(self)
    }

    fn reply_to(mut self, reply_to: &MessageHeader<'_>) -> Result<Self> {
        let serial = reply_to.primary().serial_num().ok_or(Error::MissingField)?;
        self.header
            .fields_mut()
            .replace(MessageField::ReplySerial(*serial));

        if let Some(sender) = reply_to.sender()? {
            self.destination(sender.to_owned())
        } else {
            Ok(self)
        }
    }

    /// Build the [`Message`] with the given body.
    ///
    /// You may pass `()` as the body if the message has no body.
    ///
    /// The caller is currently required to ensure that the resulting message contains the headers
    /// as compliant with the [specification]. Additional checks may be added to this builder over
    /// time as needed.
    ///
    /// [specification]:
    /// https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-header-fields
    pub fn build<B>(self, body: &B) -> Result<Message>
    where
        B: serde::ser::Serialize + DynamicType,
    {
        let ctxt = dbus_context!(0);

        // Note: this iterates the body twice, but we prefer efficient handling of large messages
        // to efficient handling of ones that are complex to serialize.
        #[cfg(unix)]
        let (body_len, fds_len) = zvariant::serialized_size_fds(ctxt, body)?;
        #[cfg(not(unix))]
        let body_len = zvariant::serialized_size(ctxt, body)?;

        let signature = body.dynamic_signature();

        self.build_generic(
            signature,
            body_len,
            move |cursor| {
                #[cfg(unix)]
                {
                    let (_, fds) = zvariant::to_writer_fds(cursor, ctxt, body)?;
                    Ok::<Vec<RawFd>, Error>(fds)
                }
                #[cfg(not(unix))]
                {
                    zvariant::to_writer(cursor, ctxt, body)?;
                    Ok::<(), Error>(())
                }
            },
            #[cfg(unix)]
            fds_len,
        )
    }

    /// Create a new message from a raw slice of bytes to populate the body with, rather than by
    /// serializing a value. The message body will be the exact bytes.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it can be used to build an invalid message.
    pub unsafe fn build_raw_body<'b, S>(
        self,
        body_bytes: &[u8],
        signature: S,
        #[cfg(unix)] fds: Vec<RawFd>,
    ) -> Result<Message>
    where
        S: TryInto<Signature<'b>>,
        S::Error: Into<Error>,
    {
        let signature: Signature<'b> = signature.try_into().map_err(Into::into)?;
        #[cfg(unix)]
        let fds_len = fds.len();

        self.build_generic(
            signature,
            body_bytes.len(),
            move |cursor: &mut Cursor<&mut Vec<u8>>| {
                cursor.write_all(body_bytes)?;

                #[cfg(unix)]
                return Ok::<Vec<RawFd>, Error>(fds);

                #[cfg(not(unix))]
                return Ok::<(), Error>(());
            },
            #[cfg(unix)]
            fds_len,
        )
    }

    fn build_generic<'b, WriteFunc>(
        self,
        mut signature: Signature<'b>,
        body_len: usize,
        write_body: WriteFunc,
        #[cfg(unix)] fds_len: usize,
    ) -> Result<Message>
    where
        WriteFunc: FnOnce(&mut Cursor<&mut Vec<u8>>) -> Result<BuildGenericResult>,
    {
        let ctxt = dbus_context!(0);
        let mut header = self.header;

        if !signature.is_empty() {
            if signature.starts_with(zvariant::STRUCT_SIG_START_STR) {
                // Remove leading and trailing STRUCT delimiters
                signature = signature.slice(1..signature.len() - 1);
            }
            header.fields_mut().add(MessageField::Signature(signature));
        }

        let body_len_u32 = body_len.try_into().map_err(|_| Error::ExcessData)?;
        header.primary_mut().set_body_len(body_len_u32);

        #[cfg(unix)]
        {
            let fds_len_u32 = fds_len.try_into().map_err(|_| Error::ExcessData)?;
            if fds_len != 0 {
                header.fields_mut().add(MessageField::UnixFDs(fds_len_u32));
            }
        }

        let hdr_len = zvariant::serialized_size(ctxt, &header)?;

        let mut bytes: Vec<u8> = Vec::with_capacity(hdr_len + body_len);
        let mut cursor = Cursor::new(&mut bytes);

        zvariant::to_writer(&mut cursor, ctxt, &header)?;
        #[cfg(unix)]
        let fds = write_body(&mut cursor)?;
        #[cfg(not(unix))]
        write_body(&mut cursor)?;

        let primary_header = header.into_primary();
        let header: MessageHeader<'_> = zvariant::from_slice(&bytes, ctxt)?;
        let quick_fields = QuickMessageFields::new(&bytes, &header)?;

        Ok(Message {
            primary_header,
            quick_fields,
            bytes,
            body_offset: hdr_len,
            #[cfg(unix)]
            fds: Arc::new(RwLock::new(Fds::Raw(fds))),
            recv_seq: MessageSequence::default(),
        })
    }
}

#[cfg(unix)]
#[derive(Debug, Eq, PartialEq)]
enum Fds {
    Owned(Vec<OwnedFd>),
    Raw(Vec<RawFd>),
}

/// A position in the stream of [`Message`] objects received by a single [`zbus::Connection`].
#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct MessageSequence {
    recv_seq: u64,
}

/// A D-Bus Message.
///
/// The content of the message are stored in serialized format. To deserialize the body of the
/// message, use the [`body`] method. You may also access the header and other details with the
/// various other getters.
///
/// Also provided are constructors for messages of different types. These will mainly be useful for
/// very advanced use cases as typically you will want to create a message for immediate dispatch
/// and hence use the API provided by [`Connection`], even when using the low-level API.
///
/// **Note**: The message owns the received FDs and will close them when dropped. You can call
/// [`take_fds`] after deserializing to `RawFD` using [`body`] if you want to take the ownership.
///
/// [`body`]: #method.body
/// [`take_fds`]: #method.take_fds
/// [`Connection`]: struct.Connection#method.call_method
#[derive(Clone)]
pub struct Message {
    primary_header: MessagePrimaryHeader,
    quick_fields: QuickMessageFields,
    bytes: Vec<u8>,
    body_offset: usize,
    #[cfg(unix)]
    fds: Arc<RwLock<Fds>>,
    recv_seq: MessageSequence,
}

assert_impl_all!(Message: Send, Sync, Unpin);

// TODO: Handle non-native byte order: https://gitlab.freedesktop.org/dbus/zbus/-/issues/19
impl Message {
    /// Create a message of type [`MessageType::MethodCall`].
    ///
    /// [`MessageType::MethodCall`]: enum.MessageType.html#variant.MethodCall
    pub fn method<'s, 'd, 'p, 'i, 'm, S, D, P, I, M, B>(
        sender: Option<S>,
        destination: Option<D>,
        path: P,
        iface: Option<I>,
        method_name: M,
        body: &B,
    ) -> Result<Self>
    where
        S: TryInto<UniqueName<'s>>,
        D: TryInto<BusName<'d>>,
        P: TryInto<ObjectPath<'p>>,
        I: TryInto<InterfaceName<'i>>,
        M: TryInto<MemberName<'m>>,
        S::Error: Into<Error>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + DynamicType,
    {
        let mut b = MessageBuilder::method_call(path, method_name)?;

        if let Some(sender) = sender {
            b = b.sender(sender)?;
        }
        if let Some(destination) = destination {
            b = b.destination(destination)?;
        }
        if let Some(iface) = iface {
            b = b.interface(iface)?;
        }
        b.build(body)
    }

    /// Create a message of type [`MessageType::Signal`].
    ///
    /// [`MessageType::Signal`]: enum.MessageType.html#variant.Signal
    pub fn signal<'s, 'd, 'p, 'i, 'm, S, D, P, I, M, B>(
        sender: Option<S>,
        destination: Option<D>,
        path: P,
        iface: I,
        signal_name: M,
        body: &B,
    ) -> Result<Self>
    where
        S: TryInto<UniqueName<'s>>,
        D: TryInto<BusName<'d>>,
        P: TryInto<ObjectPath<'p>>,
        I: TryInto<InterfaceName<'i>>,
        M: TryInto<MemberName<'m>>,
        S::Error: Into<Error>,
        D::Error: Into<Error>,
        P::Error: Into<Error>,
        I::Error: Into<Error>,
        M::Error: Into<Error>,
        B: serde::ser::Serialize + DynamicType,
    {
        let mut b = MessageBuilder::signal(path, iface, signal_name)?;

        if let Some(sender) = sender {
            b = b.sender(sender)?;
        }
        if let Some(destination) = destination {
            b = b.destination(destination)?;
        }
        b.build(body)
    }

    /// Create a message of type [`MessageType::MethodReturn`].
    ///
    /// [`MessageType::MethodReturn`]: enum.MessageType.html#variant.MethodReturn
    pub fn method_reply<'s, S, B>(sender: Option<S>, call: &Self, body: &B) -> Result<Self>
    where
        S: TryInto<UniqueName<'s>>,
        S::Error: Into<Error>,
        B: serde::ser::Serialize + DynamicType,
    {
        let mut b = MessageBuilder::method_return(&call.header()?)?;
        if let Some(sender) = sender {
            b = b.sender(sender)?;
        }
        b.build(body)
    }

    /// Create a message of type [`MessageType::MethodError`].
    ///
    /// [`MessageType::MethodError`]: enum.MessageType.html#variant.MethodError
    pub fn method_error<'s, 'e, S, E, B>(
        sender: Option<S>,
        call: &Self,
        name: E,
        body: &B,
    ) -> Result<Self>
    where
        S: TryInto<UniqueName<'s>>,
        S::Error: Into<Error>,
        E: TryInto<ErrorName<'e>>,
        E::Error: Into<Error>,
        B: serde::ser::Serialize + DynamicType,
    {
        let mut b = MessageBuilder::error(&call.header()?, name)?;
        if let Some(sender) = sender {
            b = b.sender(sender)?;
        }
        b.build(body)
    }

    /// Create a message from its full contents
    pub(crate) fn from_raw_parts(
        bytes: Vec<u8>,
        #[cfg(unix)] fds: Vec<OwnedFd>,
        recv_seq: u64,
    ) -> Result<Self> {
        if EndianSig::try_from(bytes[0])? != NATIVE_ENDIAN_SIG {
            return Err(Error::IncorrectEndian);
        }

        let (primary_header, fields_len) = MessagePrimaryHeader::read(&bytes)?;
        let header = zvariant::from_slice(&bytes, dbus_context!(0))?;
        #[cfg(unix)]
        let fds = Arc::new(RwLock::new(Fds::Owned(fds)));

        let header_len = MIN_MESSAGE_SIZE + fields_len as usize;
        let body_offset = header_len + padding_for_8_bytes(header_len);
        let quick_fields = QuickMessageFields::new(&bytes, &header)?;

        Ok(Self {
            primary_header,
            quick_fields,
            bytes,
            body_offset,
            #[cfg(unix)]
            fds,
            recv_seq: MessageSequence { recv_seq },
        })
    }

    /// Take ownership of the associated file descriptors in the message.
    ///
    /// When a message is received over a AF_UNIX socket, it may contain associated FDs. To prevent
    /// the message from closing those FDs on drop, call this method that returns all the received
    /// FDs with their ownership.
    ///
    /// This function is Unix-specific.
    ///
    /// Note: the message will continue to reference the files, so you must keep them open for as
    /// long as the message itself.
    #[cfg(unix)]
    pub fn take_fds(&self) -> Vec<OwnedFd> {
        let mut fds_lock = self.fds.write().expect(LOCK_PANIC_MSG);
        if let Fds::Owned(ref mut fds) = *fds_lock {
            // From now on, it's the caller responsibility to close the fds
            let fds = std::mem::take(&mut *fds);
            *fds_lock = Fds::Raw(fds.iter().map(|fd| fd.as_raw_fd()).collect());
            fds
        } else {
            vec![]
        }
    }

    /// The signature of the body.
    ///
    /// **Note:** While zbus treats multiple arguments as a struct (to allow you to use the tuple
    /// syntax), D-Bus does not. Since this method gives you the signature expected on the wire by
    /// D-Bus, the trailing and leading STRUCT signature parenthesis will not be present in case of
    /// multiple arguments.
    pub fn body_signature(&self) -> Result<Signature<'_>> {
        match self
            .header()?
            .into_fields()
            .into_field(MessageFieldCode::Signature)
            .ok_or(Error::NoBodySignature)?
        {
            MessageField::Signature(signature) => Ok(signature),
            _ => Err(Error::InvalidField),
        }
    }

    pub fn primary_header(&self) -> &MessagePrimaryHeader {
        &self.primary_header
    }

    pub(crate) fn modify_primary_header<F>(&mut self, mut modifier: F) -> Result<()>
    where
        F: FnMut(&mut MessagePrimaryHeader) -> Result<()>,
    {
        modifier(&mut self.primary_header)?;

        let mut cursor = Cursor::new(&mut self.bytes);
        zvariant::to_writer(&mut cursor, dbus_context!(0), &self.primary_header)
            .map(|_| ())
            .map_err(Error::from)
    }

    /// Deserialize the header.
    ///
    /// Note: prefer using the direct access methods if possible; they are more efficient.
    pub fn header(&self) -> Result<MessageHeader<'_>> {
        zvariant::from_slice(&self.bytes, dbus_context!(0)).map_err(Error::from)
    }

    /// Deserialize the fields.
    ///
    /// Note: prefer using the direct access methods if possible; they are more efficient.
    pub fn fields(&self) -> Result<MessageFields<'_>> {
        let ctxt = dbus_context!(crate::PRIMARY_HEADER_SIZE);
        zvariant::from_slice(&self.bytes[crate::PRIMARY_HEADER_SIZE..], ctxt).map_err(Error::from)
    }

    /// The message type.
    pub fn message_type(&self) -> MessageType {
        self.primary_header.msg_type()
    }

    /// The object to send a call to, or the object a signal is emitted from.
    pub fn path(&self) -> Option<ObjectPath<'_>> {
        self.quick_fields.path(self)
    }

    /// The interface to invoke a method call on, or that a signal is emitted from.
    pub fn interface(&self) -> Option<InterfaceName<'_>> {
        self.quick_fields.interface(self)
    }

    /// The member, either the method name or signal name.
    pub fn member(&self) -> Option<MemberName<'_>> {
        self.quick_fields.member(self)
    }

    /// The serial number of the message this message is a reply to.
    pub fn reply_serial(&self) -> Option<u32> {
        self.quick_fields.reply_serial()
    }

    /// Deserialize the body (without checking signature matching).
    pub fn body_unchecked<'d, 'm: 'd, B>(&'m self) -> Result<B>
    where
        B: serde::de::Deserialize<'d> + Type,
    {
        {
            #[cfg(unix)]
            {
                zvariant::from_slice_fds(
                    &self.bytes[self.body_offset..],
                    Some(&self.fds()),
                    dbus_context!(0),
                )
            }
            #[cfg(not(unix))]
            {
                zvariant::from_slice(&self.bytes[self.body_offset..], dbus_context!(0))
            }
        }
        .map_err(Error::from)
    }

    /// Deserialize the body using the contained signature.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::convert::TryInto;
    /// # use zbus::Message;
    /// # (|| -> zbus::Result<()> {
    /// let send_body = (7i32, (2i32, "foo"), vec!["bar"]);
    /// let message = Message::method(None::<&str>, Some("zbus.test"), "/", Some("zbus.test"), "ping", &send_body)?;
    /// let body : zvariant::Structure = message.body()?;
    /// let fields = body.fields();
    /// assert!(matches!(fields[0], zvariant::Value::I32(7)));
    /// assert!(matches!(fields[1], zvariant::Value::Structure(_)));
    /// assert!(matches!(fields[2], zvariant::Value::Array(_)));
    ///
    /// let reply_msg = Message::method_reply(None::<&str>, &message, &body)?;
    /// let reply_value : (i32, (i32, &str), Vec<String>) = reply_msg.body()?;
    ///
    /// assert_eq!(reply_value.0, 7);
    /// assert_eq!(reply_value.2.len(), 1);
    /// # Ok(()) })().unwrap()
    /// ```
    pub fn body<'d, 'm: 'd, B>(&'m self) -> Result<B>
    where
        B: zvariant::DynamicDeserialize<'d>,
    {
        let body_sig = match self.body_signature() {
            Ok(sig) => sig,
            Err(Error::NoBodySignature) => Signature::from_static_str_unchecked(""),
            Err(e) => return Err(e),
        };

        {
            #[cfg(unix)]
            {
                zvariant::from_slice_fds_for_dynamic_signature(
                    &self.bytes[self.body_offset..],
                    Some(&self.fds()),
                    dbus_context!(0),
                    &body_sig,
                )
            }
            #[cfg(not(unix))]
            {
                zvariant::from_slice_for_dynamic_signature(
                    &self.bytes[self.body_offset..],
                    dbus_context!(0),
                    &body_sig,
                )
            }
        }
        .map_err(Error::from)
    }

    #[cfg(unix)]
    pub(crate) fn fds(&self) -> Vec<RawFd> {
        match &*self.fds.read().expect(LOCK_PANIC_MSG) {
            Fds::Raw(fds) => fds.clone(),
            Fds::Owned(fds) => fds.iter().map(|f| f.as_raw_fd()).collect(),
        }
    }

    /// Get a reference to the byte encoding of the message.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get a reference to the byte encoding of the body of the message.
    pub fn body_as_bytes(&self) -> Result<&[u8]> {
        Ok(&self.bytes[self.body_offset..])
    }

    /// Get the receive ordering of a message.
    ///
    /// This may be used to identify how two events were ordered on the bus.  It only produces a
    /// useful ordering for messages that were produced by the same [`zbus::Connection`].
    ///
    /// This is completely unrelated to the serial number on the message, which is set by the peer
    /// and might not be ordered at all.
    pub fn recv_position(&self) -> MessageSequence {
        self.recv_seq
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut msg = f.debug_struct("Msg");
        let _ = self.header().map(|h| {
            if let Ok(t) = h.message_type() {
                msg.field("type", &t);
            }
            if let Ok(Some(sender)) = h.sender() {
                msg.field("sender", &sender);
            }
            if let Ok(Some(serial)) = h.reply_serial() {
                msg.field("reply-serial", &serial);
            }
            if let Ok(Some(path)) = h.path() {
                msg.field("path", &path);
            }
            if let Ok(Some(iface)) = h.interface() {
                msg.field("iface", &iface);
            }
            if let Ok(Some(member)) = h.member() {
                msg.field("member", &member);
            }
        });
        if let Ok(s) = self.body_signature() {
            msg.field("body", &s);
        }
        #[cfg(unix)]
        {
            let fds = self.fds();
            if !fds.is_empty() {
                msg.field("fds", &fds);
            }
        }
        msg.finish()
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = self.header();
        let (ty, error_name, sender, member) = if let Ok(h) = header.as_ref() {
            (
                h.message_type().ok(),
                h.error_name().ok().flatten(),
                h.sender().ok().flatten(),
                h.member().ok().flatten(),
            )
        } else {
            (None, None, None, None)
        };

        match ty {
            Some(MessageType::MethodCall) => {
                write!(f, "Method call")?;
                if let Some(m) = member {
                    write!(f, " {}", m)?;
                }
            }
            Some(MessageType::MethodReturn) => {
                write!(f, "Method return")?;
            }
            Some(MessageType::Error) => {
                write!(f, "Error")?;
                if let Some(e) = error_name {
                    write!(f, " {}", e)?;
                }

                let msg = self.body_unchecked::<&str>();
                if let Ok(msg) = msg {
                    write!(f, ": {}", msg)?;
                }
            }
            Some(MessageType::Signal) => {
                write!(f, "Signal")?;
                if let Some(m) = member {
                    write!(f, " {}", m)?;
                }
            }
            _ => {
                write!(f, "Unknown message")?;
            }
        }

        if let Some(s) = sender {
            write!(f, " from {}", s)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;
    use test_log::test;
    #[cfg(unix)]
    use zvariant::Fd;

    #[cfg(unix)]
    use super::Fds;
    use super::{Message, MessageBuilder};
    use crate::Error;

    #[test]
    fn test() {
        #[cfg(unix)]
        let stdout = std::io::stdout();
        let m = Message::method(
            Some(":1.72"),
            None::<()>,
            "/",
            None::<()>,
            "do",
            &(
                #[cfg(unix)]
                Fd::from(&stdout),
                "foo",
            ),
        )
        .unwrap();
        assert_eq!(
            m.body_signature().unwrap().to_string(),
            if cfg!(unix) { "hs" } else { "s" }
        );
        #[cfg(unix)]
        assert_eq!(*m.fds.read().unwrap(), Fds::Raw(vec![stdout.as_raw_fd()]));

        let body: Result<u32, Error> = m.body();
        assert!(matches!(
            body.unwrap_err(),
            Error::Variant(zvariant::Error::SignatureMismatch { .. })
        ));

        assert_eq!(m.to_string(), "Method call do from :1.72");
        let r = Message::method_reply(None::<()>, &m, &("all fine!")).unwrap();
        assert_eq!(r.to_string(), "Method return");
        let e = Message::method_error(
            None::<()>,
            &m,
            "org.freedesktop.zbus.Error",
            &("kaboom!", 32),
        )
        .unwrap();
        assert_eq!(e.to_string(), "Error org.freedesktop.zbus.Error: kaboom!");
    }

    #[test]
    fn test_raw() -> Result<(), Error> {
        let raw_body: &[u8] = &[16, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0];
        let message_builder = MessageBuilder::signal("/", "test.test", "test")?;
        let message = unsafe {
            message_builder.build_raw_body(
                raw_body,
                "ai",
                #[cfg(unix)]
                vec![],
            )?
        };

        let output: Vec<i32> = message.body()?;
        assert_eq!(output, vec![1, 2, 3, 4]);

        Ok(())
    }
}
