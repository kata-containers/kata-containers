use crate::error::Error;
use crate::key;
#[cfg(feature = "logging")]
use crate::log::{debug, error, trace, warn};
use crate::msgs::alert::AlertMessagePayload;
use crate::msgs::base::Payload;
use crate::msgs::deframer::MessageDeframer;
use crate::msgs::enums::HandshakeType;
use crate::msgs::enums::{AlertDescription, AlertLevel, ContentType, ProtocolVersion};
use crate::msgs::fragmenter::MessageFragmenter;
use crate::msgs::handshake::Random;
use crate::msgs::hsjoiner::HandshakeJoiner;
use crate::msgs::message::{
    BorrowedPlainMessage, Message, MessagePayload, OpaqueMessage, PlainMessage,
};
#[cfg(feature = "quic")]
use crate::quic;
use crate::record_layer;
use crate::suites::SupportedCipherSuite;
#[cfg(feature = "tls12")]
use crate::tls12::ConnectionSecrets;
use crate::vecbuf::ChunkVecBuffer;

use std::collections::VecDeque;
use std::convert::TryFrom;
use std::io;
use std::mem;
use std::ops::{Deref, DerefMut};

/// A client or server connection.
pub enum Connection {
    /// A client connection
    Client(crate::client::ClientConnection),
    /// A server connection
    Server(crate::server::ServerConnection),
}

impl Connection {
    /// Read TLS content from `rd`.
    ///
    /// See [`ConnectionCommon::read_tls()`] for more information.
    pub fn read_tls(&mut self, rd: &mut dyn io::Read) -> Result<usize, io::Error> {
        match self {
            Connection::Client(conn) => conn.read_tls(rd),
            Connection::Server(conn) => conn.read_tls(rd),
        }
    }

    /// Returns an object that allows reading plaintext.
    pub fn reader(&mut self) -> Reader {
        match self {
            Connection::Client(conn) => conn.reader(),
            Connection::Server(conn) => conn.reader(),
        }
    }

    /// Returns an object that allows writing plaintext.
    pub fn writer(&mut self) -> Writer {
        match self {
            Connection::Client(conn) => Writer::new(&mut **conn),
            Connection::Server(conn) => Writer::new(&mut **conn),
        }
    }

    /// Processes any new packets read by a previous call to [`Connection::read_tls`].
    ///
    /// See [`ConnectionCommon::process_new_packets()`] for more information.
    pub fn process_new_packets(&mut self) -> Result<IoState, Error> {
        match self {
            Connection::Client(conn) => conn.process_new_packets(),
            Connection::Server(conn) => conn.process_new_packets(),
        }
    }

    /// Derives key material from the agreed connection secrets.
    ///
    /// See [`ConnectionCommon::export_keying_material()`] for more information.
    pub fn export_keying_material(
        &self,
        output: &mut [u8],
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<(), Error> {
        match self {
            Connection::Client(conn) => conn.export_keying_material(output, label, context),
            Connection::Server(conn) => conn.export_keying_material(output, label, context),
        }
    }

    /// This function uses `io` to complete any outstanding IO for this connection.
    ///
    /// See [`ConnectionCommon::complete_io()`] for more information.
    pub fn complete_io<T>(&mut self, io: &mut T) -> Result<(usize, usize), io::Error>
    where
        Self: Sized,
        T: io::Read + io::Write,
    {
        match self {
            Connection::Client(conn) => conn.complete_io(io),
            Connection::Server(conn) => conn.complete_io(io),
        }
    }
}

#[cfg(feature = "quic")]
impl crate::quic::QuicExt for Connection {
    fn quic_transport_parameters(&self) -> Option<&[u8]> {
        match self {
            Connection::Client(conn) => conn.quic_transport_parameters(),
            Connection::Server(conn) => conn.quic_transport_parameters(),
        }
    }

    fn zero_rtt_keys(&self) -> Option<quic::DirectionalKeys> {
        match self {
            Connection::Client(conn) => conn.zero_rtt_keys(),
            Connection::Server(conn) => conn.zero_rtt_keys(),
        }
    }

    fn read_hs(&mut self, plaintext: &[u8]) -> Result<(), Error> {
        match self {
            Connection::Client(conn) => conn.read_quic_hs(plaintext),
            Connection::Server(conn) => conn.read_quic_hs(plaintext),
        }
    }

    fn write_hs(&mut self, buf: &mut Vec<u8>) -> Option<quic::KeyChange> {
        match self {
            Connection::Client(conn) => quic::write_hs(conn, buf),
            Connection::Server(conn) => quic::write_hs(conn, buf),
        }
    }

    fn alert(&self) -> Option<AlertDescription> {
        match self {
            Connection::Client(conn) => conn.alert(),
            Connection::Server(conn) => conn.alert(),
        }
    }
}

impl Deref for Connection {
    type Target = CommonState;

    fn deref(&self) -> &Self::Target {
        match self {
            Connection::Client(conn) => &conn.common_state,
            Connection::Server(conn) => &conn.common_state,
        }
    }
}

impl DerefMut for Connection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Connection::Client(conn) => &mut conn.common_state,
            Connection::Server(conn) => &mut conn.common_state,
        }
    }
}

/// Values of this structure are returned from [`Connection::process_new_packets`]
/// and tell the caller the current I/O state of the TLS connection.
#[derive(Debug, PartialEq)]
pub struct IoState {
    tls_bytes_to_write: usize,
    plaintext_bytes_to_read: usize,
    peer_has_closed: bool,
}

impl IoState {
    /// How many bytes could be written by [`CommonState::write_tls`] if called
    /// right now.  A non-zero value implies [`CommonState::wants_write`].
    pub fn tls_bytes_to_write(&self) -> usize {
        self.tls_bytes_to_write
    }

