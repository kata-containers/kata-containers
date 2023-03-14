#[cfg(unix)]
use nix::unistd::Uid;
use std::{
    collections::VecDeque,
    fmt,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
    str::FromStr,
    task::{Context, Poll},
};

#[cfg(windows)]
use crate::win32;
use crate::{
    guid::Guid,
    raw::{Connection, Socket},
    AuthMechanism, Error, Result,
};

use futures_core::ready;

/*
 * Client-side handshake logic
 */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
enum ClientHandshakeStep {
    Init,
    MechanismInit,
    WaitingForData,
    WaitingForOK,
    WaitingForAgreeUnixFD,
    Done,
}

// The plain-text SASL profile authentication protocol described here:
// <https://dbus.freedesktop.org/doc/dbus-specification.html#auth-protocol>
//
// These are all the known commands, which can be parsed from or serialized to text.
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
enum Command {
    Auth(Option<AuthMechanism>, Option<String>),
    Cancel,
    Begin,
    Data(Vec<u8>),
    Error(String),
    NegotiateUnixFD,
    Rejected(Vec<AuthMechanism>),
    Ok(Guid),
    AgreeUnixFD,
}

/// A representation of an in-progress handshake, client-side
///
/// This struct is an async-compatible representation of the initial handshake that must be performed before
/// a D-Bus connection can be used. To use it, you should call the [`advance_handshake`] method whenever the
/// underlying socket becomes ready (tracking the readiness itself is not managed by this abstraction) until
/// it returns `Ok(())`, at which point you can invoke the [`try_finish`] method to get an [`Authenticated`],
/// which can be given to [`Connection::new_authenticated`].
///
/// [`advance_handshake`]: struct.ClientHandshake.html#method.advance_handshake
/// [`try_finish`]: struct.ClientHandshake.html#method.try_finish
/// [`Authenticated`]: struct.AUthenticated.html
/// [`Connection::new_authenticated`]: ../struct.Connection.html#method.new_authenticated
#[derive(Debug)]
pub struct ClientHandshake<S> {
    socket: S,
    recv_buffer: Vec<u8>,
    send_buffer: Vec<u8>,
    step: ClientHandshakeStep,
    server_guid: Option<Guid>,
    cap_unix_fd: bool,
    // the current AUTH mechanism is front, ordered by priority
    mechanisms: VecDeque<AuthMechanism>,
}

/// The result of a finalized handshake
///
/// The result of a finalized [`ClientHandshake`] or [`ServerHandshake`]. It can be passed to
/// [`Connection::new_authenticated`] to initialize a connection.
///
/// [`ClientHandshake`]: struct.ClientHandshake.html
/// [`ServerHandshake`]: struct.ServerHandshake.html
/// [`Connection::new_authenticated`]: ../struct.Connection.html#method.new_authenticated
#[derive(Debug)]
pub struct Authenticated<S> {
    pub(crate) conn: Connection<S>,
    /// The server Guid
    pub(crate) server_guid: Guid,
    /// Whether file descriptor passing has been accepted by both sides
    #[cfg(unix)]
    pub(crate) cap_unix_fd: bool,
}

pub trait Handshake<S> {
    /// Attempt to advance the handshake
    ///
    /// In non-blocking mode, you need to invoke this method repeatedly
    /// until it returns `Ok(())`. Once it does, the handshake is finished
    /// and you can invoke the [`Handshake::try_finish`] method.
    ///
    /// Note that only the initial handshake is done. If you need to send a
    /// Bus Hello, this remains to be done.
    fn advance_handshake(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>>;

    /// Attempt to finalize this handshake into an initialized client.
    ///
    /// This method should only be called once `advance_handshake()` has
    /// returned `Ok(())`. Otherwise it'll error and return you the object.
    fn try_finish(self) -> std::result::Result<Authenticated<S>, Self>
    where
        Self: Sized;
}

impl<S: Socket> ClientHandshake<S> {
    /// Start a handshake on this client socket
    pub fn new(socket: S, mechanisms: Option<VecDeque<AuthMechanism>>) -> ClientHandshake<S> {
        let mechanisms = mechanisms.unwrap_or_else(|| {
            let mut mechanisms = VecDeque::new();
            mechanisms.push_back(AuthMechanism::External);
            mechanisms.push_back(AuthMechanism::Cookie);
            mechanisms.push_back(AuthMechanism::Anonymous);
            mechanisms
        });

        ClientHandshake {
            socket,
            recv_buffer: Vec::new(),
            send_buffer: Vec::new(),
            step: ClientHandshakeStep::Init,
            server_guid: None,
            cap_unix_fd: false,
            mechanisms,
        }
    }

