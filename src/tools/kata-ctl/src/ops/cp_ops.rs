// Copyright (c) Kata Containers Community
//
// SPDX-License-Identifier: Apache-2.0
//
// Description:
// Copying files and directories between the host and a guest VM, tunnelled
// through the agent debug console.

use std::{
    fs,
    io::{self, BufWriter, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process,
    sync::mpsc::{sync_channel, Receiver, SyncSender},
    thread,
};

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::args::CpArguments;
use crate::debug_console::{self, shell_quote, Session, EOT};

/// Where the devkit guest extension is mounted, and where the console shell can
/// still see the guest's real root once devkit-init has chrooted it into the
/// devkit overlay.
const DEVKIT_ROOT: &str = "/run/kata-extensions/devkit";
const REAL_ROOT: &str = "/real_root";

/// Where the guest parks the stderr of a command whose stdout we are reading.
/// The console carries one stream, so anything written there would otherwise
/// land in the middle of the payload. `$$` is the console shell's own pid, so
/// two sessions never pick the same file.
const GUEST_STDERR: &str = "/tmp/.kata-ctl-cp.$$.err";

/// base64 wraps at 76 characters, which keeps every line we push at the guest
/// well inside what its terminal accepts unbroken. 57 bytes is that much input,
/// and a multiple of 3, so each line encodes on its own with no padding until
/// the last one.
const B64_LINE_BYTES: usize = 57;

/// How much of a failed command's output to keep for the error message.
const DIAGNOSTIC_BYTES: usize = 4 * 1024;

/// Archive chunks in flight between the thread reading the console and the one
/// extracting, so neither runs ahead of the other by more than a little.
const PIPE_DEPTH: usize = 16;

// kata-ctl handle cp command starts here.
pub fn handle_cp(cp_args: CpArguments) -> Result<()> {
    let src = Endpoint::parse(&cp_args.src)?;
    let dst = Endpoint::parse(&cp_args.dst)?;

    let (sandbox_id, copy) = match (src, dst) {
        (Endpoint::Guest { sandbox_id, path }, Endpoint::Host(local)) => {
            (sandbox_id, Copy::Out { path, local })
        }
        (Endpoint::Host(local), Endpoint::Guest { sandbox_id, path }) => {
            (sandbox_id, Copy::In { local, path })
        }
        (Endpoint::Host(_), Endpoint::Host(_)) => {
            bail!("neither side names a sandbox: write one of them as <sandbox-id>:<path>")
        }
        (Endpoint::Guest { .. }, Endpoint::Guest { .. }) => {
            bail!("copying straight from one sandbox to another is not supported")
        }
    };

    let sock_stream = debug_console::connect(&sandbox_id, cp_args.vport)?;
    let mut session = Session::new(sock_stream);

    require_devkit(&mut session)?;

    let res = match copy {
        Copy::In { local, path } => copy_in(&mut session, &local, &path),
        Copy::Out { path, local } => copy_out(&mut session, &path, &local),
    };

    session.close();

    res
}

/// One side of a copy: a path on this host, or a path inside a sandbox.
#[derive(Debug, PartialEq)]
enum Endpoint {
    Host(PathBuf),
    Guest { sandbox_id: String, path: String },
}

enum Copy {
    In { local: PathBuf, path: String },
    Out { path: String, local: PathBuf },
}

impl Endpoint {
    /// `<sandbox-id>:<absolute-guest-path>` names a guest, anything else is a
    /// host path. As in docker, a colon only separates the two when what comes
    /// before it could be an id, so host paths containing one still work.
    fn parse(spec: &str) -> Result<Self> {
        let (sandbox_id, path) = match spec.split_once(':') {
            Some((id, path)) if !id.is_empty() && !id.contains('/') => (id, path),
            _ => return Ok(Endpoint::Host(PathBuf::from(spec))),
        };

        // The console shell's working directory is an implementation detail of
        // the devkit overlay, so a relative guest path would land somewhere the
        // caller cannot predict.
        if !path.starts_with('/') {
            bail!("guest path {path:?} in {spec:?} must be absolute");
        }

        Ok(Endpoint::Guest {
            sandbox_id: sandbox_id.to_string(),
            path: path.to_string(),
        })
    }
}

/// `cp` leans on what the devkit extension brings into the guest - a shell, tar
/// and base64 - none of which a minimal guest rootfs is required to have, and
/// several of which ship none of. Refuse up front rather than fail halfway
/// through a copy.
///
/// A precondition, then, and not a permission check. It cannot be one: it runs
/// on this side, so a patched kata-ctl drops it, and nothing is kept out by it
/// anyway. What carries a copy is the agent debug console, and that console is
/// a root shell in the guest - whoever can open one can already move bytes in
/// both directions in a handful of lines, with no kata-ctl involved. The
/// boundary is that console existing at all: the agent opens it only for a
/// guest booted with `agent.debug_console`, which the runtime passes only for
/// `debug_console_enabled`, and which a confidential guest measures. Turning it
/// on is deliberate, and it is visible to attestation.
///
/// The console shell chroots into the devkit overlay, where the extension's own
/// mount point is out of view but the guest's real root is reachable through
/// the /real_root symlink devkit-init leaves behind. Look in both places, so
/// this holds whether or not that chroot happened.
fn require_devkit(session: &mut Session) -> Result<()> {
    let probe = format!("test -d {DEVKIT_ROOT} || test -d {REAL_ROOT}{DEVKIT_ROOT}");

    if session.probe(&probe)? {
        return Ok(());
    }

    bail!(
        "this sandbox has no devkit guest extension, and `kata-ctl cp` needs the \
         tooling it brings into the guest; run the workload under a \
         kata-<shim>-devkit RuntimeClass, which kata-deploy creates when it is \
         installed with both `debug` and `devkit` enabled"
    )
}

/// Host to guest. The archive is built here and unpacked there.
fn copy_in(session: &mut Session, local: &Path, remote: &str) -> Result<()> {
    if !local.exists() {
        bail!("{} does not exist", local.display());
    }

    // As in docker cp, an existing directory is copied *into*; anything else
    // names the copy.
    let (dir, name) = if session.probe(&format!("test -d {}", shell_quote(remote)))? {
        (remote.to_string(), host_basename(local)?)
    } else {
        (guest_dirname(remote)?, guest_basename(remote)?)
    };

    if !session.probe(&format!("test -d {}", shell_quote(&dir)))? {
        bail!("guest directory {dir:?} does not exist");
    }

    // The host's uids mean nothing in the guest, where the console runs as
    // root, so let the archive's modes through but not its ownership. tar's own
    // stderr can go straight into the framed output: nothing else comes back
    // this way, so there is no payload for it to corrupt.
    session.begin(&format!(
        "base64 -d | tar -C {} --no-same-owner -xf -",
        shell_quote(&dir)
    ))?;

    let sink = session.writer()?;
    let feeder = {
        let src = local.to_path_buf();
        let entry = name.clone();

        thread::spawn(move || {
            let res = send_archive(&sink, &src, &entry);
            if res.is_err() {
                // Let the guest see the end of its input, so it stops waiting
                // for an archive that is never going to arrive.
                let _ = sink.shutdown(Shutdown::Write);
            }
            res
        })
    };

    let mut diagnostic = Diagnostic::default();
    let status = session.finish(&mut diagnostic)?;

    if status != 0 {
        // The guest has stopped reading, so the feeder may still be blocked
        // partway through the archive. Close the writing side to let it go.
        if let Ok(sock) = session.writer() {
            let _ = sock.shutdown(Shutdown::Write);
        }
        let _ = feeder.join();

        bail!(
            "unpacking into {dir:?} in the guest failed{}",
            diagnostic.detail()
        );
    }

    feeder
        .join()
        .map_err(|_| anyhow!("the thread writing the archive panicked"))?
}

/// Guest to host. The archive is built there and unpacked here.
fn copy_out(session: &mut Session, remote: &str, local: &Path) -> Result<()> {
    let dir = guest_dirname(remote)?;
    let name = guest_basename(remote)?;

    if !session.probe(&format!("test -e {}", shell_quote(remote)))? {
        bail!("{remote:?} does not exist in the guest");
    }

    // As in docker cp, an existing directory is copied *into*; anything else
    // names the copy.
    let (root, rename) = if local.is_dir() {
        (local.to_path_buf(), None)
    } else {
        (host_parent(local), Some(local.to_path_buf()))
    };

    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }

    // base64 is last in the pipeline, so without pipefail a tar that died would
    // still be reported as a clean exit.
    session.begin(&format!(
        "set -o pipefail 2>/dev/null; tar -C {} -cf - {} 2>{GUEST_STDERR} | base64",
        shell_quote(&dir),
        shell_quote(&name),
    ))?;

    let (tx, rx) = pipe();
    let unpacker = {
        let dest = rename.clone();
        let root = root.clone();

        thread::spawn(move || unpack(rx, root, dest))
    };

    let mut decoder = Base64Decoder::new(tx);
    let status = session.finish(&mut decoder)?;
    // Dropping the decoder closes the pipe, which is what ends the extraction.
    decoder.finish()?;

    let unpacked = unpacker
        .join()
        .map_err(|_| anyhow!("the thread extracting the archive panicked"))?;

    let (_, stderr) = session.capture(&format!(
        "cat {GUEST_STDERR} 2>/dev/null; rm -f {GUEST_STDERR}"
    ))?;

    if status != 0 {
        bail!(
            "archiving {remote:?} in the guest failed{}",
            detail(&stderr)
        );
    }

    unpacked
}