    /// How many plaintext bytes could be obtained via [`std::io::Read`]
    /// without further I/O.
    pub fn plaintext_bytes_to_read(&self) -> usize {
        self.plaintext_bytes_to_read
    }

    /// True if the peer has sent us a close_notify alert.  This is
    /// the TLS mechanism to securely half-close a TLS connection,
    /// and signifies that the peer will not send any further data
    /// on this connection.
    ///
    /// This is also signalled via returning `Ok(0)` from
    /// [`std::io::Read`], after all the received bytes have been
    /// retrieved.
    pub fn peer_has_closed(&self) -> bool {
        self.peer_has_closed
    }
}

/// A structure that implements [`std::io::Read`] for reading plaintext.
pub struct Reader<'a> {
    received_plaintext: &'a mut ChunkVecBuffer,
    peer_cleanly_closed: bool,
    has_seen_eof: bool,
}

impl<'a> io::Read for Reader<'a> {
    /// Obtain plaintext data received from the peer over this TLS connection.
    ///
    /// If the peer closes the TLS session cleanly, this returns `Ok(0)`  once all
    /// the pending data has been read. No further data can be received on that
    /// connection, so the underlying TCP connection should be half-closed too.
    ///
    /// If the peer closes the TLS session uncleanly (a TCP EOF without sending a
    /// `close_notify` alert) this function returns `Err(ErrorKind::UnexpectedEof.into())`
    /// once any pending data has been read.
    ///
    /// Note that support for `close_notify` varies in peer TLS libraries: many do not
    /// support it and uncleanly close the TCP connection (this might be
    /// vulnerable to truncation attacks depending on the application protocol).
    /// This means applications using rustls must both handle EOF
    /// from this function, *and* unexpected EOF of the underlying TCP connection.
    ///
    /// If there are no bytes to read, this returns `Err(ErrorKind::WouldBlock.into())`.
    ///
    /// You may learn the number of bytes available at any time by inspecting
    /// the return of [`Connection::process_new_packets`].
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.received_plaintext.read(buf)?;

        if len == 0 && !buf.is_empty() {
            // No bytes available:
            match (self.peer_cleanly_closed, self.has_seen_eof) {
                // cleanly closed; don't care about TCP EOF: express this as Ok(0)
                (true, _) => {}
                // unclean closure
                (false, true) => return Err(io::ErrorKind::UnexpectedEof.into()),
                // connection still going, but need more data: signal `WouldBlock` so that
                // the caller knows this
                (false, false) => return Err(io::ErrorKind::WouldBlock.into()),
            }
        }

        Ok(len)
    }

    /// Obtain plaintext data received from the peer over this TLS connection.
    ///
    /// If the peer closes the TLS session, this returns `Ok(())` without filling
    /// any more of the buffer once all the pending data has been read. No further
    /// data can be received on that connection, so the underlying TCP connection
    /// should be half-closed too.
    ///
    /// If the peer closes the TLS session uncleanly (a TCP EOF without sending a
    /// `close_notify` alert) this function returns `Err(ErrorKind::UnexpectedEof.into())`
    /// once any pending data has been read.
    ///
    /// Note that support for `close_notify` varies in peer TLS libraries: many do not
    /// support it and uncleanly close the TCP connection (this might be
    /// vulnerable to truncation attacks depending on the application protocol).
    /// This means applications using rustls must both handle EOF
    /// from this function, *and* unexpected EOF of the underlying TCP connection.
    ///
    /// If there are no bytes to read, this returns `Err(ErrorKind::WouldBlock.into())`.
    ///
    /// You may learn the number of bytes available at any time by inspecting
    /// the return of [`Connection::process_new_packets`].
    #[cfg(read_buf)]
    fn read_buf(&mut self, buf: &mut io::ReadBuf<'_>) -> io::Result<()> {
        let before = buf.filled_len();
        self.received_plaintext.read_buf(buf)?;
        let len = buf.filled_len() - before;

        if len == 0 && buf.capacity() > 0 {
            // No bytes available:
            match (self.peer_cleanly_closed, self.has_seen_eof) {
                // cleanly closed; don't care about TCP EOF: express this as Ok(0)
                (true, _) => {}
                // unclean closure
                (false, true) => return Err(io::ErrorKind::UnexpectedEof.into()),
                // connection still going, but need more data: signal `WouldBlock` so that
                // the caller knows this
                (false, false) => return Err(io::ErrorKind::WouldBlock.into()),
            }
        }

        Ok(())
    }
}

/// Internal trait implemented by the [`ServerConnection`]/[`ClientConnection`]
/// allowing them to be the subject of a [`Writer`].
pub trait PlaintextSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize>;
    fn flush(&mut self) -> io::Result<()>;
}

impl<T> PlaintextSink for ConnectionCommon<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(self.send_some_plaintext(buf))
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        let mut sz = 0;
        for buf in bufs {
            sz += self.send_some_plaintext(buf);
        }
        Ok(sz)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A structure that implements [`std::io::Write`] for writing plaintext.
pub struct Writer<'a> {
    sink: &'a mut dyn PlaintextSink,
}

