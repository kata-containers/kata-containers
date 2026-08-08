// Copyright (c) 2022 Ant Group
// Copyright (c) Kata Containers Community
//
// SPDX-License-Identifier: Apache-2.0
//
// Description:
// Client side of the kata-agent debug console: the vsock / hybrid vsock
// transport, and a request/response protocol layered on top of the PTY-backed
// shell the agent execs there, so a caller can run one command and collect its
// output and exit status instead of only attaching a terminal to the console.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        io::{AsRawFd, FromRawFd, IntoRawFd},
        net::UnixStream,
    },
    process,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Context, Result};
use http_body_util::BodyExt;
use hyper::StatusCode;
use nix::sys::socket::{
    connect as vsock_connect, socket, AddressFamily, SockFlag, SockType, VsockAddr,
};
use slog::{debug, o};

use shim_interface::shim_mgmt::{client::MgmtClient, AGENT_URL};

use crate::utils::TIMEOUT;

const CMD_CONNECT: &str = "CONNECT";
const CMD_OK: &str = "OK";
const SCHEME_VSOCK: &str = "VSOCK";
const SCHEME_HYBRID_VSOCK: &str = "HVSOCK";

const KATA_AGENT_VSOCK_TIMEOUT: u64 = 5;

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "debug_console"))
    };
}

trait SockHandler {
    fn setup_sock(&self) -> Result<UnixStream>;
}

struct VsockConfig {
    sock_cid: u32,
    sock_port: u32,
}

impl VsockConfig {
    fn new(sock_cid: u32, sock_port: u32) -> VsockConfig {
        VsockConfig {
            sock_cid,
            sock_port,
        }
    }
}

impl SockHandler for VsockConfig {
    fn setup_sock(&self) -> Result<UnixStream> {
        let sock_addr = VsockAddr::new(self.sock_cid, self.sock_port);

        // Create socket fd
        let vsock_fd = socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .context("create vsock socket")?;

        // Wrap the socket fd in UnixStream, so that it is closed
        // when anything fails.
        let stream = unsafe { UnixStream::from_raw_fd(vsock_fd.into_raw_fd()) };
        // Connect the socket to vsock server.
        vsock_connect(stream.as_raw_fd(), &sock_addr)
            .with_context(|| format!("failed to connect to server {:?}", &sock_addr))?;

        Ok(stream)
    }
}

struct HvsockConfig {
    sock_addr: String,
    sock_port: u32,
}

impl HvsockConfig {
    fn new(sock_addr: String, sock_port: u32) -> Self {
        HvsockConfig {
            sock_addr,
            sock_port,
        }
    }
}

impl SockHandler for HvsockConfig {
    fn setup_sock(&self) -> Result<UnixStream> {
        let mut stream = match UnixStream::connect(self.sock_addr.clone()) {
            Ok(s) => s,
            Err(e) => return Err(anyhow!(e).context("failed to create UNIX Stream socket")),
        };

        // Ensure the Unix Stream directly connects to the real VSOCK server which
        // the Kata agent is listening to in the VM.
        {
            let test_msg = format!("{} {}\n", CMD_CONNECT, self.sock_port);

            stream.set_read_timeout(Some(Duration::new(KATA_AGENT_VSOCK_TIMEOUT, 0)))?;
            stream.set_write_timeout(Some(Duration::new(KATA_AGENT_VSOCK_TIMEOUT, 0)))?;

            stream.write_all(test_msg.as_bytes())?;
            // Now, see if we get the expected response
            let stream_reader = stream.try_clone()?;
            let mut reader = BufReader::new(&stream_reader);
            let mut msg = String::new();

            reader.read_line(&mut msg)?;
            if msg.is_empty() {
                return Err(anyhow!(
                    "stream reader get message is empty with port: {:?}",
                    self.sock_port
                ));
            }

            // Expected response message returned was successful.
            if msg.starts_with(CMD_OK) {
                let response = msg
                    .strip_prefix(CMD_OK)
                    .ok_or(format!("invalid response: {msg:?}"))
                    .map_err(|e| anyhow!(e))?
                    .trim();
                debug!(sl!(), "Hybrid Vsock host-side port: {:?}", response);
                // Unset the timeout in order to turn the sokect to bloking mode.
                stream.set_read_timeout(None)?;
                stream.set_write_timeout(None)?;
            } else {
                return Err(anyhow!(
                    "failed to setup Hybrid Vsock connection: {:?}",
                    msg
                ));
            }
        }

        Ok(stream)
    }
}

