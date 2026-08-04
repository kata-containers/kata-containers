// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! A minimal, blocking D-Bus client.
//!
//! We only ever talk to one peer, systemd, over its private socket, and we do
//! it one call at a time. That rules out everything a general D-Bus library
//! spends its complexity on: no bus daemon, no name resolution, no signal
//! subscriptions, no concurrent calls to demultiplex. What is left is the wire
//! format, which `zvariant` implements for us, plus the handshake and framing
//! in this module.
//!
//! Blocking sockets with timeouts are a deliberate choice. An async client
//! keeps replies flowing through a reactor, an executor and the caller's
//! runtime, and a wakeup lost anywhere along that chain leaves the caller
//! waiting forever. Here a peer that stops answering can only ever produce a
//! timed-out read.

use anyhow::{anyhow, bail, Context, Result};
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};
use zvariant::serialized::{Context as EncodingContext, Data};
use zvariant::{DynamicType, Endian, ObjectPath, OwnedValue, Signature, Type, Value};

const PROTOCOL_VERSION: u8 = 1;

const MESSAGE_TYPE_METHOD_CALL: u8 = 1;
const MESSAGE_TYPE_METHOD_RETURN: u8 = 2;
const MESSAGE_TYPE_ERROR: u8 = 3;
const MESSAGE_TYPE_SIGNAL: u8 = 4;

const FLAG_NO_REPLY_EXPECTED: u8 = 1;

const FIELD_PATH: u8 = 1;
const FIELD_INTERFACE: u8 = 2;
const FIELD_MEMBER: u8 = 3;
const FIELD_ERROR_NAME: u8 = 4;
const FIELD_REPLY_SERIAL: u8 = 5;
const FIELD_DESTINATION: u8 = 6;
const FIELD_SIGNATURE: u8 = 8;

/// Fixed part of a message header: everything up to and including the length
/// of the header field array.
const PRIMARY_HEADER_LEN: usize = 16;

/// Same cap the reference implementation puts on a single message.
const MAX_MESSAGE_LEN: u32 = 128 * 1024 * 1024;

/// How many signals to hold on to while a call is in flight. A subscribed peer
/// can talk a lot, and only the handful that arrive between sending a call and
/// reading its reply can be of interest to whoever waits next.
const MAX_BUFFERED_SIGNALS: usize = 64;

fn endian_code(endian: Endian) -> u8 {
    match endian {
        Endian::Little => b'l',
        Endian::Big => b'B',
    }
}

fn padding_to_8(len: usize) -> usize {
    (8 - (len % 8)) % 8
}

/// A method call reply, kept as the bytes we read off the socket so that the
/// body is only deserialized if the caller asks for it.
pub struct Reply {
    bytes: Vec<u8>,
    body_offset: usize,
    endian: Endian,
}

impl Reply {
    /// Deserialize the reply body as `T`.
    pub fn body<T>(&self) -> Result<T>
    where
        T: serde::de::DeserializeOwned + zvariant::Type,
    {
        // The body starts on an 8 byte boundary, so its alignment is the same
        // whether it is counted from the start of the message or of the body.
        let ctxt = EncodingContext::new_dbus(self.endian, 0);
        let data = Data::new(&self.bytes[self.body_offset..], ctxt);

        data.deserialize()
            .map(|(value, _)| value)
            .context("Failed to deserialize the reply body")
    }
}

/// The method a call is addressed to.
pub struct Method<'a> {
    pub destination: &'a str,
    pub path: &'a str,
    pub interface: &'a str,
    pub member: &'a str,
}

pub struct Connection {
    socket: UnixStream,
    serial: u32,
    timeout: Duration,
    signals: VecDeque<(String, Reply)>,
}

impl Connection {
    /// Connect to a peer listening on `path` and complete the SASL handshake.
    ///
    /// `timeout` bounds every individual read and write, so no call can block
    /// for longer than a peer's silence multiplied by the reads it takes to
    /// assemble one message.
    pub fn connect(path: impl AsRef<Path>, timeout: Duration) -> Result<Self> {
        let path = path.as_ref();
        let socket = UnixStream::connect(path)
            .with_context(|| format!("Failed to connect to {}", path.display()))?;
        socket.set_read_timeout(Some(timeout))?;
        socket.set_write_timeout(Some(timeout))?;

        let mut connection = Self {
            socket,
            serial: 0,
            timeout,
            signals: VecDeque::new(),
        };
        connection
            .authenticate()
            .with_context(|| format!("Failed to authenticate to {}", path.display()))?;

        Ok(connection)
    }

