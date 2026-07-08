// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    io,
    os::unix::{
        fs::{FileTypeExt, OpenOptionsExt},
        io::{FromRawFd, IntoRawFd, RawFd},
        prelude::AsRawFd,
    },
    pin::Pin,
    process::Stdio,
    sync::{Arc, Mutex},
    task::{Context as TaskContext, Poll},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncRead, AsyncWrite},
    process::{Child, Command},
};
use url::Url;

/// Grace period given to a binary logger process to flush and exit after we
/// close its input pipes, before we forcibly kill it. Matches the Go runtime's
/// `binaryIOProcTermTimeout` (12s).
const BINARY_IO_PROC_TERM_TIMEOUT: Duration = Duration::from_secs(12);

/// Bounded time to wait for the binary logger's readiness signal on fd 5.
/// The Go runtime blocks unconditionally; we add a timeout so a misbehaving
/// logger cannot hang container start indefinitely.
const BINARY_IO_READY_TIMEOUT: Duration = Duration::from_secs(2);

/// Block until the logger signals readiness on `sync_r` (a byte written or the
/// fd closed = EOF), or until `timeout` elapses. Returns regardless; readiness
/// is best-effort. Does NOT close `sync_r`.
fn wait_logger_ready(sync_r: RawFd, timeout: Duration) {
    let mut pfd = libc::pollfd {
        fd: sync_r,
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
    // SAFETY: `pfd` is a fully-initialized local `pollfd` of length 1; the
    // pointer we pass is valid for `nfds=1`. The kernel only reads/writes
    // that single struct. `millis` is a well-formed c_int.
    let ret = unsafe { libc::poll(&mut pfd, 1, millis) };
    match ret {
        0 => warn!(
            sl!(),
            "binary logger readiness timed out after {:?}, proceeding", timeout
        ),
        r if r < 0 => {
            let e = io::Error::last_os_error();
            // EINTR etc.: don't fail start over a readiness poll.
            warn!(sl!(), "binary logger readiness poll failed: {:?}", e);
        }
        _ => {
            // Readable (data or EOF). Drain the single readiness byte if any.
            let mut buf = [0u8; 1];
            // SAFETY: `buf` is a stack array of exactly 1 byte and lives for
            // the duration of the call; the kernel writes at most 1 byte
            // into it. A short read or EOF is fine (we ignore the count).
            let _ = unsafe { libc::read(sync_r, buf.as_mut_ptr() as *mut libc::c_void, 1) };
            info!(sl!(), "binary logger ready");
        }
    }
}

/// Clear O_NONBLOCK for an fd (turn it into blocking mode).
fn set_flag_with_blocking(fd: RawFd) {
    // SAFETY: `F_GETFL` is a pure read of the fd's flags; passing an invalid
    // fd is well-defined (returns -1 with errno set) and handled below.
    let flag = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flag < 0 {
        error!(sl!(), "failed to fcntl(F_GETFL) fd {} ret {}", fd, flag);
        return;
    }

    // SAFETY: `F_SETFL` writes the given flags to the fd; failure is
    // non-fatal and just logged.
    let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flag & !libc::O_NONBLOCK) };
    if ret < 0 {
        error!(sl!(), "failed to fcntl(F_SETFL) fd {} ret {}", fd, ret);
    }
}

fn open_fifo_write(path: &str) -> Result<File> {
    let std_file = std::fs::OpenOptions::new()
        .write(true)
        // It's not for non-block openning FIFO but for non-block stream which
        // will be add into tokio runtime.
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("open fifo for write: {path}"))?;

    // Debug
    let meta = std_file.metadata()?;
    if !meta.file_type().is_fifo() {
        debug!(sl!(), "[DEBUG]{} is not a fifo (type mismatch)", path);
    }

    set_flag_with_blocking(std_file.as_raw_fd());

    Ok(File::from_std(std_file))
}

/// A minimal RAII guard over a raw fd: closes on drop unless ownership is
/// released via [`PipeFd::into_raw`]. Used to keep pipe fds leak-free across
/// the `?` early returns in [`open_binary_io`].
struct PipeFd(RawFd);

impl PipeFd {
    fn as_raw(&self) -> RawFd {
        self.0
    }

    /// Take ownership of the raw fd; the caller becomes responsible for
    /// closing it (or handing it off to another owner such as `File` or
    /// `Stdio`). The guard's destructor becomes a no-op.
    fn into_raw(mut self) -> RawFd {
        let fd = self.0;
        self.0 = -1;
        fd
    }
}

impl Drop for PipeFd {
    fn drop(&mut self) {
        if self.0 >= 0 {
            // SAFETY: `self.0` is a fd we exclusively own (created by
            // `create_pipe` and not handed to any other owner, since
            // `into_raw` sets it to -1). Double-close is prevented by the
            // guard above.
            unsafe {
                libc::close(self.0);
            }
        }
    }
}