/// Tar `src` under the name `entry`, base64 it, and push it at the command
/// waiting on the other end of `sink`, then close that command's stdin.
fn send_archive(sink: &UnixStream, src: &Path, entry: &str) -> Result<()> {
    let mut out = Base64Lines::new(BufWriter::new(sink));

    {
        let mut archive = tar::Builder::new(&mut out);
        let meta = fs::symlink_metadata(src).with_context(|| format!("stat {}", src.display()))?;

        if meta.is_dir() {
            archive
                .append_dir_all(entry, src)
                .with_context(|| format!("archive {}", src.display()))?;
        } else {
            archive
                .append_path_with_name(src, entry)
                .with_context(|| format!("archive {}", src.display()))?;
        }

        archive.finish().context("finish the archive")?;
    }

    let mut sink = out.finish()?;
    sink.write_all(&[b'\n', EOT])
        .context("close the guest command's stdin")?;
    sink.flush().context("flush to the guest")?;

    Ok(())
}

/// Extract the archive `reader` carries into `root`. With `rename`, the
/// archive's single top-level entry is put there under that name instead.
fn unpack(reader: PipeReader, root: PathBuf, rename: Option<PathBuf>) -> Result<()> {
    // Unpack whole-archive rather than entry by entry: the guest chose these
    // names, and that is what rejects the ones that would write outside the
    // directory we picked.
    let mut archive = tar::Archive::new(reader);
    archive.set_overwrite(true);
    archive.set_preserve_permissions(true);

    let Some(dest) = rename else {
        return archive
            .unpack(&root)
            .with_context(|| format!("extract into {}", root.display()));
    };

    // A rename only happens when the destination does not exist yet, so stage
    // the archive next to it and move the one thing it holds into place. That
    // keeps the extraction itself inside a directory of our own making.
    let staging = Staging::new(&root)?;

    archive
        .unpack(staging.path())
        .with_context(|| format!("extract into {}", staging.path().display()))?;

    let mut top = fs::read_dir(staging.path())
        .with_context(|| format!("read {}", staging.path().display()))?;

    let entry = top
        .next()
        .transpose()?
        .ok_or_else(|| anyhow!("the guest sent an empty archive"))?;

    if top.next().transpose()?.is_some() {
        bail!("the guest sent an archive holding more than one top-level entry");
    }

    fs::rename(entry.path(), &dest)
        .with_context(|| format!("move the copy into place at {}", dest.display()))
}