    /// Authenticate as our own uid, which is what the EXTERNAL mechanism means
    /// on a Unix socket: the peer reads the credentials off the socket itself
    /// and we merely state which uid it is going to find.
    fn authenticate(&mut self) -> Result<()> {
        // SAFETY: getuid() is always successful.
        let uid = unsafe { libc::getuid() };
        let uid = uid
            .to_string()
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        // The leading NUL is not part of the SASL protocol; it is how a D-Bus
        // client announces itself on the socket before the exchange starts.
        self.socket.write_all(b"\0")?;
        self.write_command(&format!("AUTH EXTERNAL {uid}"))?;

        let response = self.read_command()?;
        if !response.starts_with("OK ") {
            bail!("Peer rejected the EXTERNAL authentication: {response}");
        }

        // We never pass file descriptors, so there is nothing to negotiate.
        self.write_command("BEGIN")?;
        self.wait_until_read();

        Ok(())
    }

    /// Wait until the peer has taken everything we wrote out of the socket.
    ///
    /// BEGIN ends the handshake, and the first method call follows it closely
    /// enough that the two can reach the peer as a single chunk. systemd reads
    /// that chunk while it is still authenticating, keeps whatever follows
    /// BEGIN in a buffer of its own, and only looks at that buffer again the
    /// next time something else wakes its event loop. On an idle machine that
    /// can be a minute later, which we see as a call nobody answers. Once
    /// BEGIN has been read on its own, the call stays in the kernel's queue,
    /// where it is guaranteed to wake the peer.
    ///
    /// Waiting is best effort: everything past this point is guarded by the
    /// socket timeouts anyway.
    fn wait_until_read(&self) {
        let deadline = Instant::now() + self.timeout;

        while Instant::now() < deadline {
            let mut pending: libc::c_int = 0;
            // SAFETY: the socket is ours and pending is a valid c_int. On a
            // Unix socket this asks how much of what we sent the peer has yet
            // to read.
            let result =
                unsafe { libc::ioctl(self.socket.as_raw_fd(), libc::TIOCOUTQ, &mut pending) };

            if result != 0 || pending == 0 {
                return;
            }

            sleep(Duration::from_micros(200));
        }
    }

    fn write_command(&mut self, command: &str) -> Result<()> {
        self.socket.write_all(command.as_bytes())?;
        self.socket.write_all(b"\r\n")?;
        Ok(())
    }

    /// Read one CRLF-terminated line of the handshake.
    ///
    /// Byte at a time, because the handshake is the one part of the protocol
    /// that is not length-prefixed and we must not swallow any of the message
    /// stream that follows it.
    fn read_command(&mut self) -> Result<String> {
        let mut line = Vec::new();

        loop {
            let mut byte = [0u8; 1];
            self.socket.read_exact(&mut byte)?;

            if byte[0] == b'\n' {
                break;
            }
            if byte[0] != b'\r' {
                line.push(byte[0]);
            }

            if line.len() > 1024 {
                bail!("Peer sent an overlong authentication response");
            }
        }

        String::from_utf8(line).context("Peer sent a non-UTF-8 authentication response")
    }