    fn flush_buffer(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        while !self.send_buffer.is_empty() {
            let written = ready!(self.socket.poll_sendmsg(
                cx,
                &self.send_buffer,
                #[cfg(unix)]
                &[]
            ))?;
            self.send_buffer.drain(..written);
        }
        Ok(()).into()
    }

    fn read_command(&mut self, cx: &mut Context<'_>) -> Poll<Result<Command>> {
        self.recv_buffer.clear(); // maybe until \r\n instead?
        while !self.recv_buffer.ends_with(b"\r\n") {
            let mut buf = [0; 40];
            let res = ready!(self.socket.poll_recvmsg(cx, &mut buf))?;
            let read = {
                #[cfg(unix)]
                {
                    let (read, fds) = res;
                    if !fds.is_empty() {
                        return Poll::Ready(Err(Error::Handshake(
                            "Unexpected FDs during handshake".into(),
                        )));
                    }
                    read
                }
                #[cfg(not(unix))]
                {
                    res
                }
            };
            self.recv_buffer.extend(&buf[..read]);
        }

        let line = String::from_utf8_lossy(&self.recv_buffer);
        Poll::Ready(line.parse())
    }

    fn mechanism(&self) -> Result<&AuthMechanism> {
        self.mechanisms
            .front()
            .ok_or_else(|| Error::Handshake("Exhausted available AUTH mechanisms".into()))
    }

    fn mechanism_init(&mut self) -> Result<(ClientHandshakeStep, Command)> {
        use ClientHandshakeStep::*;
        let mech = self.mechanism()?;
        match mech {
            AuthMechanism::Anonymous => Ok((WaitingForOK, Command::Auth(Some(*mech), None))),
            AuthMechanism::External => Ok((
                WaitingForOK,
                Command::Auth(Some(*mech), Some(sasl_auth_id()?)),
            )),
            AuthMechanism::Cookie => Ok((
                WaitingForData,
                Command::Auth(Some(*mech), Some(sasl_auth_id()?)),
            )),
        }
    }

    fn mechanism_data(&mut self, data: Vec<u8>) -> Result<(ClientHandshakeStep, Command)> {
        use ClientHandshakeStep::*;
        let mech = self.mechanism()?;
        match mech {
            AuthMechanism::Cookie => {
                let context = String::from_utf8_lossy(&data);
                let mut split = context.split_ascii_whitespace();
                let name = split
                    .next()
                    .ok_or_else(|| Error::Handshake("Missing cookie context name".into()))?;
                let id = split
                    .next()
                    .ok_or_else(|| Error::Handshake("Missing cookie ID".into()))?;
                let server_chall = split
                    .next()
                    .ok_or_else(|| Error::Handshake("Missing cookie challenge".into()))?;

                let cookie = Cookie::lookup(name, id)?;
                let client_chall = random_ascii(16);
                let sec = format!("{}:{}:{}", server_chall, client_chall, cookie);
                let sha1 = sha1::Sha1::from(sec).hexdigest();
                let data = format!("{} {}", client_chall, sha1);
                Ok((WaitingForOK, Command::Data(data.into())))
            }
            _ => Err(Error::Handshake("Unexpected mechanism DATA".into())),
        }
    }
}

fn random_ascii(len: usize) -> String {
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    use std::iter;

    let mut rng = thread_rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(len)
        .collect()
}

fn sasl_auth_id() -> Result<String> {
    let id = {
        #[cfg(unix)]
        {
            Uid::current().to_string()
        }

        #[cfg(windows)]
        {
            win32::ProcessToken::open(None)?.sid()?
        }
    };

    Ok(hex::encode(id))
}

#[derive(Debug)]
struct Cookie {
    id: String,
    cookie: String,
}

impl Cookie {
    fn keyring_path() -> Result<PathBuf> {
        let mut path = dirs::home_dir()
            .ok_or_else(|| Error::Handshake("Failed to get home directory".into()))?;
        path.push(".dbus-keyrings");
        Ok(path)
    }