/// A scratch directory that goes away with the value.
struct Staging(PathBuf);

impl Staging {
    fn new(parent: &Path) -> Result<Self> {
        let path = parent.join(format!(".kata-ctl-cp.{}", process::id()));

        fs::create_dir(&path).with_context(|| format!("create {}", path.display()))?;

        Ok(Staging(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Wraps a writer so what is written reaches it as base64, in lines short
/// enough for the guest's terminal to take unbroken.
struct Base64Lines<W: Write> {
    inner: W,
    pending: Vec<u8>,
}

impl<W: Write> Base64Lines<W> {
    fn new(inner: W) -> Self {
        Base64Lines {
            inner,
            pending: Vec::with_capacity(B64_LINE_BYTES * 2),
        }
    }

    /// Emit the remainder, the only line that carries padding, and hand the
    /// underlying writer back.
    fn finish(mut self) -> Result<W> {
        if !self.pending.is_empty() {
            let line = BASE64.encode(&self.pending);
            self.pending.clear();
            self.inner.write_all(line.as_bytes())?;
            self.inner.write_all(b"\n")?;
        }

        self.inner.flush()?;

        Ok(self.inner)
    }
}

impl<W: Write> Write for Base64Lines<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);

        let full = self.pending.len() / B64_LINE_BYTES * B64_LINE_BYTES;
        if full > 0 {
            let mut lines = String::with_capacity(full / B64_LINE_BYTES * 77);
            for chunk in self.pending[..full].chunks(B64_LINE_BYTES) {
                lines.push_str(&BASE64.encode(chunk));
                lines.push('\n');
            }

            self.pending.drain(..full);
            self.inner.write_all(lines.as_bytes())?;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Wraps a writer so base64 written to it reaches the writer decoded.
struct Base64Decoder<W: Write> {
    inner: W,
    pending: Vec<u8>,
}

impl<W: Write> Base64Decoder<W> {
    fn new(inner: W) -> Self {
        Base64Decoder {
            inner,
            pending: Vec::new(),
        }
    }

    fn finish(mut self) -> Result<()> {
        let rest = std::mem::take(&mut self.pending);
        self.decode_line(&rest)?;
        self.inner.flush()?;

        Ok(())
    }

    fn decode_line(&mut self, line: &[u8]) -> io::Result<()> {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            return Ok(());
        }

        let data = BASE64.decode(line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("the guest sent something that is not base64: {e}"),
            )
        })?;

        self.inner.write_all(&data)
    }
}

impl<W: Write> Write for Base64Decoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);

        while let Some(at) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=at).collect();
            self.decode_line(&line[..at])?;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// A `Write` end and a `Read` end joined by a bounded queue, so bytes being