impl<'a> Writer<'a> {
    /// Create a new Writer.
    ///
    /// This is not an external interface.  Get one of these objects
    /// from [`Connection::writer`].
    #[doc(hidden)]
    pub fn new(sink: &'a mut dyn PlaintextSink) -> Writer<'a> {
        Writer { sink }
    }
}

impl<'a> io::Write for Writer<'a> {
    /// Send the plaintext `buf` to the peer, encrypting
    /// and authenticating it.  Once this function succeeds
    /// you should call [`CommonState::write_tls`] which will output the
    /// corresponding TLS records.
    ///
    /// This function buffers plaintext sent before the
    /// TLS handshake completes, and sends it as soon
    /// as it can.  See [`CommonState::set_buffer_limit`] to control
    /// the size of this buffer.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sink.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.sink.write_vectored(bufs)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sink.flush()
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) enum Protocol {
    Tcp,
    #[cfg(feature = "quic")]
    Quic,
}

#[derive(Debug)]
pub(crate) struct ConnectionRandoms {
    pub(crate) client: [u8; 32],
    pub(crate) server: [u8; 32],
}

/// How many ChangeCipherSpec messages we accept and drop in TLS1.3 handshakes.
/// The spec says 1, but implementations (namely the boringssl test suite) get
/// this wrong.  BoringSSL itself accepts up to 32.
static TLS13_MAX_DROPPED_CCS: u8 = 2u8;

impl ConnectionRandoms {
    pub(crate) fn new(client: Random, server: Random) -> Self {
        Self {
            client: client.0,
            server: server.0,
        }
    }
}

// --- Common (to client and server) connection functions ---

fn is_valid_ccs(msg: &OpaqueMessage) -> bool {
    // nb. this is prior to the record layer, so is unencrypted. see
    // third paragraph of section 5 in RFC8446.
    msg.typ == ContentType::ChangeCipherSpec && msg.payload.0 == [0x01]
}

enum Limit {
    Yes,
    No,
}

/// Interface shared by client and server connections.
pub struct ConnectionCommon<Data> {
    state: Result<Box<dyn State<Data>>, Error>,
    pub(crate) data: Data,
    pub(crate) common_state: CommonState,
    message_deframer: MessageDeframer,
    handshake_joiner: HandshakeJoiner,
}

impl<Data> ConnectionCommon<Data> {
    pub(crate) fn new(state: Box<dyn State<Data>>, data: Data, common_state: CommonState) -> Self {
        Self {
            state: Ok(state),
            data,
            common_state,
            message_deframer: MessageDeframer::new(),
            handshake_joiner: HandshakeJoiner::new(),
        }
    }

    /// Returns an object that allows reading plaintext.
    pub fn reader(&mut self) -> Reader {
        Reader {
            received_plaintext: &mut self.common_state.received_plaintext,
            /// Are we done? i.e., have we processed all received messages, and received a
            /// close_notify to indicate that no new messages will arrive?
            peer_cleanly_closed: self
                .common_state
                .has_received_close_notify
                && !self.message_deframer.has_pending(),
            has_seen_eof: self.common_state.has_seen_eof,
        }
    }

    /// Returns an object that allows writing plaintext.
    pub fn writer(&mut self) -> Writer {
        Writer::new(self)
    }

    /// This function uses `io` to complete any outstanding IO for
    /// this connection.
    ///
    /// This is a convenience function which solely uses other parts
    /// of the public API.
    ///
    /// What this means depends on the connection  state:
    ///
    /// - If the connection [`is_handshaking`], then IO is performed until
    ///   the handshake is complete.
    /// - Otherwise, if [`wants_write`] is true, [`write_tls`] is invoked
    ///   until it is all written.
    /// - Otherwise, if [`wants_read`] is true, [`read_tls`] is invoked
    ///   once.
    ///
    /// The return value is the number of bytes read from and written
    /// to `io`, respectively.
    ///
    /// This function will block if `io` blocks.
    ///
    /// Errors from TLS record handling (i.e., from [`process_new_packets`])
    /// are wrapped in an `io::ErrorKind::InvalidData`-kind error.
    ///
    /// [`is_handshaking`]: CommonState::is_handshaking
    /// [`wants_read`]: CommonState::wants_read
    /// [`wants_write`]: CommonState::wants_write
    /// [`write_tls`]: CommonState::write_tls
    /// [`read_tls`]: ConnectionCommon::read_tls
    /// [`process_new_packets`]: ConnectionCommon::process_new_packets
    pub fn complete_io<T>(&mut self, io: &mut T) -> Result<(usize, usize), io::Error>
    where
        Self: Sized,
        T: io::Read + io::Write,
    {
        let until_handshaked = self.is_handshaking();
        let mut eof = false;
        let mut wrlen = 0;
        let mut rdlen = 0;

        loop {
            while self.wants_write() {
                wrlen += self.write_tls(io)?;
            }

            if !until_handshaked && wrlen > 0 {
                return Ok((rdlen, wrlen));
            }

            while !eof && self.wants_read() {
                let read_size = match self.read_tls(io) {
                    Ok(0) => {
                        eof = true;
                        Some(0)
                    }
                    Ok(n) => {
                        rdlen += n;
                        Some(n)
                    }
                    Err(ref err) if err.kind() == io::ErrorKind::Interrupted => None, // nothing to do
                    Err(err) => return Err(err),
                };
                if read_size.is_some() {
                    break;
                }
            }

            match self.process_new_packets() {
                Ok(_) => {}
                Err(e) => {
                    // In case we have an alert to send describing this error,
                    // try a last-gasp write -- but don't predate the primary
                    // error.
                    let _ignored = self.write_tls(io);

                    return Err(io::Error::new(io::ErrorKind::InvalidData, e));
                }
            };

            match (eof, until_handshaked, self.is_handshaking()) {
                (_, true, false) => return Ok((rdlen, wrlen)),
                (_, false, _) => return Ok((rdlen, wrlen)),
                (true, true, true) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                (..) => {}
            }
        }
    }

    /// Extract the first handshake message.
    ///
    /// This is a shortcut to the `process_new_packets()` -> `process_msg()` ->
    /// `process_handshake_messages()` path, specialized for the first handshake message.
    pub(crate) fn first_handshake_message(&mut self) -> Result<Option<Message>, Error> {
        if self.message_deframer.desynced {
            return Err(Error::CorruptMessage);
        }

        let msg = match self.message_deframer.frames.pop_front() {
            Some(msg) => msg,
            None => return Ok(None),
        };

        let msg = msg.into_plain_message();
        if !self.handshake_joiner.want_message(&msg) {
            return Err(Error::CorruptMessagePayload(ContentType::Handshake));
        }

        if self
            .handshake_joiner
            .take_message(msg)
            .is_none()
        {
            self.common_state
                .send_fatal_alert(AlertDescription::DecodeError);
            return Err(Error::CorruptMessagePayload(ContentType::Handshake));
        }

        self.common_state.aligned_handshake = self.handshake_joiner.is_empty();
        Ok(self.handshake_joiner.frames.pop_front())
    }