    fn read_keyring(name: &str) -> Result<Vec<Cookie>> {
        let mut path = Cookie::keyring_path()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let perms = std::fs::metadata(&path)?.permissions().mode();
            if perms & 0o066 != 0 {
                return Err(Error::Handshake(
                    "DBus keyring has invalid permissions".into(),
                ));
            }
        }
        #[cfg(not(unix))]
        {
            // FIXME: add code to check directory permissions
        }
        path.push(name);
        let file = File::open(&path)?;
        let mut cookies = vec![];
        for (n, line) in BufReader::new(file).lines().enumerate() {
            let line = line?;
            let mut split = line.split_whitespace();
            let id = split
                .next()
                .ok_or_else(|| {
                    Error::Handshake(format!(
                        "DBus cookie `{}` missing ID at line {}",
                        path.to_str().unwrap(),
                        n
                    ))
                })?
                .to_string();
            let _ = split.next().ok_or_else(|| {
                Error::Handshake(format!(
                    "DBus cookie `{}` missing creation time at line {}",
                    path.to_str().unwrap(),
                    n
                ))
            })?;
            let cookie = split
                .next()
                .ok_or_else(|| {
                    Error::Handshake(format!(
                        "DBus cookie `{}` missing cookie data at line {}",
                        path.to_str().unwrap(),
                        n
                    ))
                })?
                .to_string();
            cookies.push(Cookie { id, cookie })
        }
        Ok(cookies)
    }

    fn lookup(name: &str, id: &str) -> Result<String> {
        let keyring = Self::read_keyring(name)?;
        let c = keyring
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| Error::Handshake(format!("DBus cookie ID {} not found", id)))?;
        Ok(c.cookie.to_string())
    }
}

impl<S: Socket> Handshake<S> for ClientHandshake<S> {
    fn advance_handshake(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        use ClientHandshakeStep::*;
        loop {
            ready!(self.flush_buffer(cx))?;
            let (next_step, cmd) = match self.step {
                Init | MechanismInit => self.mechanism_init()?,
                WaitingForData | WaitingForOK => {
                    let reply = ready!(self.read_command(cx))?;
                    match (self.step, reply) {
                        (_, Command::Data(data)) => self.mechanism_data(data)?,
                        (_, Command::Rejected(_)) => {
                            self.mechanisms.pop_front();
                            self.step = MechanismInit;
                            continue;
                        }
                        (WaitingForOK, Command::Ok(guid)) => {
                            self.server_guid = Some(guid);
                            if self.socket.can_pass_unix_fd() {
                                (WaitingForAgreeUnixFD, Command::NegotiateUnixFD)
                            } else {
                                (Done, Command::Begin)
                            }
                        }
                        (_, reply) => {
                            return Poll::Ready(Err(Error::Handshake(format!(
                                "Unexpected server AUTH OK reply: {}",
                                reply
                            ))));
                        }
                    }
                }
                WaitingForAgreeUnixFD => {
                    let reply = ready!(self.read_command(cx))?;
                    match reply {
                        Command::AgreeUnixFD => self.cap_unix_fd = true,
                        Command::Error(_) => self.cap_unix_fd = false,
                        _ => {
                            return Poll::Ready(Err(Error::Handshake(format!(
                                "Unexpected server UNIX_FD reply: {}",
                                reply
                            ))));
                        }
                    }
                    (Done, Command::Begin)
                }
                Done => return Poll::Ready(Ok(())),
            };
            self.send_buffer = if self.step == Init {
                format!("\0{}", cmd).into()
            } else {
                cmd.into()
            };
            // The dbus daemon on these platforms currently requires sending the zero byte
            // as a separate message with SCM_CREDS
            #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
            if self.step == Init {
                use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};

                // Steal the leading null byte from the buffer.
                let zero = &[self.send_buffer.drain(0..1).next().unwrap()];
                let iov = [nix::sys::uio::IoVec::from_slice(zero)];

                if sendmsg(
                    self.socket.as_raw_fd(),
                    &iov,
                    &[ControlMessage::ScmCreds],
                    MsgFlags::empty(),
                    None,
                ) != Ok(1)
                {
                    return Poll::Ready(Err(Error::Handshake(
                        "Could not send zero byte with credentials".to_string(),
                    )));
                }
            }
            self.step = next_step;
        }
    }

    fn try_finish(self) -> std::result::Result<Authenticated<S>, Self> {
        if let ClientHandshakeStep::Done = self.step {
            Ok(Authenticated {
                conn: Connection::wrap(self.socket),
                server_guid: self.server_guid.unwrap(),
                #[cfg(unix)]
                cap_unix_fd: self.cap_unix_fd,
            })
        } else {
            Err(self)
        }
    }
}