    /// Call `method` and wait for its reply.
    pub fn call<B>(&mut self, method: &Method<'_>, body: &B) -> Result<Reply>
    where
        B: Serialize + DynamicType + ?Sized,
    {
        let serial = self.send(method, body, false)?;

        loop {
            let reply = self.receive()?;
            if reply.reply_serial == Some(serial) {
                return reply.into_result(method.member);
            }
            // Whatever else turns up is either a signal, which the next
            // waiter may be after, or a reply to a call we gave up on.
            self.buffer_signal(reply);
        }
    }

    /// Wait for the peer to emit `member`, until `deadline`.
    ///
    /// Signals only arrive at all once the peer has been asked for them, which
    /// for systemd means calling Subscribe.
    pub fn wait_for_signal(&mut self, member: &str, deadline: Instant) -> Result<Reply> {
        while let Some((name, reply)) = self.signals.pop_front() {
            if name == member {
                return Ok(reply);
            }
        }

        loop {
            // The socket timeout is about a peer that has gone quiet, while
            // the deadline is about how long the thing we are waiting for may
            // legitimately take. A restart outlasting a read is normal.
            match self.receive() {
                Ok(reply) => {
                    if reply.message_type == MESSAGE_TYPE_SIGNAL
                        && reply.member.as_deref() == Some(member)
                    {
                        return Ok(reply.reply);
                    }
                }
                Err(error) if !is_timeout(&error) => return Err(error),
                Err(_) => {}
            }

            if Instant::now() >= deadline {
                bail!("Timed out waiting for the peer to emit {member}");
            }
        }
    }

    fn buffer_signal(&mut self, reply: RawReply) {
        if reply.message_type != MESSAGE_TYPE_SIGNAL {
            return;
        }

        if let Some(member) = reply.member {
            if self.signals.len() == MAX_BUFFERED_SIGNALS {
                self.signals.pop_front();
            }
            self.signals.push_back((member, reply.reply));
        }
    }

    /// Call `method` without waiting for a reply.
    ///
    /// Only correct for methods whose completion the caller does not depend
    /// on: nothing here reports whether the peer even accepted the call.
    pub fn call_no_reply<B>(&mut self, method: &Method<'_>, body: &B) -> Result<()>
    where
        B: Serialize + DynamicType + ?Sized,
    {
        self.send(method, body, true)?;
        Ok(())
    }

    fn send<B>(&mut self, method: &Method<'_>, body: &B, no_reply: bool) -> Result<u32>
    where
        B: Serialize + DynamicType + ?Sized,
    {
        self.serial = self.serial.wrapping_add(1);
        let serial = self.serial;

        let message = build_method_call(serial, method, body, no_reply)?;
        self.socket
            .write_all(&message)
            .with_context(|| format!("Failed to send the {} call", method.member))?;

        Ok(serial)
    }

    fn receive(&mut self) -> Result<RawReply> {
        let mut bytes = vec![0u8; PRIMARY_HEADER_LEN];
        self.socket
            .read_exact(&mut bytes)
            .map_err(describe_read_error)
            .context("Failed to read a message header")?;

        let endian = match bytes[0] {
            b'l' => Endian::Little,
            b'B' => Endian::Big,
            other => bail!("Message declares an unknown byte order: {other:#x}"),
        };
        let message_type = bytes[1];
        let body_len = read_u32(&bytes[4..8], endian);
        let fields_len = read_u32(&bytes[12..16], endian);

        // The field array is followed by padding to the body's 8 byte
        // alignment, even when the body is empty.
        let fields_end = PRIMARY_HEADER_LEN + fields_len as usize;
        let body_offset = fields_end + padding_to_8(fields_end);
        let total_len = body_offset as u64 + body_len as u64;
        if total_len > MAX_MESSAGE_LEN as u64 {
            bail!("Message is too large: {total_len} bytes");
        }

        bytes.resize(total_len as usize, 0);
        self.socket
            .read_exact(&mut bytes[PRIMARY_HEADER_LEN..])
            .map_err(describe_read_error)
            .context("Failed to read a message body")?;

        let ctxt = EncodingContext::new_dbus(endian, PRIMARY_HEADER_LEN - 4);
        let data = Data::new(&bytes[PRIMARY_HEADER_LEN - 4..fields_end], ctxt);
        let (fields, _) = data
            .deserialize::<Vec<(u8, OwnedValue)>>()
            .context("Failed to deserialize the message header fields")?;

        let mut reply_serial = None;
        let mut error_name = None;
        let mut member = None;
        for (code, value) in fields {
            match code {
                FIELD_REPLY_SERIAL => reply_serial = u32::try_from(value).ok(),
                FIELD_ERROR_NAME => error_name = String::try_from(value).ok(),
                FIELD_MEMBER => member = String::try_from(value).ok(),
                _ => {}
            }
        }

        Ok(RawReply {
            reply: Reply {
                bytes,
                body_offset,
                endian,
            },
            message_type,
            reply_serial,
            error_name,
            member,
        })
    }
}

struct RawReply {
    reply: Reply,
    message_type: u8,
    reply_serial: Option<u32>,
    error_name: Option<String>,
    member: Option<String>,
}

impl RawReply {
    fn into_result(self, member: &str) -> Result<Reply> {
        match self.message_type {
            MESSAGE_TYPE_METHOD_RETURN => Ok(self.reply),
            MESSAGE_TYPE_ERROR => {
                let name = self
                    .error_name
                    .unwrap_or_else(|| "an unnamed error".to_string());
                match self.reply.body::<String>() {
                    Ok(message) => Err(anyhow!("{member} failed: {name}: {message}")),
                    Err(_) => Err(anyhow!("{member} failed: {name}")),
                }
            }
            other => Err(anyhow!(
                "{member} got a reply of unexpected type {other} instead of a return value"
            )),
        }
    }
}

/// The socket timeout expiring, kept as its own error so that a caller who is
/// prepared to wait longer can tell it apart from a broken connection.
#[derive(Debug)]
struct TimedOut;

impl std::fmt::Display for TimedOut {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "timed out waiting for the peer to answer")
    }
}

impl std::error::Error for TimedOut {}

fn describe_read_error(error: std::io::Error) -> anyhow::Error {
    match error.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => anyhow!(TimedOut),
        _ => error.into(),
    }
}