    pub(crate) fn replace_state(&mut self, new: Box<dyn State<Data>>) {
        self.state = Ok(new);
    }

    fn process_msg(
        &mut self,
        msg: OpaqueMessage,
        state: Box<dyn State<Data>>,
    ) -> Result<Box<dyn State<Data>>, Error> {
        // Drop CCS messages during handshake in TLS1.3
        if msg.typ == ContentType::ChangeCipherSpec
            && !self
                .common_state
                .may_receive_application_data
            && self.common_state.is_tls13()
        {
            if !is_valid_ccs(&msg)
                || self.common_state.received_middlebox_ccs > TLS13_MAX_DROPPED_CCS
            {
                // "An implementation which receives any other change_cipher_spec value or
                //  which receives a protected change_cipher_spec record MUST abort the
                //  handshake with an "unexpected_message" alert."
                self.common_state
                    .send_fatal_alert(AlertDescription::UnexpectedMessage);
                return Err(Error::PeerMisbehavedError(
                    "illegal middlebox CCS received".into(),
                ));
            } else {
                self.common_state.received_middlebox_ccs += 1;
                trace!("Dropping CCS");
                return Ok(state);
            }
        }

        // Decrypt if demanded by current state.
        let msg = match self
            .common_state
            .record_layer
            .is_decrypting()
        {
            true => match self.common_state.decrypt_incoming(msg) {
                Ok(None) => {
                    // message dropped
                    return Ok(state);
                }
                Err(e) => {
                    return Err(e);
                }
                Ok(Some(msg)) => msg,
            },
            false => msg.into_plain_message(),
        };

        // For handshake messages, we need to join them before parsing
        // and processing.
        if self.handshake_joiner.want_message(&msg) {
            // First decryptable handshake message concludes trial decryption
            self.common_state
                .record_layer
                .finish_trial_decryption();

            self.handshake_joiner
                .take_message(msg)
                .ok_or_else(|| {
                    self.common_state
                        .send_fatal_alert(AlertDescription::DecodeError);
                    Error::CorruptMessagePayload(ContentType::Handshake)
                })?;
            return self.process_new_handshake_messages(state);
        }

        // Now we can fully parse the message payload.
        let msg = Message::try_from(msg)?;

        // For alerts, we have separate logic.
        if let MessagePayload::Alert(alert) = &msg.payload {
            self.common_state.process_alert(alert)?;
            return Ok(state);
        }

        self.common_state
            .process_main_protocol(msg, state, &mut self.data)
    }

    /// Processes any new packets read by a previous call to
    /// [`Connection::read_tls`].
    ///
    /// Errors from this function relate to TLS protocol errors, and
    /// are fatal to the connection.  Future calls after an error will do
    /// no new work and will return the same error. After an error is
    /// received from [`process_new_packets`], you should not call [`read_tls`]
    /// any more (it will fill up buffers to no purpose). However, you
    /// may call the other methods on the connection, including `write`,
    /// `send_close_notify`, and `write_tls`. Most likely you will want to
    /// call `write_tls` to send any alerts queued by the error and then
    /// close the underlying connection.
    ///
    /// Success from this function comes with some sundry state data
    /// about the connection.
    ///
    /// [`read_tls`]: Connection::read_tls
    /// [`process_new_packets`]: Connection::process_new_packets
    pub fn process_new_packets(&mut self) -> Result<IoState, Error> {
        let mut state = match mem::replace(&mut self.state, Err(Error::HandshakeNotComplete)) {
            Ok(state) => state,
            Err(e) => {
                self.state = Err(e.clone());
                return Err(e);
            }
        };

        if self.message_deframer.desynced {
            return Err(Error::CorruptMessage);
        }

        while let Some(msg) = self.message_deframer.frames.pop_front() {
            match self.process_msg(msg, state) {
                Ok(new) => state = new,
                Err(e) => {
                    self.state = Err(e.clone());
                    return Err(e);
                }
            }
        }

        self.state = Ok(state);
        Ok(self.common_state.current_io_state())
    }

    fn process_new_handshake_messages(
        &mut self,
        mut state: Box<dyn State<Data>>,
    ) -> Result<Box<dyn State<Data>>, Error> {
        self.common_state.aligned_handshake = self.handshake_joiner.is_empty();
        while let Some(msg) = self.handshake_joiner.frames.pop_front() {
            state = self
                .common_state
                .process_main_protocol(msg, state, &mut self.data)?;
        }

        Ok(state)
    }

    pub(crate) fn send_some_plaintext(&mut self, buf: &[u8]) -> usize {
        if let Ok(st) = &mut self.state {
            st.perhaps_write_key_update(&mut self.common_state);
        }
        self.common_state
            .send_some_plaintext(buf)
    }

    /// Read TLS content from `rd`.  This method does internal
    /// buffering, so `rd` can supply TLS messages in arbitrary-
    /// sized chunks (like a socket or pipe might).
    ///
    /// You should call [`process_new_packets`] each time a call to
    /// this function succeeds.
    ///
    /// The returned error only relates to IO on `rd`.  TLS-level
    /// errors are emitted from [`process_new_packets`].
    ///
    /// This function returns `Ok(0)` when the underlying `rd` does
    /// so.  This typically happens when a socket is cleanly closed,
    /// or a file is at EOF.
    ///
    /// [`process_new_packets`]: Connection::process_new_packets
    pub fn read_tls(&mut self, rd: &mut dyn io::Read) -> Result<usize, io::Error> {
        let res = self.message_deframer.read(rd);
        if let Ok(0) = res {
            self.common_state.has_seen_eof = true;
        }
        res
    }