/// Create a blocking pipe, returning `(read, write)` wrapped in [`PipeFd`]
/// guards that will close the fds on drop unless released.
fn create_pipe() -> Result<(PipeFd, PipeFd)> {
    use nix::unistd::pipe;
    let (r, w) = pipe().context("pipe()")?;
    Ok((PipeFd(r.into_raw_fd()), PipeFd(w.into_raw_fd())))
}

/// A handle to a spawned shim v2 "binary" logger process. A single logger
/// process handles both stdout and stderr for one container, following the
/// containerd binary IO convention.
#[derive(Debug)]
struct BinaryLoggerProc {
    child: Option<Child>,
}

impl BinaryLoggerProc {
    /// Reap the logger process gracefully in the background: SIGTERM, wait up
    /// to the grace period, then SIGKILL. Called once, when the last stream
    /// sink referencing this logger is dropped.
    fn reap(&mut self) {
        if let Some(mut child) = self.child.take() {
            let pid = child.id();
            tokio::spawn(async move {
                if let Some(pid) = pid {
                    // SAFETY: `kill(2)` with SIGTERM to our own child pid is
                    // safe; if the child has already been reaped by the
                    // kernel or waited on, kill returns ESRCH which we
                    // intentionally ignore.
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                    }
                }
                match tokio::time::timeout(BINARY_IO_PROC_TERM_TIMEOUT, child.wait()).await {
                    Ok(Ok(status)) => info!(sl!(), "binary logger exited: {:?}", status),
                    Ok(Err(e)) => warn!(sl!(), "failed to wait binary logger: {:?}", e),
                    Err(_) => {
                        warn!(sl!(), "binary logger did not exit in time, killing");
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                    }
                }
            });
        }
    }
}

impl Drop for BinaryLoggerProc {
    fn drop(&mut self) {
        self.reap();
    }
}

/// Result of spawning a binary logger: the stdout and stderr write ends (fed
/// into the container IO copy loop) plus a shared handle to the logger process
/// that is reaped when both sinks are dropped.
struct BinaryIo {
    stdout: File,
    stderr: File,
    proc: Arc<Mutex<BinaryLoggerProc>>,
}