fn is_timeout(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.is::<TimedOut>())
}

fn read_u32(bytes: &[u8], endian: Endian) -> u32 {
    let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match endian {
        Endian::Little => u32::from_le_bytes(bytes),
        Endian::Big => u32::from_be_bytes(bytes),
    }
}

fn build_method_call<B>(
    serial: u32,
    method: &Method<'_>,
    body: &B,
    no_reply: bool,
) -> Result<Vec<u8>>
where
    B: Serialize + DynamicType + ?Sized,
{
    let endian = Endian::native();
    let ctxt = EncodingContext::new_dbus(endian, 0);

    let body_bytes = zvariant::to_bytes(ctxt, body)
        .with_context(|| format!("Failed to serialize the {} call arguments", method.member))?;
    let body_bytes = body_bytes.bytes();

    let mut fields = vec![
        Field::value(
            FIELD_PATH,
            Value::ObjectPath(ObjectPath::try_from(method.path).context("Invalid object path")?),
        ),
        Field::value(FIELD_INTERFACE, Value::Str(method.interface.into())),
        Field::value(FIELD_MEMBER, Value::Str(method.member.into())),
        Field::value(FIELD_DESTINATION, Value::Str(method.destination.into())),
    ];
    let signature = body.signature();
    if !body_bytes.is_empty() {
        fields.push(Field::signature(FIELD_SIGNATURE, &signature));
    }

    let flags = if no_reply { FLAG_NO_REPLY_EXPECTED } else { 0 };
    let header = (
        endian_code(endian),
        MESSAGE_TYPE_METHOD_CALL,
        flags,
        PROTOCOL_VERSION,
        body_bytes.len() as u32,
        serial,
        fields,
    );

    let header_bytes =
        zvariant::to_bytes(ctxt, &header).context("Failed to serialize the message header")?;
    let header_bytes = header_bytes.bytes();

    let mut message = Vec::with_capacity(
        header_bytes.len() + padding_to_8(header_bytes.len()) + body_bytes.len(),
    );
    message.extend_from_slice(header_bytes);
    message.resize(header_bytes.len() + padding_to_8(header_bytes.len()), 0);
    message.extend_from_slice(body_bytes);

    Ok(message)
}

/// One entry of the header field array: a field code and its value.
#[derive(Type)]
#[zvariant(signature = "(yv)")]
struct Field<'a> {
    code: u8,
    value: FieldValue<'a>,
}

enum FieldValue<'a> {
    Value(Value<'a>),
    Signature(&'a Signature),
}

impl<'a> Field<'a> {
    fn value(code: u8, value: Value<'a>) -> Self {
        Self {
            code,
            value: FieldValue::Value(value),
        }
    }

    fn signature(code: u8, signature: &'a Signature) -> Self {
        Self {
            code,
            value: FieldValue::Signature(signature),
        }
    }
}

impl Serialize for Field<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut field = serializer.serialize_struct("Field", 2)?;
        field.serialize_field("code", &self.code)?;
        match &self.value {
            FieldValue::Value(value) => field.serialize_field("value", value)?,
            FieldValue::Signature(signature) => {
                field.serialize_field("value", &BodySignature(signature))?
            }
        }
        field.end()
    }
}

/// A variant holding the signature of a message body.
///
/// A body's signature is the signature of each of its arguments in turn, while
/// serializing the argument tuple describes it as a structure. The bytes on
/// the wire are the same either way, but the peer matches the declared
/// signature against the method it is dispatching to: a two argument call has
/// to announce itself as `ss` and not as `(ss)`.
#[derive(Type)]
#[zvariant(signature = "v")]
struct BodySignature<'a>(&'a Signature);