    /// Derives key material from the agreed connection secrets.
    ///
    /// This function fills in `output` with `output.len()` bytes of key
    /// material derived from the master session secret using `label`
    /// and `context` for diversification.
    ///
    /// See RFC5705 for more details on what this does and is for.
    ///
    /// For TLS1.3 connections, this function does not use the
    /// "early" exporter at any point.
    ///
    /// This function fails if called prior to the handshake completing;
    /// check with [`CommonState::is_handshaking`] first.
    pub fn export_keying_material(
        &self,
        output: &mut [u8],
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<(), Error> {
        match self.state.as_ref() {
            Ok(st) => st.export_keying_material(output, label, context),
            Err(e) => Err(e.clone()),
        }
    }
}

#[cfg(feature = "quic")]
impl<Data> ConnectionCommon<Data> {
    pub(crate) fn read_quic_hs(&mut self, plaintext: &[u8]) -> Result<(), Error> {
        let state = match mem::replace(&mut self.state, Err(Error::HandshakeNotComplete)) {
            Ok(state) => state,
            Err(e) => {
                self.state = Err(e.clone());
                return Err(e);
            }
        };

        let msg = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_3,
            payload: Payload::new(plaintext.to_vec()),
        };

        if self
            .handshake_joiner
            .take_message(msg)
            .is_none()
        {
            self.common_state.quic.alert = Some(AlertDescription::DecodeError);
            return Err(Error::CorruptMessage);
        }

        self.process_new_handshake_messages(state)
            .map(|state| self.state = Ok(state))
    }
}

impl<T> Deref for ConnectionCommon<T> {
    type Target = CommonState;

    fn deref(&self) -> &Self::Target {
        &self.common_state
    }
}

impl<T> DerefMut for ConnectionCommon<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.common_state
    }
}

/// Connection state common to both client and server connections.
pub struct CommonState {
    pub(crate) negotiated_version: Option<ProtocolVersion>,
    pub(crate) side: Side,
    pub(crate) record_layer: record_layer::RecordLayer,
    pub(crate) suite: Option<SupportedCipherSuite>,
    pub(crate) alpn_protocol: Option<Vec<u8>>,
    aligned_handshake: bool,
    pub(crate) may_send_application_data: bool,
    pub(crate) may_receive_application_data: bool,
    pub(crate) early_traffic: bool,
    sent_fatal_alert: bool,
    /// If the peer has signaled end of stream.
    has_received_close_notify: bool,
    has_seen_eof: bool,
    received_middlebox_ccs: u8,
    pub(crate) peer_certificates: Option<Vec<key::Certificate>>,
    message_fragmenter: MessageFragmenter,
    received_plaintext: ChunkVecBuffer,
    sendable_plaintext: ChunkVecBuffer,
    pub(crate) sendable_tls: ChunkVecBuffer,
    #[allow(dead_code)] // only read for QUIC
    /// Protocol whose key schedule should be used. Unused for TLS < 1.3.
    pub(crate) protocol: Protocol,
    #[cfg(feature = "quic")]
    pub(crate) quic: Quic,
}

impl CommonState {
    pub(crate) fn new(max_fragment_size: Option<usize>, side: Side) -> Result<Self, Error> {
        Ok(Self {
            negotiated_version: None,
            side,
            record_layer: record_layer::RecordLayer::new(),
            suite: None,
            alpn_protocol: None,
            aligned_handshake: true,
            may_send_application_data: false,
            may_receive_application_data: false,
            early_traffic: false,
            sent_fatal_alert: false,
            has_received_close_notify: false,
            has_seen_eof: false,
            received_middlebox_ccs: 0,
            peer_certificates: None,
            message_fragmenter: MessageFragmenter::new(max_fragment_size)
                .map_err(|_| Error::BadMaxFragmentSize)?,
            received_plaintext: ChunkVecBuffer::new(Some(0)),
            sendable_plaintext: ChunkVecBuffer::new(Some(DEFAULT_BUFFER_LIMIT)),
            sendable_tls: ChunkVecBuffer::new(Some(DEFAULT_BUFFER_LIMIT)),

            protocol: Protocol::Tcp,
            #[cfg(feature = "quic")]
            quic: Quic::new(),
        })
    }

    /// Returns true if the caller should call [`CommonState::write_tls`] as soon
    /// as possible.
    pub fn wants_write(&self) -> bool {
        !self.sendable_tls.is_empty()
    }

    /// Returns true if the connection is currently performing the TLS handshake.
    ///
    /// During this time plaintext written to the connection is buffered in memory. After
    /// [`Connection::process_new_packets`] has been called, this might start to return `false`
    /// while the final handshake packets still need to be extracted from the connection's buffers.
    pub fn is_handshaking(&self) -> bool {
        !(self.may_send_application_data && self.may_receive_application_data)
    }

    /// Retrieves the certificate chain used by the peer to authenticate.
    ///
    /// The order of the certificate chain is as it appears in the TLS
    /// protocol: the first certificate relates to the peer, the
    /// second certifies the first, the third certifies the second, and
    /// so on.
    ///
    /// This is made available for both full and resumed handshakes.
    ///
    /// For clients, this is the certificate chain of the server.
    ///
    /// For servers, this is the certificate chain of the client,
    /// if client authentication was completed.
    ///
    /// The return value is None until this value is available.
    pub fn peer_certificates(&self) -> Option<&[key::Certificate]> {
        self.peer_certificates.as_deref()
    }

    /// Retrieves the protocol agreed with the peer via ALPN.
    ///
    /// A return value of `None` after handshake completion
    /// means no protocol was agreed (because no protocols
    /// were offered or accepted by the peer).
    pub fn alpn_protocol(&self) -> Option<&[u8]> {
        self.get_alpn_protocol()
    }