fn setup_client(server_url: String, dbg_console_port: u32) -> Result<UnixStream> {
    // server address format: scheme://[cid|/x/domain.sock]:port
    let url_fields: Vec<&str> = server_url.split("://").collect();
    if url_fields.len() != 2 {
        return Err(anyhow!("invalid URI"));
    }

    let scheme = url_fields[0].to_uppercase();
    let sock_addr: Vec<&str> = url_fields[1].split(':').collect();
    if sock_addr.len() != 2 {
        return Err(anyhow!("invalid VSOCK server address URI"));
    }

    match scheme.as_str() {
        // Hybrid Vsock: hvsock://<path>:<port>.
        // Example: "hvsock:///x/y/z/kata.hvsock:port"
        // Firecracker/Dragonball/CLH implements the hybrid vsock device model.
        SCHEME_HYBRID_VSOCK => {
            let hvsock_path = sock_addr[0].to_string();
            if hvsock_path.is_empty() {
                return Err(anyhow!("hvsock path cannot be empty"));
            }

            let hvsock = HvsockConfig::new(hvsock_path, dbg_console_port);
            hvsock.setup_sock().context("set up hvsock")
        }
        // Vsock: vsock://<cid>:<port>
        // Example: "vsock://31513974:1024"
        // Qemu using the Vsock device model.
        SCHEME_VSOCK => {
            let sock_cid: u32 = match sock_addr[0] {
                "-1" | "" => libc::VMADDR_CID_ANY,
                _ => match sock_addr[0].parse::<u32>() {
                    Ok(cid) => cid,
                    Err(e) => return Err(anyhow!("vsock addr CID is INVALID: {:?}", e)),
                },
            };

            let vsock = VsockConfig::new(sock_cid, dbg_console_port);
            vsock.setup_sock().context("set up vsock")
        }
        // Others will be INVALID URI.
        _ => Err(anyhow!("invalid URI scheme: {:?}", scheme)),
    }
}

async fn get_agent_socket(sandbox_id: &str) -> Result<String> {
    let shim_client = MgmtClient::new(sandbox_id, Some(TIMEOUT))?;

    // get agent sock from body when status code is OK.
    let response = shim_client.get(AGENT_URL).await?;
    let status = response.status();
    if status != StatusCode::OK {
        return Err(anyhow!("shim client get connection failed: {:?} ", status));
    }

    let body = response.into_body().collect().await?.to_bytes();
    let agent_sock = String::from_utf8(body.to_vec())?;

    Ok(agent_sock)
}

fn get_server_socket(sandbox_id: &str) -> Result<String> {
    let server_url = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(get_agent_socket(sandbox_id))
        .context("get connection vsock")?;

    Ok(server_url)
}

/// Connect to the debug console the agent of `sandbox_id` serves on `vport`.
pub fn connect(sandbox_id: &str, vport: u32) -> Result<UnixStream> {
    let server_url = get_server_socket(sandbox_id).context("get debug console socket URL")?;
    if server_url.is_empty() {
        return Err(anyhow!("server url is empty."));
    }

    setup_client(server_url, vport)
}

/// End-of-transmission. The guest terminal stays in canonical mode, so this at
/// the start of a line is what closes the stdin of a running command.
pub const EOT: u8 = 0x04;

/// Appended to the marker to frame the output of a command. Neither is ever
/// spelled out in what we write to the shell - both are only ever produced by
/// expanding the variable holding the marker - so a terminal that echoes our
/// input back can never be mistaken for the real thing.
const BEGIN: &str = "-BEGIN";
const END: &str = "-END:";