/// pushed out of the console can feed a reader that wants to pull them.
fn pipe() -> (PipeWriter, PipeReader) {
    let (tx, rx) = sync_channel(PIPE_DEPTH);

    (
        PipeWriter(tx),
        PipeReader {
            rx,
            chunk: Vec::new(),
            at: 0,
        },
    )
}

struct PipeWriter(SyncSender<Vec<u8>>);

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // A reader that gave up took its error with it, and that error says far
        // more than a broken pipe would. Keep draining the console so the
        // command's own status still comes back, and let the caller report it.
        let _ = self.0.send(buf.to_vec());

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct PipeReader {
    rx: Receiver<Vec<u8>>,
    chunk: Vec<u8>,
    at: usize,
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.at == self.chunk.len() {
            match self.rx.recv() {
                Ok(chunk) => {
                    self.chunk = chunk;
                    self.at = 0;
                }
                Err(_) => return Ok(0),
            }
        }

        let n = std::cmp::min(buf.len(), self.chunk.len() - self.at);
        buf[..n].copy_from_slice(&self.chunk[self.at..self.at + n]);
        self.at += n;

        Ok(n)
    }
}

/// Keeps only the tail of what a failing command printed.
#[derive(Default)]
struct Diagnostic(Vec<u8>);

impl Diagnostic {
    fn detail(&self) -> String {
        detail(&String::from_utf8_lossy(&self.0))
    }
}