/*
 * Server-side handshake logic
 */

#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
enum ServerHandshakeStep {
    WaitingForNull,
    WaitingForAuth,
    SendingAuthOK,
    SendingAuthError,
    WaitingForBegin,
    #[cfg(unix)]
    SendingBeginMessage,
    Done,
}

/// A representation of an in-progress handshake, server-side
///
/// This would typically be used to implement a D-Bus broker, or in the context of a P2P connection.
///
/// This struct is an async-compatible representation of the initial handshake that must be performed before
/// a D-Bus connection can be used. To use it, you should call the [`advance_handshake`] method whenever the
/// underlying socket becomes ready (tracking the readiness itself is not managed by this abstraction) until
/// it returns `Ok(())`, at which point you can invoke the [`try_finish`] method to get an [`Authenticated`],
/// which can be given to [`Connection::new_authenticated`].
///
/// [`advance_handshake`]: struct.ServerHandshake.html#method.advance_handshake
/// [`try_finish`]: struct.ServerHandshake.html#method.try_finish
/// [`Authenticated`]: struct.Authenticated.html
/// [`Connection::new_authenticated`]: ../struct.Connection.html#method.new_authenticated
#[derive(Debug)]
pub struct ServerHandshake<S> {
    socket: S,
    buffer: Vec<u8>,
    step: ServerHandshakeStep,
    server_guid: Guid,
    #[cfg(unix)]
    cap_unix_fd: bool,
    #[cfg(unix)]
    client_uid: u32,
    #[cfg(windows)]
    client_sid: Option<String>,
    mechanisms: VecDeque<AuthMechanism>,
}

impl<S: Socket> ServerHandshake<S> {
    pub fn new(
        socket: S,
        guid: Guid,
        #[cfg(unix)] client_uid: u32,
        #[cfg(windows)] client_sid: Option<String>,
        mechanisms: Option<VecDeque<AuthMechanism>>,
    ) -> Result<ServerHandshake<S>> {
        let can_external = (|| {
            #[cfg(unix)]
            return true;

            #[cfg(windows)]
            return client_sid.is_some();

            #[cfg(not(any(unix, windows)))]
            return false;
        })();
        let mechanisms = mechanisms.unwrap_or_else(|| {
            let mut mechanisms = VecDeque::new();
            if can_external {
                mechanisms.push_back(AuthMechanism::External);
            }
            mechanisms
        });

        if mechanisms.contains(&AuthMechanism::Cookie) {
            return Err(Error::Unsupported);
        }

        if mechanisms.contains(&AuthMechanism::External) && !can_external {
            return Err(Error::Unsupported);
        }

        Ok(ServerHandshake {
            socket,
            buffer: Vec::new(),
            step: ServerHandshakeStep::WaitingForNull,
            server_guid: guid,
            #[cfg(unix)]
            cap_unix_fd: false,
            #[cfg(unix)]
            client_uid,
            #[cfg(windows)]
            client_sid,
            mechanisms,
        })
    }