/// How long to wait for a command to start before giving up. Only covers the
/// round trip to the shell, never the command itself, so it can be generous:
/// the devkit console shell assembles an overlay before it is reachable.
const START_TIMEOUT: Duration = Duration::from_secs(300);

const READ_CHUNK: usize = 8 * 1024;

/// Quote `arg` so the console shell passes it on as a single word.
pub fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Undo the terminal's NL -> CR NL output translation.
fn strip_cr(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        if data[i] == b'\r' && data.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(data[i]);
        i += 1;
    }

    out
}

/// A non-interactive session on the console shell.
///
/// The console offers a shell on a pseudo-terminal and nothing else: no exit
/// status, no framing, and no way to tell the shell's own chatter from what a
/// command printed. So each command is wrapped in a pair of markers that only
/// the guest can produce, and the status is printed after the second one.
pub struct Session {
    stream: UnixStream,
    marker: String,
    buf: Vec<u8>,
    /// The guest is still translating NL to CR NL on output, so undo it.
    crlf: bool,
    prepared: bool,
}

impl Session {
    pub fn new(stream: UnixStream) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        Session {
            stream,
            marker: format!("KATACTL{:x}{:x}", process::id(), nanos),
            buf: Vec::new(),
            crlf: false,
            prepared: false,
        }
    }

    /// A second handle on the connection, so a command's stdin can be fed
    /// while its output is being read.
    pub fn writer(&self) -> Result<UnixStream> {
        self.stream
            .try_clone()
            .context("clone debug console socket")
    }

    /// Run `cmd` to completion, relaying its output to `sink`, and report the
    /// exit status. The console is one stream, so this is stdout and stderr
    /// interleaved.
    pub fn run(&mut self, cmd: &str, sink: &mut dyn Write) -> Result<i32> {
        self.begin(cmd)?;
        self.finish(sink)
    }

    /// Run `cmd` for its output, trimmed, as text.
    pub fn capture(&mut self, cmd: &str) -> Result<(i32, String)> {
        let mut out = Vec::new();
        let status = self.run(cmd, &mut out)?;

        Ok((status, String::from_utf8_lossy(&out).trim().to_string()))
    }

    /// Run `cmd` purely for its exit status, e.g. a `test` probe.
    pub fn probe(&mut self, cmd: &str) -> Result<bool> {
        Ok(self.run(cmd, &mut io::sink())? == 0)
    }

    /// Send `cmd` and return once it is running, so the caller may then write
    /// to its stdin over the same connection.
    pub fn begin(&mut self, cmd: &str) -> Result<()> {
        self.prepare()?;

        let marker = self.marker.clone();
        self.send(&format!(
            "__kc_m={marker}; printf '%s\\n' \"${{__kc_m}}{BEGIN}\"; {cmd}; __kc_rc=$?; \
             printf '%s%s\\n' \"${{__kc_m}}{END}\" \"${{__kc_rc}}\""
        ))?;

        // Anything ahead of the marker is the shell's own noise: a prompt, a
        // login banner, or our command echoed back.
        self.stream.set_read_timeout(Some(START_TIMEOUT))?;
        let begin = format!("{marker}{BEGIN}");
        self.scan(begin.as_bytes(), &mut io::sink())?;
        self.crlf = self.consume_eol()?;
        self.stream.set_read_timeout(None)?;

        Ok(())
    }

    /// Relay the output of the command started by [`Session::begin`] to
    /// `sink`, and report its exit status.
    pub fn finish(&mut self, sink: &mut dyn Write) -> Result<i32> {
        let end = format!("{}{END}", self.marker);
        self.scan(end.as_bytes(), sink)?;

        self.read_status()
    }

    /// Leave the shell, so the guest side tears the session down promptly.
    pub fn close(mut self) {
        let _ = self.send("exit");
    }

    /// Put the guest terminal in a shape we can parse: no echo of what we
    /// write, which would otherwise be indistinguishable from command output,
    /// and no NL -> CR NL translation, so output arrives byte for byte. Best
    /// effort - a guest with no stty still works, [`Session::begin`] infers
    /// the translation from how the marker line ends.
    fn prepare(&mut self) -> Result<()> {
        if self.prepared {
            return Ok(());
        }

        self.send("stty -opost -echo 2>/dev/null")?;
        self.prepared = true;

        Ok(())
    }

    fn send(&mut self, line: &str) -> Result<()> {
        self.stream
            .write_all(format!("{line}\n").as_bytes())
            .context("write to debug console")?;

        self.stream.flush().context("flush debug console")
    }

    fn fill(&mut self) -> Result<()> {
        let mut chunk = [0u8; READ_CHUNK];

        let n = self.stream.read(&mut chunk).map_err(|e| {
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) {
                anyhow!(
                    "timed out after {}s waiting for the guest debug console shell",
                    START_TIMEOUT.as_secs()
                )
            } else {
                anyhow!(e).context("read from debug console")
            }
        })?;

        if n == 0 {
            bail!("debug console closed the connection");
        }

        self.buf.extend_from_slice(&chunk[..n]);

        Ok(())
    }

    fn take(&mut self, n: usize) {
        self.buf.drain(..n);
    }

    /// Consume input up to and including `needle`, relaying everything ahead
    /// of it to `sink`.
    fn scan(&mut self, needle: &[u8], sink: &mut dyn Write) -> Result<()> {
        loop {
            if let Some(at) = find(&self.buf, needle) {
                self.emit(at, sink)?;
                self.take(needle.len());
                return Ok(());
            }

            // Hand over everything that cannot still turn out to be the head
            // of the marker, or the CR of a CR LF whose LF has yet to arrive.
            let hold = needle.len() - 1;
            if self.buf.len() > hold {
                let mut upto = self.buf.len() - hold;
                if self.crlf && self.buf[upto - 1] == b'\r' {
                    upto -= 1;
                }
                self.emit(upto, sink)?;
            }

            self.fill()?;
        }
    }

    fn emit(&mut self, n: usize, sink: &mut dyn Write) -> Result<()> {
        if n == 0 {
            return Ok(());
        }

        let chunk: Vec<u8> = self.buf.drain(..n).collect();
        let chunk = if self.crlf { strip_cr(&chunk) } else { chunk };

        sink.write_all(&chunk).context("relay debug console output")
    }

    /// Consume the end of the marker line, reporting whether it was CR LF.
    fn consume_eol(&mut self) -> Result<bool> {
        let crlf = self.peek()? == b'\r';
        if crlf {
            self.take(1);
        }

        if self.peek()? != b'\n' {
            bail!("unexpected data after the debug console marker");
        }
        self.take(1);

        Ok(crlf)
    }

    fn peek(&mut self) -> Result<u8> {
        while self.buf.is_empty() {
            self.fill()?;
        }

        Ok(self.buf[0])
    }

    /// Read the exit status the wrapper prints right after the end marker.
    fn read_status(&mut self) -> Result<i32> {
        let mut line = Vec::new();

        loop {
            if let Some(at) = self.buf.iter().position(|b| *b == b'\n') {
                line.extend_from_slice(&self.buf[..at]);
                self.take(at + 1);
                break;
            }

            line.extend_from_slice(&self.buf);
            self.buf.clear();
            self.fill()?;
        }

        let status = String::from_utf8_lossy(&line);
        let status = status.trim();

        status
            .parse::<i32>()
            .with_context(|| format!("parse exit status {status:?} from the debug console"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use micro_http::HttpServer;
    use std::thread;

    #[test]
    fn test_setup_hvsock_failed() {
        let kata_hybrid_addr = "/tmp/kata_hybrid_vsock02.hvsock";
        let hybrid_sock_addr = "hvsock:///tmp/kata_hybrid_vsock02.hvsock:1024";
        std::fs::remove_file(kata_hybrid_addr).unwrap_or_default();
        let dbg_console_port: u32 = 1026;
        let mut server = HttpServer::new(kata_hybrid_addr).unwrap();
        server.start_server().unwrap();

        let stream = setup_client(hybrid_sock_addr.to_string(), dbg_console_port);
        assert!(stream.is_err());
        std::fs::remove_file(kata_hybrid_addr).unwrap_or_default();
    }

    #[test]
    fn test_setup_vsock_client_failed() {
        let hybrid_sock_addr = "hvsock://8:1024";
        let dbg_console_port: u32 = 1026;
        let stream = setup_client(hybrid_sock_addr.to_string(), dbg_console_port);
        assert!(stream.is_err());
    }

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("ls"), "'ls'");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote("$(id)"), "'$(id)'");
    }

    #[test]
    fn test_strip_cr() {
        assert_eq!(strip_cr(b"a\r\nb\r\n"), b"a\nb\n");
        // A CR that is not part of a line ending belongs to the payload.
        assert_eq!(strip_cr(b"a\rb"), b"a\rb");
        assert_eq!(strip_cr(b"\r"), b"\r");
    }

    #[test]
    fn test_find() {
        assert_eq!(find(b"hello", b"ll"), Some(2));
        assert_eq!(find(b"hello", b"xx"), None);
        assert_eq!(find(b"hi", b"longer"), None);
        assert_eq!(find(b"hello", b""), None);
    }

    /// Stand in for the guest: read the wrapper command line, echo the markers
    /// back around `output`, then report `status`.
    fn fake_shell(mut sock: UnixStream, output: &'static [u8], status: i32, crlf: bool) {
        thread::spawn(move || {
            let peer = sock.try_clone().unwrap();
            let mut reader = BufReader::new(peer);
            let eol: &[u8] = if crlf { b"\r\n" } else { b"\n" };

            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }

                // Only the wrapper carries a marker; skip the stty prelude.
                let marker = match line.split_once("__kc_m=") {
                    Some((_, rest)) => rest.split(';').next().unwrap().to_string(),
                    None => continue,
                };

                sock.write_all(b"noise from the shell prompt").unwrap();
                sock.write_all(format!("{marker}{BEGIN}").as_bytes())
                    .unwrap();
                sock.write_all(eol).unwrap();
                sock.write_all(output).unwrap();
                sock.write_all(format!("{marker}{END}{status}").as_bytes())
                    .unwrap();
                sock.write_all(eol).unwrap();
                sock.flush().unwrap();
            }
        });
    }

    #[test]
    fn test_session_relays_output() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        fake_shell(theirs, b"hello\n", 0, false);

        let mut session = Session::new(ours);
        let mut out = Vec::new();
        let status = session.run("echo hello", &mut out).unwrap();

        assert_eq!(status, 0);
        assert_eq!(out, b"hello\n");
    }

    #[test]
    fn test_session_reports_exit_status() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        fake_shell(theirs, b"", 42, false);

        let mut session = Session::new(ours);

        assert_eq!(session.run("false", &mut io::sink()).unwrap(), 42);
    }

    /// A guest with no stty leaves the NL -> CR NL translation on.
    #[test]
    fn test_session_undoes_crlf() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        fake_shell(theirs, b"one\r\ntwo\r\n", 0, true);

        let mut session = Session::new(ours);
        let mut out = Vec::new();
        session.run("cat", &mut out).unwrap();

        assert_eq!(out, b"one\ntwo\n");
    }

    /// Output that does not end in a newline must not gain or lose bytes, even
    /// though the end marker is then glued to the last line.
    #[test]
    fn test_session_output_is_verbatim() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        fake_shell(theirs, b"no trailing newline", 0, false);

        let mut session = Session::new(ours);
        let mut out = Vec::new();
        session.run("printf x", &mut out).unwrap();

        assert_eq!(out, b"no trailing newline");
    }

    /// Successive commands share one connection, and one shell.
    #[test]
    fn test_session_runs_several_commands() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        fake_shell(theirs, b"out\n", 0, false);

        let mut session = Session::new(ours);
        for _ in 0..3 {
            let mut out = Vec::new();
            assert_eq!(session.run("echo out", &mut out).unwrap(), 0);
            assert_eq!(out, b"out\n");
        }
    }
}