impl Write for Diagnostic {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(buf);
        if self.0.len() > DIAGNOSTIC_BYTES {
            self.0.drain(..self.0.len() - DIAGNOSTIC_BYTES);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn detail(msg: &str) -> String {
    match msg.trim() {
        "" => String::new(),
        msg => format!(": {msg}"),
    }
}

fn guest_basename(path: &str) -> Result<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{path:?} names nothing inside the guest"))
}

fn guest_dirname(path: &str) -> Result<String> {
    Path::new(path)
        .parent()
        .and_then(|dir| dir.to_str())
        .filter(|dir| !dir.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{path:?} has no parent directory inside the guest"))
}

fn host_basename(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{} names nothing", path.display()))
}

fn host_parent(path: &Path) -> PathBuf {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};

    use nix::pty::openpty;
    use tempfile::tempdir;

    /// A stand-in for the guest side of the console: a real shell on a real
    /// pseudo-terminal, reachable over a socket, which is what the agent
    /// offers. Yields nothing where the machine cannot provide one, so the
    /// suite still runs somewhere without a pty or without the guest tools.
    fn console() -> Option<(UnixStream, Child)> {
        for tool in ["bash", "tar", "base64", "stty"] {
            let found = Command::new("sh")
                .arg("-c")
                .arg(format!("command -v {tool}"))
                .stdout(Stdio::null())
                .status()
                .ok()?
                .success();

            if !found {
                return None;
            }
        }

        let pty = openpty(None, None).ok()?;
        let slave = pty.slave;

        let child = unsafe {
            Command::new("bash")
                .args(["--noprofile", "--norc"])
                .stdin(Stdio::from(slave.try_clone().ok()?))
                .stdout(Stdio::from(slave.try_clone().ok()?))
                .stderr(Stdio::from(slave.try_clone().ok()?))
                .pre_exec(|| {
                    // Same shape as the agent's console: session leader, with
                    // the pty as its controlling terminal.
                    nix::unistd::setsid()?;
                    libc::ioctl(0, libc::TIOCSCTTY, 0);
                    Ok(())
                })
                .spawn()
                .ok()?
        };
        drop(slave);

        let (ours, theirs) = UnixStream::pair().ok()?;
        let master = fs::File::from(pty.master);
        let mut to_shell = master.try_clone().ok()?;
        let mut from_shell = master;
        let mut sock_out = theirs.try_clone().ok()?;
        let mut sock_in = theirs;

        thread::spawn(move || io::copy(&mut from_shell, &mut sock_out));
        thread::spawn(move || io::copy(&mut sock_in, &mut to_shell));

        Some((ours, child))
    }

    fn read(path: &Path) -> Vec<u8> {
        fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    /// Drive both directions against a real shell, so the whole tunnel is
    /// exercised: terminal setup, framing, base64 lines, the end-of-input the
    /// upload closes with, and tar on either side.
    #[test]
    fn test_copy_round_trip_over_a_real_shell() {
        let Some((sock, mut shell)) = console() else {
            eprintln!("skipping: no pty-backed shell with tar and base64 here");
            return;
        };

        let mut session = Session::new(sock);

        let host = tempdir().unwrap();
        let guest = tempdir().unwrap();

        // A file big enough to span many base64 lines, and to make the writer
        // and the reader overlap rather than fit in a socket buffer.
        let payload: Vec<u8> = (0..300_000).map(|i| (i % 251) as u8).collect();
        let src = host.path().join("payload.bin");
        fs::write(&src, &payload).unwrap();

        // Host to guest, onto a name of its own.
        let remote = guest.path().join("copied.bin");
        copy_in(&mut session, &src, remote.to_str().unwrap()).unwrap();
        assert_eq!(read(&remote), payload);

        // Host to guest again, this time into an existing directory.
        copy_in(&mut session, &src, guest.path().to_str().unwrap()).unwrap();
        assert_eq!(read(&guest.path().join("payload.bin")), payload);

        // A directory, back out to a name of its own.
        let tree = guest.path().join("tree");
        fs::create_dir(&tree).unwrap();
        fs::write(tree.join("one"), b"first").unwrap();
        fs::create_dir(tree.join("nested")).unwrap();
        fs::write(tree.join("nested/two"), b"second").unwrap();

        let out = host.path().join("pulled");
        copy_out(&mut session, tree.to_str().unwrap(), &out).unwrap();
        assert_eq!(read(&out.join("one")), b"first");
        assert_eq!(read(&out.join("nested/two")), b"second");

        // And a single file back out, into an existing directory.
        copy_out(&mut session, remote.to_str().unwrap(), host.path()).unwrap();
        assert_eq!(read(&host.path().join("copied.bin")), payload);

        session.close();
        let _ = shell.kill();
        let _ = shell.wait();
    }

    /// Nothing here is a devkit guest, so the gate has to hold, and say why.
    #[test]
    fn test_require_devkit_refuses_a_guest_without_it() {
        let Some((sock, mut shell)) = console() else {
            eprintln!("skipping: no pty-backed shell with tar and base64 here");
            return;
        };

        let mut session = Session::new(sock);
        let err = require_devkit(&mut session).unwrap_err().to_string();

        assert!(err.contains("devkit"), "unhelpful error: {}", err);

        session.close();
        let _ = shell.kill();
        let _ = shell.wait();
    }

    /// A guest-side failure has to surface as the guest's own complaint, not as
    /// a decoding error from the stderr that leaked into the payload.
    #[test]
    fn test_copy_out_reports_the_guest_error() {
        let Some((sock, mut shell)) = console() else {
            eprintln!("skipping: no pty-backed shell with tar and base64 here");
            return;
        };

        let mut session = Session::new(sock);
        let host = tempdir().unwrap();

        let err = copy_out(
            &mut session,
            "/nonexistent-a4f2/thing",
            &host.path().join("out"),
        )
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("does not exist in the guest"),
            "unhelpful error: {}",
            err
        );

        session.close();
        let _ = shell.kill();
        let _ = shell.wait();
    }

    #[test]
    fn test_endpoint_parse() {
        assert_eq!(
            Endpoint::parse("abc123:/var/log").unwrap(),
            Endpoint::Guest {
                sandbox_id: "abc123".to_string(),
                path: "/var/log".to_string(),
            }
        );

        // No colon, so a host path.
        assert_eq!(
            Endpoint::parse("/tmp/out").unwrap(),
            Endpoint::Host(PathBuf::from("/tmp/out"))
        );
        assert_eq!(
            Endpoint::parse("out").unwrap(),
            Endpoint::Host(PathBuf::from("out"))
        );

        // A colon behind a path separator belongs to the host path.
        assert_eq!(
            Endpoint::parse("/tmp/a:b").unwrap(),
            Endpoint::Host(PathBuf::from("/tmp/a:b"))
        );

        // A sandbox id with a relative path is too ambiguous to accept.
        assert!(Endpoint::parse("abc123:var/log").is_err());
    }

    #[test]
    fn test_path_helpers() {
        assert_eq!(guest_basename("/var/log/kata.log").unwrap(), "kata.log");
        assert_eq!(guest_basename("/var/log/").unwrap(), "log");
        assert!(guest_basename("/").is_err());

        assert_eq!(guest_dirname("/var/log/kata.log").unwrap(), "/var/log");
        assert_eq!(guest_dirname("/kata.log").unwrap(), "/");
        assert!(guest_dirname("/").is_err());

        assert_eq!(host_parent(Path::new("out.tar")), PathBuf::from("."));
        assert_eq!(
            host_parent(Path::new("/tmp/out.tar")),
            PathBuf::from("/tmp")
        );
    }

    /// What goes out as base64 lines has to come back byte for byte, including
    /// the payload sizes that do not land on a line boundary.
    #[test]
    fn test_base64_round_trip() {
        for len in [0usize, 1, 56, 57, 58, 4096] {
            let payload: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();

            let mut wire = Vec::new();
            let mut encoder = Base64Lines::new(&mut wire);
            // Write in awkward slices, so a line boundary falls mid-write.
            for chunk in payload.chunks(13) {
                encoder.write_all(chunk).unwrap();
            }
            encoder.finish().unwrap();

            for line in wire.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
                assert!(line.len() <= 76, "line of {} chars is too long", line.len());
            }

            let mut back = Vec::new();
            let mut decoder = Base64Decoder::new(&mut back);
            for chunk in wire.chunks(7) {
                decoder.write_all(chunk).unwrap();
            }
            decoder.finish().unwrap();

            assert_eq!(back, payload, "round trip of {len} bytes");
        }
    }