impl Serialize for BodySignature<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut variant = serializer.serialize_struct("Variant", 2)?;
        variant.serialize_field("signature", &Signature::Signature)?;
        variant.serialize_field("value", &self.0.to_string_no_parens())?;
        variant.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zvariant::OwnedObjectPath;

    /// Parse a message the way `receive` does, without a socket.
    fn parse(bytes: &[u8]) -> (u8, u32, Vec<(u8, OwnedValue)>, Reply) {
        let endian = match bytes[0] {
            b'l' => Endian::Little,
            _ => Endian::Big,
        };
        let body_len = read_u32(&bytes[4..8], endian);
        let serial = read_u32(&bytes[8..12], endian);
        let fields_len = read_u32(&bytes[12..16], endian);

        let fields_end = PRIMARY_HEADER_LEN + fields_len as usize;
        let body_offset = fields_end + padding_to_8(fields_end);
        assert_eq!(bytes.len(), body_offset + body_len as usize);

        let ctxt = EncodingContext::new_dbus(endian, PRIMARY_HEADER_LEN - 4);
        let data = Data::new(&bytes[PRIMARY_HEADER_LEN - 4..fields_end], ctxt);
        let (fields, _) = data.deserialize::<Vec<(u8, OwnedValue)>>().unwrap();

        (
            bytes[1],
            serial,
            fields,
            Reply {
                bytes: bytes.to_vec(),
                body_offset,
                endian,
            },
        )
    }

    fn field(fields: &[(u8, OwnedValue)], code: u8) -> Option<String> {
        fields
            .iter()
            .find(|(field_code, _)| *field_code == code)
            .map(|(_, value)| match &**value {
                Value::Str(value) => value.to_string(),
                Value::ObjectPath(value) => value.to_string(),
                Value::Signature(value) => value.to_string_no_parens(),
                value => value.to_string(),
            })
    }

    fn manager(member: &str) -> Method<'_> {
        Method {
            destination: "org.freedesktop.systemd1",
            path: "/org/freedesktop/systemd1",
            interface: "org.freedesktop.systemd1.Manager",
            member,
        }
    }

    #[test]
    fn method_call_carries_its_arguments() {
        let message = build_method_call(
            7,
            &manager("RestartUnit"),
            &("containerd.service", "replace"),
            false,
        )
        .unwrap();

        let (message_type, serial, fields, reply) = parse(&message);
        assert_eq!(message_type, MESSAGE_TYPE_METHOD_CALL);
        assert_eq!(serial, 7);
        assert_eq!(message[2], 0);
        assert_eq!(
            field(&fields, FIELD_PATH).as_deref(),
            Some("/org/freedesktop/systemd1")
        );
        assert_eq!(
            field(&fields, FIELD_INTERFACE).as_deref(),
            Some("org.freedesktop.systemd1.Manager")
        );
        assert_eq!(field(&fields, FIELD_MEMBER).as_deref(), Some("RestartUnit"));
        assert_eq!(
            field(&fields, FIELD_DESTINATION).as_deref(),
            Some("org.freedesktop.systemd1")
        );
        assert_eq!(field(&fields, FIELD_SIGNATURE).as_deref(), Some("ss"));

        let body: (String, String) = reply.body().unwrap();
        assert_eq!(body.0, "containerd.service");
        assert_eq!(body.1, "replace");
    }

    #[test]
    fn method_call_without_arguments_has_no_body() {
        let message = build_method_call(1, &manager("Reload"), &(), true).unwrap();

        let (_, _, fields, _) = parse(&message);
        assert_eq!(read_u32(&message[4..8], Endian::native()), 0);
        assert_eq!(message[2], FLAG_NO_REPLY_EXPECTED);
        assert!(field(&fields, FIELD_SIGNATURE).is_none());
    }

    #[test]
    fn array_arguments_keep_their_signature() {
        let message = build_method_call(
            2,
            &manager("EnableUnitFiles"),
            &(vec!["kata-cleanup.service"], false, false),
            false,
        )
        .unwrap();

        let (_, _, fields, reply) = parse(&message);
        assert_eq!(field(&fields, FIELD_SIGNATURE).as_deref(), Some("asbb"));

        let body: (Vec<String>, bool, bool) = reply.body().unwrap();
        assert_eq!(body.0, vec!["kata-cleanup.service".to_string()]);
        assert!(!body.1);
        assert!(!body.2);
    }

    /// A signal, built out of a method call: the two differ in the type byte,
    /// and everything we read off a signal is read the same way for both.
    fn signal(member: &str, body: &(u32, OwnedObjectPath, String, String)) -> Vec<u8> {
        let mut message = build_method_call(1, &manager(member), body, false).unwrap();
        message[1] = MESSAGE_TYPE_SIGNAL;
        message
    }

    fn connected_to(peer: UnixStream, timeout: Duration) -> Connection {
        peer.set_read_timeout(Some(timeout)).unwrap();
        Connection {
            socket: peer,
            serial: 0,
            timeout,
            signals: VecDeque::new(),
        }
    }

    #[test]
    fn waiting_for_a_signal_passes_over_the_others() {
        let (mut peer, ours) = UnixStream::pair().unwrap();
        let mut connection = connected_to(ours, Duration::from_millis(200));

        let job = OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/42").unwrap();
        let unit = "containerd.service".to_string();
        peer.write_all(&signal(
            "JobNew",
            &(42, job.clone(), unit.clone(), String::new()),
        ))
        .unwrap();
        peer.write_all(&signal(
            "JobRemoved",
            &(42, job.clone(), unit.clone(), "done".to_string()),
        ))
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let reply = connection.wait_for_signal("JobRemoved", deadline).unwrap();

        let body: (u32, OwnedObjectPath, String, String) = reply.body().unwrap();
        assert_eq!(body, (42, job, unit, "done".to_string()));
    }

    /// A job we never hear about again must not hold the caller forever.
    #[test]
    fn waiting_for_a_signal_gives_up_at_the_deadline() {
        let (_peer, ours) = UnixStream::pair().unwrap();
        let mut connection = connected_to(ours, Duration::from_millis(50));

        let deadline = Instant::now() + Duration::from_millis(100);
        let error = match connection.wait_for_signal("JobRemoved", deadline) {
            Err(error) => error,
            Ok(_) => panic!("a silent peer produced a signal"),
        };

        assert!(
            format!("{error:#}").contains("JobRemoved"),
            "unexpected error: {error:#}"
        );
    }

    /// The timeout is the only thing standing between us and an install that
    /// hangs for as long as the pod lives, so check that it is in force.
    #[test]
    fn a_peer_that_never_answers_times_out() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("socket");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();

        // Accept, then say nothing at all.
        let accepted = std::thread::spawn(move || {
            let connection = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(5));
            drop(connection);
        });

        let started = std::time::Instant::now();
        let error = match Connection::connect(&path, Duration::from_millis(200)) {
            Err(error) => error,
            Ok(_) => panic!("connecting to a silent peer succeeded"),
        };

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "connecting took {:?}",
            started.elapsed()
        );
        assert!(
            format!("{error:#}").contains("Failed to authenticate"),
            "unexpected error: {error:#}"
        );

        accepted.join().unwrap();
    }

    /// Arrays of structures are where a misplaced pad byte hides: every entry
    /// after the first depends on the one before it being sized correctly.
    #[test]
    fn arrays_of_structures_survive_a_round_trip() {
        let jobs = vec![
            (
                1u32,
                "containerd.service".to_string(),
                "restart".to_string(),
                "running".to_string(),
                OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/1").unwrap(),
                OwnedObjectPath::try_from("/org/freedesktop/systemd1/unit/containerd_2eservice")
                    .unwrap(),
            ),
            (
                2u32,
                "kubelet.service".to_string(),
                "start".to_string(),
                "waiting".to_string(),
                OwnedObjectPath::try_from("/org/freedesktop/systemd1/job/2").unwrap(),
                OwnedObjectPath::try_from("/org/freedesktop/systemd1/unit/kubelet_2eservice")
                    .unwrap(),
            ),
        ];

        let message = build_method_call(3, &manager("ListJobs"), &(jobs.clone(),), false).unwrap();

        let (_, _, fields, reply) = parse(&message);
        assert_eq!(
            field(&fields, FIELD_SIGNATURE).as_deref(),
            Some("a(usssoo)")
        );

        let body: Vec<(
            u32,
            String,
            String,
            String,
            OwnedObjectPath,
            OwnedObjectPath,
        )> = reply.body().unwrap();
        assert_eq!(body, jobs);
    }
}
