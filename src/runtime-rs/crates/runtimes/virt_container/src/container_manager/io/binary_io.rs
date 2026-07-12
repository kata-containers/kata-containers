// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    sys::signal::{kill, Signal},
    unistd::{pipe2, Pid},
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWrite},
    net::unix::pipe::Sender,
    process::{Child, Command},
};
use url::Url;

const BINARY_IO_PROC_DRAIN_TIMEOUT: Duration = Duration::from_secs(12);
const BINARY_IO_PROC_TERM_TIMEOUT: Duration = Duration::from_secs(12);

pub(crate) struct BinaryIo {
    pub(crate) stdout: Box<dyn AsyncWrite + Send + Unpin>,
    pub(crate) stderr: Box<dyn AsyncWrite + Send + Unpin>,
    pub(crate) logger: BinaryLogger,
}

pub(crate) struct BinaryLogger {
    child: Child,
}

impl BinaryLogger {
    pub(crate) async fn shutdown(mut self) {
        match tokio::time::timeout(BINARY_IO_PROC_DRAIN_TIMEOUT, self.child.wait()).await {
            Ok(Ok(status)) => {
                info!(sl!(), "binary logger exited with {}", status);
                return;
            }
            Ok(Err(err)) => {
                warn!(sl!(), "failed to wait for binary logger: {}", err);
                return;
            }
            Err(_) => warn!(
                sl!(),
                "binary logger did not exit after EOF; terminating it"
            ),
        }

        if let Some(pid) = self.child.id() {
            if let Err(err) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                warn!(sl!(), "failed to terminate binary logger: {}", err);
                let _ = self.child.start_kill();
            }
        }

        match tokio::time::timeout(BINARY_IO_PROC_TERM_TIMEOUT, self.child.wait()).await {
            Ok(Ok(status)) => info!(sl!(), "binary logger exited with {}", status),
            Ok(Err(err)) => warn!(
                sl!(),
                "failed to wait for binary logger after SIGTERM: {}", err
            ),
            Err(_) => {
                warn!(sl!(), "binary logger ignored SIGTERM; killing it");
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
            }
        }
    }
}

fn pipe() -> Result<(OwnedFd, OwnedFd)> {
    pipe2(OFlag::O_CLOEXEC).context("create pipe")
}

fn duplicate_for_child(fd: &OwnedFd) -> Result<OwnedFd> {
    // Keep the source descriptors away from their fd 3/4/5 destinations so
    // the dup2 calls cannot overwrite one another.
    let duplicated = fcntl(fd, FcntlArg::F_DUPFD_CLOEXEC(6)).context("duplicate logger fd")?;

    // SAFETY: F_DUPFD_CLOEXEC returns a new descriptor owned by the caller.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicated) })
}

fn pipe_writer(fd: OwnedFd) -> Result<Box<dyn AsyncWrite + Send + Unpin>> {
    Ok(Box::new(
        Sender::from_owned_fd(fd).context("register logger pipe")?,
    ))
}

fn command_args(uri: &Url) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut args = Vec::new();

    // Go's url.Values iteration passes each key and only its first value.
    for (key, value) in uri.query_pairs() {
        if seen.insert(key.to_string()) {
            args.push(key.into_owned());
            args.push(value.into_owned());
        }
    }

    args
}

fn command_path(uri: &Url) -> Result<PathBuf> {
    if uri.path().is_empty() {
        return Err(anyhow!("binary logger URI has an empty path"));
    }

    uri.to_file_path()
        .map_err(|_| anyhow!("binary logger URI has an invalid path"))
}