/// Spawn a shim v2 "binary" logging process.
///
/// The URI looks like:
///     binary:///usr/bin/nerdctl?_NERDCTL_INTERNAL_LOGGING=%2Fvar%2Flib%2Fnerdctl%2F<id>
///
/// Following containerd's binary IO convention:
/// - the binary path is taken from the URI path/host;
/// - query parameters become `key value` command line arguments;
/// - `CONTAINER_ID` / `CONTAINER_NAMESPACE` are injected as environment vars;
/// - the container stdout/stderr read ends are passed as fd 3 / fd 4, and a
///   "ready" sync pipe write end as fd 5 (i.e. `ExtraFiles[0..3]`);
/// - one logger process serves both stdout and stderr.
///
/// The write ends of the stdout/stderr pipes are returned as the IO sinks.
fn open_binary_io(url: &Url, container_id: &str, namespace: &str) -> Result<BinaryIo> {
    // Reconstruct the binary path. nerdctl uses `binary:///usr/bin/nerdctl`
    // (empty host, path holds everything); others may use `binary://host/path`.
    let host = url.host_str().unwrap_or("");
    let path = url.path();
    let bin = if host.is_empty() {
        path.to_string()
    } else {
        format!("/{}{}", host, path)
    };
    if bin.is_empty() {
        return Err(anyhow!("binary logger uri has empty path: {}", url));
    }

    // Query pairs become `key value` argv, matching containerd/Go.
    let mut args: Vec<String> = Vec::new();
    for (k, v) in url.query_pairs() {
        args.push(k.to_string());
        if !v.is_empty() {
            args.push(v.to_string());
        }
    }

    info!(
        sl!(),
        "spawn binary logger: bin={} args={:?} id={} ns={}", bin, args, container_id, namespace
    );

    // Data pipes: container stdout/stderr flow from the write end (kept in the
    // shim) to the read end (handed to the logger as fd 3 / fd 4).
    let (out_r, out_w) = create_pipe().context("create stdout pipe")?;
    let (err_r, err_w) = create_pipe().context("create stderr pipe")?;
    // Sync pipe: logger signals readiness by writing/closing its fd 5; the
    // shim reads the read end.
    let (sync_r, sync_w) = create_pipe().context("create sync pipe")?;

    // No CLOEXEC dance is needed: parent-held ends are never passed to the
    // child (tokio's Command closes non-stdio fds before exec), and dup2
    // clears FD_CLOEXEC on its target, so fd 3/4/5 inherit across exec.

    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .env("CONTAINER_ID", container_id)
        .env("CONTAINER_NAMESPACE", namespace)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Release the child-side fds from their guards; spawn-failure path below
    // closes them explicitly, and on success the child inherits their dup'd
    // copies via pre_exec.
    let out_r_raw = out_r.into_raw();
    let err_r_raw = err_r.into_raw();
    let sync_w_raw = sync_w.into_raw();

    // Place fd 3/4/5 in the child, matching containerd's ExtraFiles[0..3].
    // SAFETY: pre_exec runs in the forked child before exec; only
    // async-signal-safe libc calls are used.
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(out_r_raw, 3) < 0
                || libc::dup2(err_r_raw, 4) < 0
                || libc::dup2(sync_w_raw, 5) < 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            // SAFETY: on spawn failure `pre_exec` never ran, so these three
            // fds are still exclusively owned by us (their `PipeFd` guards
            // were disarmed just above by `into_raw`). Close them here to
            // avoid an fd leak.
            unsafe {
                libc::close(out_r_raw);
                libc::close(err_r_raw);
                libc::close(sync_w_raw);
            }
            return Err(anyhow::Error::from(e))
                .with_context(|| format!("spawn binary logger {}", bin));
        }
    };

    // Spawn succeeded: close the parent-side originals of the child-inherited fds.
    // SAFETY: the child now holds its own dup'd copies (fd 3/4/5); the
    // parent-side originals are exclusively owned by us and unused.
    unsafe {
        libc::close(out_r_raw);
        libc::close(err_r_raw);
        libc::close(sync_w_raw);
    }

    // Readiness handshake (mirrors containerd's Go `newBinaryIO`).
    //
    // The logger signals readiness via fd 5: it either writes a byte or closes
    // the fd (EOF) once it has finished initializing and is ready to consume
    // fd 3 / fd 4. We MUST block here until that happens, before the IO copy
    // loop starts and before `TaskStart` is published. Otherwise, for a very
    // short-lived container, the copy loop can push the single output line and
    // close the write end before the logger has started reading, so the logger
    // exits without draining and the log file ends up empty.
    //
    // A bounded timeout guards against a misbehaving logger hanging container
    // start forever; on timeout we proceed anyway (best-effort logging).
    wait_logger_ready(sync_r.as_raw(), BINARY_IO_READY_TIMEOUT);
    drop(sync_r);

    // SAFETY: `out_w` / `err_w` are pipe fds we exclusively own; `into_raw`
    // disarms their `PipeFd` destructor so no double-close can occur, and
    // ownership is transferred to the returned `File` which will close them
    // on drop.
    let stdout = File::from_std(unsafe { std::fs::File::from_raw_fd(out_w.into_raw()) });
    let stderr = File::from_std(unsafe { std::fs::File::from_raw_fd(err_w.into_raw()) });

    Ok(BinaryIo {
        stdout,
        stderr,
        proc: Arc::new(Mutex::new(BinaryLoggerProc { child: Some(child) })),
    })
}

pub struct ShimIo {
    pub stdin: Option<Box<dyn AsyncRead + Send + Unpin>>,
    pub stdout: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    pub stderr: Option<Box<dyn AsyncWrite + Send + Unpin>>,
}

impl ShimIo {
    pub async fn new(
        stdin: &Option<String>,
        stdout: &Option<String>,
        stderr: &Option<String>,
        container_id: &str,
        namespace: &str,
    ) -> Result<Self> {
        info!(
            sl!(),
            "new shim io stdin {:?} stdout {:?} stderr {:?}", stdin, stdout, stderr
        );

        let stdin_fd: Option<Box<dyn AsyncRead + Send + Unpin>> = if let Some(stdin) = stdin {
            info!(sl!(), "open stdin {:?}", &stdin);

            // Since we had opened the stdin as write mode in the Process::new function,
            // thus it wouldn't be blocked to open it as read mode.
            match OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&stdin)
                .await
            {
                Ok(file) => {
                    // Set it to blocking to avoid infinitely handling EAGAIN when the reader is empty
                    set_flag_with_blocking(file.as_raw_fd());
                    Some(Box::new(file))
                }
                Err(err) => {
                    error!(sl!(), "failed to open {} error {:?}", &stdin, err);
                    None
                }
            }
        } else {
            None
        };

        let get_url = |url: &Option<String>| -> Option<Url> {
            info!(sl!(), "get url for {:?}", url);

            match url {
                None => None,
                Some(out) => match Url::parse(out.as_str()) {
                    Err(url::ParseError::RelativeUrlWithoutBase) => {
                        Url::parse(&format!("fifo://{}", out)).ok()
                    }
                    Err(err) => {
                        warn!(sl!(), "unable to parse stdio uri: {}", err);
                        None
                    }
                    Ok(u) => Some(u),
                },
            }
        };