    fn flush_buffer(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        while !self.buffer.is_empty() {
            let written = ready!(self.socket.poll_sendmsg(
                cx,
                &self.buffer,
                #[cfg(unix)]
                &[]
            ))?;
            self.buffer.drain(..written);
        }
        Poll::Ready(Ok(()))
    }

    fn read_command(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        while !self.buffer.ends_with(b"\r\n") {
            let mut buf = [0; 40];
            let read = ready!(self.socket.poll_recvmsg(cx, &mut buf))?;
            #[cfg(unix)]
            let read = read.0;
            self.buffer.extend(&buf[..read]);
        }
        Poll::Ready(Ok(()))
    }

    fn auth_ok(&mut self) {
        self.buffer = format!("OK {}\r\n", self.server_guid).into();
        self.step = ServerHandshakeStep::SendingAuthOK;
    }

    fn unsupported_command_error(&mut self) {
        self.buffer = Vec::from(&b"ERROR Unsupported command\r\n"[..]);
        self.step = ServerHandshakeStep::SendingAuthError;
    }

    fn rejected_error(&mut self) {
        let mechanisms = self
            .mechanisms
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        self.buffer = format!("REJECTED {}\r\n", mechanisms).into();
        self.step = ServerHandshakeStep::SendingAuthError;
    }
}

impl<S: Socket> Handshake<S> for ServerHandshake<S> {
    fn advance_handshake(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        loop {
            match self.step {
                ServerHandshakeStep::WaitingForNull => {
                    let mut buffer = [0; 1];
                    let read = ready!(self.socket.poll_recvmsg(cx, &mut buffer))?;
                    #[cfg(unix)]
                    let read = read.0;
                    // recvmsg cannot return anything else than Ok(1) or Err
                    debug_assert!(read == 1);
                    if buffer[0] != 0 {
                        return Poll::Ready(Err(Error::Handshake(
                            "First client byte is not NUL!".to_string(),
                        )));
                    }
                    self.step = ServerHandshakeStep::WaitingForAuth;
                }
                ServerHandshakeStep::WaitingForAuth => {
                    ready!(self.read_command(cx))?;
                    let mut reply = String::new();
                    (&self.buffer[..]).read_line(&mut reply)?;
                    let mut words = reply.split_whitespace();
                    match words.next() {
                        Some("AUTH") => {
                            let mech = words
                                .next()
                                .and_then(|m| AuthMechanism::from_str(m).ok())
                                .filter(|m| self.mechanisms.contains(m));

                            match (mech, words.next(), words.next()) {
                                (Some(AuthMechanism::Anonymous), None, None) => {
                                    self.auth_ok();
                                }
                                (Some(AuthMechanism::External), Some(sasl_id), None) => {
                                    let auth_ok = {
                                        #[cfg(unix)]
                                        {
                                            let uid = id_from_str(sasl_id).map_err(|e| {
                                                Error::Handshake(format!("Invalid UID: {}", e))
                                            })?;
                                            // Safe to unwrap since we checked earlier external & UID
                                            uid == self.client_uid
                                        }
                                        #[cfg(windows)]
                                        {
                                            let sid = hex::decode(sasl_id)?;
                                            let sid = std::str::from_utf8(&sid).map_err(|e| {
                                                Error::Handshake(format!("Invalid SID: {}", e))
                                            })?;
                                            // Safe to unwrap since we checked earlier external & SID
                                            sid == self.client_sid.as_ref().unwrap()
                                        }
                                    };

                                    if auth_ok {
                                        self.auth_ok();
                                    } else {
                                        self.rejected_error();
                                    }
                                }
                                _ => self.rejected_error(),
                            }
                        }
                        Some("ERROR") => self.rejected_error(),
                        Some("BEGIN") => {
                            return Poll::Ready(Err(Error::Handshake(
                                "Received BEGIN while not authenticated".to_string(),
                            )));
                        }
                        _ => self.unsupported_command_error(),
                    }
                }
                ServerHandshakeStep::SendingAuthError => {
                    ready!(self.flush_buffer(cx))?;
                    self.step = ServerHandshakeStep::WaitingForAuth;
                }
                ServerHandshakeStep::SendingAuthOK => {
                    ready!(self.flush_buffer(cx))?;
                    self.step = ServerHandshakeStep::WaitingForBegin;
                }
                ServerHandshakeStep::WaitingForBegin => {
                    ready!(self.read_command(cx))?;
                    let mut reply = String::new();
                    (&self.buffer[..]).read_line(&mut reply)?;
                    let mut words = reply.split_whitespace();
                    match (words.next(), words.next()) {
                        (Some("BEGIN"), None) => {
                            self.step = ServerHandshakeStep::Done;
                        }
                        (Some("CANCEL"), None) | (Some("ERROR"), _) => self.rejected_error(),
                        #[cfg(unix)]
                        (Some("NEGOTIATE_UNIX_FD"), None) => {
                            self.cap_unix_fd = true;
                            self.buffer = Vec::from(&b"AGREE_UNIX_FD\r\n"[..]);
                            self.step = ServerHandshakeStep::SendingBeginMessage;
                        }
                        _ => self.unsupported_command_error(),
                    }
                }
                #[cfg(unix)]
                ServerHandshakeStep::SendingBeginMessage => {
                    ready!(self.flush_buffer(cx))?;
                    self.step = ServerHandshakeStep::WaitingForBegin;
                }
                ServerHandshakeStep::Done => return Poll::Ready(Ok(())),
            }
        }
    }