    /// The guest's stderr can end up in the stream we are decoding.
    #[test]
    fn test_base64_decoder_rejects_junk() {
        let mut out = Vec::new();
        let mut decoder = Base64Decoder::new(&mut out);

        assert!(decoder.write_all(b"tar: /nope: Cannot stat\n").is_err());
    }

    #[test]
    fn test_pipe_carries_everything() {
        let (mut tx, mut rx) = pipe();
        let payload: Vec<u8> = (0..10_000).map(|i| (i % 251) as u8).collect();

        let expected = payload.clone();
        let writer = thread::spawn(move || {
            for chunk in payload.chunks(101) {
                tx.write_all(chunk).unwrap();
            }
        });

        let mut back = Vec::new();
        rx.read_to_end(&mut back).unwrap();
        writer.join().unwrap();

        assert_eq!(back, expected);
    }

    #[test]
    fn test_diagnostic_keeps_the_tail() {
        let mut diagnostic = Diagnostic::default();
        diagnostic
            .write_all(&vec![b'x'; DIAGNOSTIC_BYTES * 2])
            .unwrap();

        assert_eq!(diagnostic.0.len(), DIAGNOSTIC_BYTES);

        let mut empty = Diagnostic::default();
        empty.write_all(b"  \n ").unwrap();
        assert_eq!(empty.detail(), "");
    }
}