        let stdout_url = get_url(stdout);
        let stderr_url = get_url(stderr);

        // Determine the scheme. A binary logger serves both stdout and stderr
        // with a single process, so it is handled specially. In practice (see
        // containerd's cio.BinaryIO and nerdctl's usage) stdout and stderr URIs
        // are identical when this mode is used, but we defensively pick either
        // one so that a stderr-only binary URI still works.
        let is_binary = |u: &Option<Url>| -> bool {
            u.as_ref().map(|u| u.scheme() == "binary").unwrap_or(false)
        };

        if is_binary(&stdout_url) || is_binary(&stderr_url) {
            let url = if is_binary(&stdout_url) {
                stdout_url.clone().unwrap()
            } else {
                stderr_url.clone().unwrap()
            };
            let cid = container_id.to_string();
            let ns = namespace.to_string();
            // `open_binary_io` blocks on the logger readiness handshake, so run
            // it on a blocking thread to avoid stalling the tokio worker.
            let result = tokio::task::spawn_blocking(move || open_binary_io(&url, &cid, &ns))
                .await
                .context("join open_binary_io")?;
            match result {
                Ok(bio) => {
                    let proc = bio.proc.clone();
                    let stdout_sink = ShimIoWrite::Binary {
                        file: Some(bio.stdout),
                        proc: proc.clone(),
                    };
                    let stderr_sink = ShimIoWrite::Binary {
                        file: Some(bio.stderr),
                        proc,
                    };
                    return Ok(Self {
                        stdin: stdin_fd,
                        stdout: Some(Box::new(stdout_sink)),
                        stderr: Some(Box::new(stderr_sink)),
                    });
                }
                Err(err) => {
                    error!(sl!(), "failed to open binary logger, error {:?}", err);
                    // Fall through: no stdout/stderr sink. The container still
                    // runs and can be stopped; logs are just dropped.
                    return Ok(Self {
                        stdin: stdin_fd,
                        stdout: None,
                        stderr: None,
                    });
                }
            }
        }

        // FIFO / other schemes: open each stream independently.
        let get_fd = |url: &Option<Url>| -> Option<Box<dyn AsyncWrite + Send + Unpin>> {
            info!(sl!(), "get fd for {:?}", &url);
            if let Some(url) = url {
                match url.scheme() {
                    "fifo" => {
                        let path = url.path();
                        match open_fifo_write(path) {
                            Ok(f) => return Some(Box::new(ShimIoWrite::File(f))),
                            Err(err) => {
                                error!(sl!(), "failed to open fifo {} error {:?}", path, err)
                            }
                        }
                    }
                    other => {
                        warn!(sl!(), "unsupported io scheme {}", other);
                    }
                }
            }
            None
        };

        Ok(Self {
            stdin: stdin_fd,
            stdout: get_fd(&stdout_url),
            stderr: get_fd(&stderr_url),
        })
    }
}

enum ShimIoWrite {
    File(File),
    Binary {
        file: Option<File>,
        // Shared logger process handle; reaped when the last sink drops.
        proc: Arc<Mutex<BinaryLoggerProc>>,
    },
}

impl Drop for ShimIoWrite {
    fn drop(&mut self) {
        if let ShimIoWrite::Binary { file, proc } = self {
            // Close our write end so the logger sees EOF on the corresponding
            // stream.
            file.take();
            // Reap the logger when the last sink drops. Only the stdout and
            // stderr sinks ever hold this Arc (both created in `ShimIo::new`
            // and immediately handed to the caller); no other clones exist,
            // so `strong_count <= 1` reliably identifies the final drop
            // without a TOCTOU concern.
            if Arc::strong_count(proc) <= 1 {
                if let Ok(mut p) = proc.lock() {
                    p.reap();
                }
            }
        }
    }
}

impl AsyncWrite for ShimIoWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            ShimIoWrite::File(f) => Pin::new(f).poll_write(cx, buf),
            ShimIoWrite::Binary { file, .. } => match file.as_mut() {
                Some(f) => Pin::new(f).poll_write(cx, buf),
                None => Poll::Ready(Ok(0)),
            },
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            ShimIoWrite::File(f) => Pin::new(f).poll_flush(cx),
            ShimIoWrite::Binary { file, .. } => match file.as_mut() {
                Some(f) => Pin::new(f).poll_flush(cx),
                None => Poll::Ready(Ok(())),
            },
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            ShimIoWrite::File(f) => Pin::new(f).poll_shutdown(cx),
            ShimIoWrite::Binary { file, .. } => match file.as_mut() {
                Some(f) => Pin::new(f).poll_shutdown(cx),
                None => Poll::Ready(Ok(())),
            },
        }
    }
}