pub(crate) async fn open(uri: &Url, container_id: &str, namespace: &str) -> Result<BinaryIo> {
    let binary = command_path(uri)?;

    let (stdout_read, stdout_write) = pipe().context("create stdout pipe")?;
    let (stderr_read, stderr_write) = pipe().context("create stderr pipe")?;
    let (ready_read, ready_write) = pipe().context("create readiness pipe")?;

    let child_stdout = duplicate_for_child(&stdout_read)?;
    let child_stderr = duplicate_for_child(&stderr_read)?;
    let child_ready = duplicate_for_child(&ready_write)?;
    let child_fds = [
        child_stdout.as_raw_fd(),
        child_stderr.as_raw_fd(),
        child_ready.as_raw_fd(),
    ];

    let mut command = Command::new(&binary);
    command
        .args(command_args(uri))
        .env_clear()
        .env("CONTAINER_ID", container_id)
        .env("CONTAINER_NAMESPACE", namespace)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.kill_on_drop(true);

    // Match containerd's binary logger ABI: stdout, stderr, and readiness are
    // inherited as fd 3, 4, and 5 respectively.
    // SAFETY: the closure only invokes async-signal-safe descriptor operations.
    unsafe {
        command.pre_exec(move || {
            for (source, destination) in child_fds.iter().copied().zip(3..=5) {
                if libc::dup2(source, destination) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }

    let child = command
        .spawn()
        .with_context(|| format!("start binary logger {}", binary.display()))?;

    drop(stdout_read);
    drop(stderr_read);
    drop(ready_write);
    drop(child_stdout);
    drop(child_stderr);
    drop(child_ready);

    // The legacy protocol accepts either one byte or EOF as readiness. Like
    // the Go runtime, logger startup waits for that handshake.
    let mut ready = File::from_std(std::fs::File::from(ready_read));
    let mut byte = [0_u8; 1];
    ready
        .read(&mut byte)
        .await
        .context("wait for binary logger readiness")?;

    Ok(BinaryIo {
        stdout: pipe_writer(stdout_write)?,
        stderr: pipe_writer(stderr_write)?,
        logger: BinaryLogger { child },
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, time::SystemTime};

    use tokio::io::AsyncWriteExt;

    use super::*;

    #[test]
    fn query_parameters_match_go_binary_logger_arguments() {
        let uri =
            Url::parse("binary:///logger?config=%2Frun%2Flog.json&empty=&config=ignored").unwrap();
        assert_eq!(command_args(&uri), ["config", "/run/log.json", "empty", ""]);
    }

    #[test]
    fn decodes_binary_logger_path_like_go() {
        use std::os::unix::ffi::OsStrExt;

        let uri = Url::parse("binary:///opt/log%20helper%25/a%2Fb/%FF").unwrap();
        assert_eq!(
            command_path(&uri).unwrap().as_os_str().as_bytes(),
            b"/opt/log helper%/a/b/\xff"
        );
    }

    #[tokio::test]
    async fn streams_to_binary_logger_and_passes_metadata() {
        let dir = std::env::temp_dir().join(format!(
            "kata-binary-logger-test-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let logger = dir.join("logger helper%");
        let output = dir.join("output");
        fs::write(
            &logger,
            r#"#!/bin/sh
printf x >&5
printf '%s\n%s\n%s\n%s\n' "$CONTAINER_ID" "$CONTAINER_NAMESPACE" "$1" "$2" > "$2.meta"
/bin/cat <&3 > "$2.stdout"
/bin/cat <&4 > "$2.stderr"
"#,
        )
        .unwrap();
        fs::set_permissions(&logger, fs::Permissions::from_mode(0o755)).unwrap();

        let uri = Url::parse(&format!(
            "binary://{}/logger%20helper%25?output={}",
            dir.display(),
            output.display()
        ))
        .unwrap();
        let mut io = open(&uri, "container-id", "k8s.io").await.unwrap();
        io.stdout.write_all(b"stdout data\n").await.unwrap();
        io.stderr.write_all(b"stderr data\n").await.unwrap();
        drop(io.stdout);
        drop(io.stderr);
        io.logger.shutdown().await;

        assert_eq!(
            fs::read(output.with_extension("stdout")).unwrap(),
            b"stdout data\n"
        );
        assert_eq!(
            fs::read(output.with_extension("stderr")).unwrap(),
            b"stderr data\n"
        );
        assert_eq!(
            fs::read_to_string(output.with_extension("meta")).unwrap(),
            format!("container-id\nk8s.io\noutput\n{}\n", output.display())
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