    /// Retrieves the ciphersuite agreed with the peer.
    ///
    /// This returns None until the ciphersuite is agreed.
    pub fn negotiated_cipher_suite(&self) -> Option<SupportedCipherSuite> {
        self.suite
    }

    /// Retrieves the protocol version agreed with the peer.
    ///
    /// This returns `None` until the version is agreed.
    pub fn protocol_version(&self) -> Option<ProtocolVersion> {
        self.negotiated_version
    }

    pub(crate) fn is_tls13(&self) -> bool {
        matches!(self.negotiated_version, Some(ProtocolVersion::TLSv1_3))
    }

    fn process_main_protocol<Data>(
        &mut self,
        msg: Message,
        mut state: Box<dyn State<Data>>,
        data: &mut Data,
    ) -> Result<Box<dyn State<Data>>, Error> {
        // For TLS1.2, outside of the handshake, send rejection alerts for
        // renegotiation requests.  These can occur any time.
        if self.may_receive_application_data && !self.is_tls13() {
            let reject_ty = match self.side {
                Side::Client => HandshakeType::HelloRequest,
                Side::Server => HandshakeType::ClientHello,
            };
            if msg.is_handshake_type(reject_ty) {
                self.send_warning_alert(AlertDescription::NoRenegotiation);
                return Ok(state);
            }
        }

        let mut cx = Context { common: self, data };
        match state.handle(&mut cx, msg) {
            Ok(next) => {
                state = next;
                Ok(state)
            }
            Err(e @ Error::InappropriateMessage { .. })
            | Err(e @ Error::InappropriateHandshakeMessage { .. }) => {
                self.send_fatal_alert(AlertDescription::UnexpectedMessage);
                Err(e)
            }
            Err(e) => Err(e),
        }
    }

    /// Send plaintext application data, fragmenting and
    /// encrypting it as it goes out.
    ///
    /// If internal buffers are too small, this function will not accept
    /// all the data.
    pub(crate) fn send_some_plaintext(&mut self, data: &[u8]) -> usize {
        self.send_plain(data, Limit::Yes)
    }

    pub(crate) fn send_early_plaintext(&mut self, data: &[u8]) -> usize {
        debug_assert!(self.early_traffic);
        debug_assert!(self.record_layer.is_encrypting());

        if data.is_empty() {
            // Don't send empty fragments.
            return 0;
        }

        self.send_appdata_encrypt(data, Limit::Yes)
    }