    fn try_finish(self) -> std::result::Result<Authenticated<S>, Self> {
        if let ServerHandshakeStep::Done = self.step {
            Ok(Authenticated {
                conn: Connection::wrap(self.socket),
                server_guid: self.server_guid,
                #[cfg(unix)]
                cap_unix_fd: self.cap_unix_fd,
            })
        } else {
            Err(self)
        }
    }
}

#[cfg(unix)]
fn id_from_str(s: &str) -> std::result::Result<u32, Box<dyn std::error::Error>> {
    let mut id = String::new();
    for s in s.as_bytes().chunks(2) {
        let c = char::from(u8::from_str_radix(std::str::from_utf8(s)?, 16)?);
        id.push(c);
    }
    Ok(id.parse::<u32>()?)
}

impl fmt::Display for AuthMechanism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mech = match self {
            AuthMechanism::External => "EXTERNAL",
            AuthMechanism::Cookie => "DBUS_COOKIE_SHA1",
            AuthMechanism::Anonymous => "ANONYMOUS",
        };
        write!(f, "{}", mech)
    }
}

impl FromStr for AuthMechanism {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "EXTERNAL" => Ok(AuthMechanism::External),
            "DBUS_COOKIE_SHA1" => Ok(AuthMechanism::Cookie),
            "ANONYMOUS" => Ok(AuthMechanism::Anonymous),
            _ => Err(Error::Handshake(format!("Unknown mechanism: {}", s))),
        }
    }
}