    // Changing the keys must not span any fragmented handshake
    // messages.  Otherwise the defragmented messages will have
    // been protected with two different record layer protections,
    // which is illegal.  Not mentioned in RFC.
    pub(crate) fn check_aligned_handshake(&mut self) -> Result<(), Error> {
        if !self.aligned_handshake {
            self.send_fatal_alert(AlertDescription::UnexpectedMessage);
            Err(Error::PeerMisbehavedError(
                "key epoch or handshake flight with pending fragment".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn illegal_param(&mut self, why: &str) -> Error {
        self.send_fatal_alert(AlertDescription::IllegalParameter);
        Error::PeerMisbehavedError(why.to_string())
    }

    pub(crate) fn decrypt_incoming(
        &mut self,
        encr: OpaqueMessage,
    ) -> Result<Option<PlainMessage>, Error> {
        if self
            .record_layer
            .wants_close_before_decrypt()
        {
            self.send_close_notify();
        }

        let encrypted_len = encr.payload.0.len();
        let plain = self.record_layer.decrypt_incoming(encr);

        match plain {
            Err(Error::PeerSentOversizedRecord) => {
                self.send_fatal_alert(AlertDescription::RecordOverflow);
                Err(Error::PeerSentOversizedRecord)
            }
            Err(Error::DecryptError)
                if self
                    .record_layer
                    .doing_trial_decryption(encrypted_len) =>
            {
                trace!("Dropping undecryptable message after aborted early_data");
                Ok(None)
            }
            Err(Error::DecryptError) => {
                self.send_fatal_alert(AlertDescription::BadRecordMac);
                Err(Error::DecryptError)
            }
            Err(e) => Err(e),
            Ok(plain) => Ok(Some(plain)),
        }
    }

    /// Fragment `m`, encrypt the fragments, and then queue
    /// the encrypted fragments for sending.
    pub(crate) fn send_msg_encrypt(&mut self, m: PlainMessage) {
        let mut plain_messages = VecDeque::new();
        self.message_fragmenter
            .fragment(m, &mut plain_messages);

        for m in plain_messages {
            self.send_single_fragment(m.borrow());
        }
    }

    /// Like send_msg_encrypt, but operate on an appdata directly.
    fn send_appdata_encrypt(&mut self, payload: &[u8], limit: Limit) -> usize {
        // Here, the limit on sendable_tls applies to encrypted data,
        // but we're respecting it for plaintext data -- so we'll
        // be out by whatever the cipher+record overhead is.  That's a
        // constant and predictable amount, so it's not a terrible issue.
        let len = match limit {
            Limit::Yes => self
                .sendable_tls
                .apply_limit(payload.len()),
            Limit::No => payload.len(),
        };

        let mut plain_messages = VecDeque::new();
        self.message_fragmenter.fragment_borrow(
            ContentType::ApplicationData,
            ProtocolVersion::TLSv1_2,
            &payload[..len],
            &mut plain_messages,
        );

        for m in plain_messages {
            self.send_single_fragment(m);
        }

        len
    }

    fn send_single_fragment(&mut self, m: BorrowedPlainMessage) {
        // Close connection once we start to run out of
        // sequence space.
        if self
            .record_layer
            .wants_close_before_encrypt()
        {
            self.send_close_notify();
        }

        // Refuse to wrap counter at all costs.  This
        // is basically untestable unfortunately.
        if self.record_layer.encrypt_exhausted() {
            return;
        }

        let em = self.record_layer.encrypt_outgoing(m);
        self.queue_tls_message(em);
    }

    /// Writes TLS messages to `wr`.
    ///
    /// On success, this function returns `Ok(n)` where `n` is a number of bytes written to `wr`
    /// (after encoding and encryption).
    ///
    /// After this function returns, the connection buffer may not yet be fully flushed. The
    /// [`CommonState::wants_write`] function can be used to check if the output buffer is empty.
    pub fn write_tls(&mut self, wr: &mut dyn io::Write) -> Result<usize, io::Error> {
        self.sendable_tls.write_to(wr)
    }

    /// Encrypt and send some plaintext `data`.  `limit` controls
    /// whether the per-connection buffer limits apply.
    ///
    /// Returns the number of bytes written from `data`: this might
    /// be less than `data.len()` if buffer limits were exceeded.
    fn send_plain(&mut self, data: &[u8], limit: Limit) -> usize {
        if !self.may_send_application_data {
            // If we haven't completed handshaking, buffer
            // plaintext to send once we do.
            let len = match limit {
                Limit::Yes => self
                    .sendable_plaintext
                    .append_limited_copy(data),
                Limit::No => self
                    .sendable_plaintext
                    .append(data.to_vec()),
            };
            return len;
        }

        debug_assert!(self.record_layer.is_encrypting());

        if data.is_empty() {
            // Don't send empty fragments.
            return 0;
        }

        self.send_appdata_encrypt(data, limit)
    }

    pub(crate) fn start_outgoing_traffic(&mut self) {
        self.may_send_application_data = true;
        self.flush_plaintext();
    }

    pub(crate) fn start_traffic(&mut self) {
        self.may_receive_application_data = true;
        self.start_outgoing_traffic();
    }

    /// Sets a limit on the internal buffers used to buffer
    /// unsent plaintext (prior to completing the TLS handshake)
    /// and unsent TLS records.  This limit acts only on application
    /// data written through [`Connection::writer`].
    ///
    /// By default the limit is 64KB.  The limit can be set
    /// at any time, even if the current buffer use is higher.
    ///
    /// [`None`] means no limit applies, and will mean that written
    /// data is buffered without bound -- it is up to the application
    /// to appropriately schedule its plaintext and TLS writes to bound
    /// memory usage.
    ///
    /// For illustration: `Some(1)` means a limit of one byte applies:
    /// [`Connection::writer`] will accept only one byte, encrypt it and
    /// add a TLS header.  Once this is sent via [`CommonState::write_tls`],
    /// another byte may be sent.
    ///
    /// # Internal write-direction buffering
    /// rustls has two buffers whose size are bounded by this setting:
    ///
    /// ## Buffering of unsent plaintext data prior to handshake completion
    ///
    /// Calls to [`Connection::writer`] before or during the handshake
    /// are buffered (up to the limit specified here).  Once the
    /// handshake completes this data is encrypted and the resulting
    /// TLS records are added to the outgoing buffer.
    ///
    /// ## Buffering of outgoing TLS records
    ///
    /// This buffer is used to store TLS records that rustls needs to
    /// send to the peer.  It is used in these two circumstances:
    ///
    /// - by [`Connection::process_new_packets`] when a handshake or alert
    ///   TLS record needs to be sent.
    /// - by [`Connection::writer`] post-handshake: the plaintext is
    ///   encrypted and the resulting TLS record is buffered.
    ///
    /// This buffer is emptied by [`CommonState::write_tls`].
    pub fn set_buffer_limit(&mut self, limit: Option<usize>) {
        self.sendable_plaintext.set_limit(limit);
        self.sendable_tls.set_limit(limit);
    }

    /// Send any buffered plaintext.  Plaintext is buffered if
    /// written during handshake.
    fn flush_plaintext(&mut self) {
        if !self.may_send_application_data {
            return;
        }

        while let Some(buf) = self.sendable_plaintext.pop() {
            self.send_plain(&buf, Limit::No);
        }
    }

    // Put m into sendable_tls for writing.
    fn queue_tls_message(&mut self, m: OpaqueMessage) {
        self.sendable_tls.append(m.encode());
    }

    /// Send a raw TLS message, fragmenting it if needed.
    pub(crate) fn send_msg(&mut self, m: Message, must_encrypt: bool) {
        #[cfg(feature = "quic")]
        {
            if let Protocol::Quic = self.protocol {
                if let MessagePayload::Alert(alert) = m.payload {
                    self.quic.alert = Some(alert.description);
                } else {
                    debug_assert!(
                        matches!(m.payload, MessagePayload::Handshake { .. }),
                        "QUIC uses TLS for the cryptographic handshake only"
                    );
                    let mut bytes = Vec::new();
                    m.payload.encode(&mut bytes);
                    self.quic
                        .hs_queue
                        .push_back((must_encrypt, bytes));
                }
                return;
            }
        }
        if !must_encrypt {
            let mut to_send = VecDeque::new();
            self.message_fragmenter
                .fragment(m.into(), &mut to_send);
            for mm in to_send {
                self.queue_tls_message(mm.into_unencrypted_opaque());
            }
        } else {
            self.send_msg_encrypt(m.into());
        }
    }

    pub(crate) fn take_received_plaintext(&mut self, bytes: Payload) {
        self.received_plaintext.append(bytes.0);
    }

    #[cfg(feature = "tls12")]
    pub(crate) fn start_encryption_tls12(&mut self, secrets: &ConnectionSecrets, side: Side) {
        let (dec, enc) = secrets.make_cipher_pair(side);
        self.record_layer
            .prepare_message_encrypter(enc);
        self.record_layer
            .prepare_message_decrypter(dec);
    }

    #[cfg(feature = "quic")]
    pub(crate) fn missing_extension(&mut self, why: &str) -> Error {
        self.send_fatal_alert(AlertDescription::MissingExtension);
        Error::PeerMisbehavedError(why.to_string())
    }

    fn send_warning_alert(&mut self, desc: AlertDescription) {
        warn!("Sending warning alert {:?}", desc);
        self.send_warning_alert_no_log(desc);
    }

    fn process_alert(&mut self, alert: &AlertMessagePayload) -> Result<(), Error> {
        // Reject unknown AlertLevels.
        if let AlertLevel::Unknown(_) = alert.level {
            self.send_fatal_alert(AlertDescription::IllegalParameter);
        }

        // If we get a CloseNotify, make a note to declare EOF to our
        // caller.
        if alert.description == AlertDescription::CloseNotify {
            self.has_received_close_notify = true;
            return Ok(());
        }

        // Warnings are nonfatal for TLS1.2, but outlawed in TLS1.3
        // (except, for no good reason, user_cancelled).
        if alert.level == AlertLevel::Warning {
            if self.is_tls13() && alert.description != AlertDescription::UserCanceled {
                self.send_fatal_alert(AlertDescription::DecodeError);
            } else {
                warn!("TLS alert warning received: {:#?}", alert);
                return Ok(());
            }
        }

        error!("TLS alert received: {:#?}", alert);
        Err(Error::AlertReceived(alert.description))
    }

    pub(crate) fn send_fatal_alert(&mut self, desc: AlertDescription) {
        warn!("Sending fatal alert {:?}", desc);
        debug_assert!(!self.sent_fatal_alert);
        let m = Message::build_alert(AlertLevel::Fatal, desc);
        self.send_msg(m, self.record_layer.is_encrypting());
        self.sent_fatal_alert = true;
    }

    /// Queues a close_notify warning alert to be sent in the next
    /// [`CommonState::write_tls`] call.  This informs the peer that the
    /// connection is being closed.
    pub fn send_close_notify(&mut self) {
        debug!("Sending warning alert {:?}", AlertDescription::CloseNotify);
        self.send_warning_alert_no_log(AlertDescription::CloseNotify);
    }

    fn send_warning_alert_no_log(&mut self, desc: AlertDescription) {
        let m = Message::build_alert(AlertLevel::Warning, desc);
        self.send_msg(m, self.record_layer.is_encrypting());
    }

    pub(crate) fn set_max_fragment_size(&mut self, new: Option<usize>) -> Result<(), Error> {
        self.message_fragmenter
            .set_max_fragment_size(new)
    }

    pub(crate) fn get_alpn_protocol(&self) -> Option<&[u8]> {
        self.alpn_protocol
            .as_ref()
            .map(AsRef::as_ref)
    }

    /// Returns true if the caller should call [`Connection::read_tls`] as soon
    /// as possible.
    ///
    /// If there is pending plaintext data to read with [`Connection::reader`],
    /// this returns false.  If your application respects this mechanism,
    /// only one full TLS message will be buffered by rustls.
    pub fn wants_read(&self) -> bool {
        // We want to read more data all the time, except when we have unprocessed plaintext.
        // This provides back-pressure to the TCP buffers. We also don't want to read more after
        // the peer has sent us a close notification.
        //
        // In the handshake case we don't have readable plaintext before the handshake has
        // completed, but also don't want to read if we still have sendable tls.
        self.received_plaintext.is_empty()
            && !self.has_received_close_notify
            && (self.may_send_application_data || self.sendable_tls.is_empty())
    }

    fn current_io_state(&self) -> IoState {
        IoState {
            tls_bytes_to_write: self.sendable_tls.len(),
            plaintext_bytes_to_read: self.received_plaintext.len(),
            peer_has_closed: self.has_received_close_notify,
        }
    }

    pub(crate) fn is_quic(&self) -> bool {
        #[cfg(feature = "quic")]
        {
            self.protocol == Protocol::Quic
        }
        #[cfg(not(feature = "quic"))]
        false
    }
}

pub(crate) trait State<Data>: Send + Sync {
    fn handle(
        self: Box<Self>,
        cx: &mut Context<'_, Data>,
        message: Message,
    ) -> Result<Box<dyn State<Data>>, Error>;

    fn export_keying_material(
        &self,
        _output: &mut [u8],
        _label: &[u8],
        _context: Option<&[u8]>,
    ) -> Result<(), Error> {
        Err(Error::HandshakeNotComplete)
    }

    fn perhaps_write_key_update(&mut self, _cx: &mut CommonState) {}
}

pub(crate) struct Context<'a, Data> {
    pub(crate) common: &'a mut CommonState,
    pub(crate) data: &'a mut Data,
}

#[cfg(feature = "quic")]
pub(crate) struct Quic {
    /// QUIC transport parameters received from the peer during the handshake
    pub(crate) params: Option<Vec<u8>>,
    pub(crate) alert: Option<AlertDescription>,
    pub(crate) hs_queue: VecDeque<(bool, Vec<u8>)>,
    pub(crate) early_secret: Option<ring::hkdf::Prk>,
    pub(crate) hs_secrets: Option<quic::Secrets>,
    pub(crate) traffic_secrets: Option<quic::Secrets>,
    /// Whether keys derived from traffic_secrets have been passed to the QUIC implementation
    pub(crate) returned_traffic_keys: bool,
}

#[cfg(feature = "quic")]
impl Quic {
    fn new() -> Self {
        Self {
            params: None,
            alert: None,
            hs_queue: VecDeque::new(),
            early_secret: None,
            hs_secrets: None,
            traffic_secrets: None,
            returned_traffic_keys: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Side {
    Client,
    Server,
}

/// Data specific to the peer's side (client or server).
pub trait SideData {}

const DEFAULT_BUFFER_LIMIT: usize = 64 * 1024;