impl From<Command> for Vec<u8> {
    fn from(c: Command) -> Self {
        c.to_string().into()
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cmd = match self {
            Command::Auth(mech, resp) => match (mech, resp) {
                (Some(mech), Some(resp)) => format!("AUTH {} {}", mech, resp),
                (Some(mech), None) => format!("AUTH {}", mech),
                _ => "AUTH".into(),
            },
            Command::Cancel => "CANCEL".into(),
            Command::Begin => "BEGIN".into(),
            Command::Data(data) => {
                format!("DATA {}", hex::encode(data))
            }
            Command::Error(expl) => {
                format!("ERROR {}", expl)
            }
            Command::NegotiateUnixFD => "NEGOTIATE_UNIX_FD".into(),
            Command::Rejected(mechs) => {
                format!(
                    "REJECTED {}",
                    mechs
                        .iter()
                        .map(|m| m.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            Command::Ok(guid) => {
                format!("OK {}", guid)
            }
            Command::AgreeUnixFD => "AGREE_UNIX_FD".into(),
        };
        write!(f, "{}\r\n", cmd)
    }
}

impl From<hex::FromHexError> for Error {
    fn from(e: hex::FromHexError) -> Self {
        Error::Handshake(format!("Invalid hexcode: {}", e))
    }
}

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut words = s.split_ascii_whitespace();
        let cmd = match words.next() {
            Some("AUTH") => {
                let mech = if let Some(m) = words.next() {
                    Some(m.parse()?)
                } else {
                    None
                };
                let resp = words.next().map(|s| s.into());
                Command::Auth(mech, resp)
            }
            Some("CANCEL") => Command::Cancel,
            Some("BEGIN") => Command::Begin,
            Some("DATA") => {
                let data = words
                    .next()
                    .ok_or_else(|| Error::Handshake("Missing DATA data".into()))?;
                Command::Data(hex::decode(data)?)
            }
            Some("ERROR") => Command::Error(s.into()),
            Some("NEGOTIATE_UNIX_FD") => Command::NegotiateUnixFD,
            Some("REJECTED") => {
                let mechs = words.map(|m| m.parse()).collect::<Result<_>>()?;
                Command::Rejected(mechs)
            }
            Some("OK") => {
                let guid = words
                    .next()
                    .ok_or_else(|| Error::Handshake("Missing OK server GUID!".into()))?;
                Command::Ok(guid.parse()?)
            }
            Some("AGREE_UNIX_FD") => Command::AgreeUnixFD,
            _ => return Err(Error::Handshake(format!("Unknown command: {}", s))),
        };
        Ok(cmd)
    }
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use futures_util::future::poll_fn;
    #[cfg(feature = "async-io")]
    use std::os::unix::net::UnixStream;
    use test_log::test;
    #[cfg(not(feature = "async-io"))]
    use tokio::net::UnixStream;

    use super::*;

    use crate::Guid;

    #[test]
    fn handshake() {
        // Tokio needs us to call the sync function from async context. :shrug:
        let (p0, p1) = crate::utils::block_on(async { UnixStream::pair().unwrap() });

        // initialize both handshakes
        #[cfg(feature = "async-io")]
        let (p0, p1) = {
            p0.set_nonblocking(true).unwrap();
            p1.set_nonblocking(true).unwrap();

            (
                async_io::Async::new(p0).unwrap(),
                async_io::Async::new(p1).unwrap(),
            )
        };
        let mut client = ClientHandshake::new(p0, None);
        let mut server =
            ServerHandshake::new(p1, Guid::generate(), Uid::current().into(), None).unwrap();

        // proceed to the handshakes
        let mut client_done = false;
        let mut server_done = false;
        crate::utils::block_on(poll_fn(|cx| {
            match client.advance_handshake(cx) {
                Poll::Ready(Ok(())) => client_done = true,
                Poll::Ready(Err(e)) => panic!("Unexpected error: {:?}", e),
                Poll::Pending => {}
            }

            match server.advance_handshake(cx) {
                Poll::Ready(Ok(())) => server_done = true,
                Poll::Ready(Err(e)) => panic!("Unexpected error: {:?}", e),
                Poll::Pending => {}
            }
            if client_done && server_done {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }));

        let client = client.try_finish().unwrap();
        let server = server.try_finish().unwrap();

        assert_eq!(client.server_guid, server.server_guid);
        assert_eq!(client.cap_unix_fd, server.cap_unix_fd);
    }
}
