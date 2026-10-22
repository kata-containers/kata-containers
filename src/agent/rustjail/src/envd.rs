// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

//! Optional guest-image envd injection. No supervisor or readiness wait.

use std::{
    fs::File,
    io::Write,
    os::unix::fs::PermissionsExt,
    os::unix::io::FromRawFd,
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{ensure, Context, Result};
use kata_types::annotations::{EnvdAnnotation, KATA_ANNO_CFG_RUNTIME_ENVD};
use nix::fcntl::{fcntl, FcntlArg};
use nix::unistd::{fork, ForkResult};

use kata_types::mount::{ENVD_CONTAINER_PATH, ENVD_GUEST_PATH};

/// Validate custom guest sources before mounting them into the workload.
pub(crate) fn validate_mount_source(spec: &oci_spec::runtime::Spec) -> Result<()> {
    if !spec
        .annotations()
        .as_ref()
        .is_some_and(|annotations| annotations.contains_key(KATA_ANNO_CFG_RUNTIME_ENVD))
    {
        return Ok(());
    }

    let source = spec
        .mounts()
        .as_ref()
        .and_then(|mounts| {
            mounts
                .iter()
                .rfind(|mount| mount.destination() == Path::new(ENVD_CONTAINER_PATH))
        })
        .and_then(|mount| mount.source().as_deref())
        .context("envd mount source is missing")?;
    if source == Path::new(ENVD_GUEST_PATH) {
        return Ok(());
    }

    // Follow symlinks so directory aliases cannot expose guest directories.
    let metadata = std::fs::metadata(source)
        .with_context(|| format!("inspect envd guest source {}", source.display()))?;
    ensure!(
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        "envd guest source must be a regular executable file: {}",
        source.display()
    );
    Ok(())
}

fn args(spec: &oci_spec::runtime::Spec) -> Result<Option<Vec<String>>> {
    let Some(raw) = spec
        .annotations()
        .as_ref()
        .and_then(|a| a.get(KATA_ANNO_CFG_RUNTIME_ENVD))
    else {
        return Ok(None);
    };
    let config: EnvdAnnotation =
        serde_json::from_str(raw).context("invalid envd Runtime request")?;
    Ok(Some(config.args))
}

fn command(args: &[String], path: &str) -> Command {
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Invoked only in the single-threaded container init helper, after the Start
/// FIFO and security setup, immediately before the original process exec.
pub fn start_if_enabled(spec: &oci_spec::runtime::Spec) {
    let args = match args(spec) {
        Ok(Some(args)) => args,
        Ok(None) => return,
        Err(error) => {
            eprintln!("envd start failed: {}", error);
            return;
        }
    };
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => (),
        Err(err) => eprintln!("envd fork failed: {}", err),
        Ok(ForkResult::Child) => {
            // Keep exec failures visible, but close this copy on successful
            // exec so envd and its descendants cannot hold workload output open.
            let error_output = fcntl(std::io::stderr(), FcntlArg::F_DUPFD_CLOEXEC(3))
                .ok()
                .map(|fd| unsafe { File::from_raw_fd(fd) });
            let error = command(&args, ENVD_CONTAINER_PATH).exec();
            if let Some(mut output) = error_output {
                let _ = writeln!(output, "envd start failed: {}", error);
            }
            unsafe { libc::_exit(127) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn spec() -> oci_spec::runtime::Spec {
        let mut spec = oci_spec::runtime::Spec::default();
        spec.annotations_mut()
            .get_or_insert_with(Default::default)
            .insert(
            KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
            r#"{"port":32123,"args":["serve","--addr",":32123","--disable-ssh","--adb-addr","-"]}"#
                .to_string(),
        );
        spec
    }

    fn spec_with_source(path: &Path) -> oci_spec::runtime::Spec {
        let mut spec = spec();
        spec.mounts_mut().get_or_insert_with(Vec::new).push(
            oci_spec::runtime::MountBuilder::default()
                .source(path)
                .destination(ENVD_CONTAINER_PATH)
                .typ("bind")
                .options(vec!["bind".to_string(), "ro".to_string()])
                .build()
                .unwrap(),
        );
        spec
    }

    #[test]
    fn custom_guest_source_must_be_a_regular_executable() {
        use std::os::unix::{fs::symlink, net::UnixListener};

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("non-executable");
        fs::write(&file, "data").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        let link = dir.path().join("directory-link");
        symlink(dir.path(), &link).unwrap();
        let socket = dir.path().join("socket");
        let _listener = UnixListener::bind(&socket).unwrap();
        let missing = dir.path().join("missing");

        for path in [
            dir.path(),
            &link,
            &file,
            &socket,
            Path::new("/dev/null"),
            &missing,
        ] {
            assert!(
                validate_mount_source(&spec_with_source(path)).is_err(),
                "{:?}",
                path
            );
        }
    }

    #[test]
    fn custom_guest_source_accepts_executable_and_file_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("envd");
        fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        let link = dir.path().join("envd-link");
        symlink(&file, &link).unwrap();

        for path in [&file, &link] {
            validate_mount_source(&spec_with_source(path)).unwrap();
        }
    }

    #[test]
    fn guest_source_validation_skips_disabled_and_default_paths() {
        let mut disabled = spec_with_source(Path::new("/etc"));
        disabled.annotations_mut().as_mut().unwrap().clear();
        validate_mount_source(&disabled).unwrap();
        validate_mount_source(&spec_with_source(Path::new(ENVD_GUEST_PATH))).unwrap();
    }

    #[test]
    fn reads_only_unified_annotation() {
        let mut spec = oci_spec::runtime::Spec::default();
        assert_eq!(args(&spec).unwrap(), None);
        for raw in ["{", "{}", r#"{"port":49983}"#, r#"{"args":[]}"#] {
            spec.annotations_mut()
                .get_or_insert_with(Default::default)
                .insert(KATA_ANNO_CFG_RUNTIME_ENVD.to_string(), raw.to_string());
            assert!(args(&spec).is_err());
        }
    }

    #[test]
    fn command_preserves_argument_boundaries_and_empty_args() {
        for values in [
            vec![
                "serve",
                "--custom",
                "two words",
                "$(literal)",
                "",
                "--addr=:18080",
            ],
            vec![],
        ] {
            let mut spec = oci_spec::runtime::Spec::default();
            spec.annotations_mut()
                .get_or_insert_with(Default::default)
                .insert(
                    KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
                    serde_json::json!({"port":49983,"args":values}).to_string(),
                );
            let args = args(&spec).unwrap().unwrap();
            let cmd = command(&args, ENVD_CONTAINER_PATH);
            assert_eq!(cmd.get_program(), ENVD_CONTAINER_PATH);
            assert_eq!(cmd.get_args().collect::<Vec<_>>(), values);
        }
    }

    #[test]
    fn child_exec_failure_exits_without_returning() {
        let mut value = spec();
        value
            .annotations_mut()
            .get_or_insert_with(Default::default)
            .insert(
                KATA_ANNO_CFG_RUNTIME_ENVD.to_string(),
                serde_json::json!({"port":49983,"args":["invalid\0"]}).to_string(),
            );
        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                start_if_enabled(&value);
                let exited = matches!(
                    nix::sys::wait::waitpid(None, None),
                    Ok(nix::sys::wait::WaitStatus::Exited(_, 127))
                );
                unsafe { libc::_exit(if exited { 0 } else { 1 }) }
            }
            ForkResult::Parent { child } => {
                assert_eq!(
                    nix::sys::wait::waitpid(child, None).unwrap(),
                    nix::sys::wait::WaitStatus::Exited(child, 0)
                );
            }
        }
    }

    #[test]
    fn command_inherits_context_and_closes_cloexec_fds() {
        use std::os::unix::io::AsRawFd;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("envd");
        let output = dir.path().join("result");
        let control = tempfile::NamedTempFile::new().unwrap();
        let fd = unsafe { libc::fcntl(control.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 200) };
        assert!(fd >= 200);
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nset -eu\ntest ! -e /proc/self/fd/{fd}\n\
             for stream in 0 1 2; do\n\
             test \"$(readlink /proc/$$/fd/$stream)\" = /dev/null\n\
             done\n\
             printf '%s\\n' \"$*\" \"$(id -u)\" \"$(pwd)\" \"$HOME\" > '{out}'\n\
             cat /proc/self/cgroup >> '{out}'\n",
                fd = fd,
                out = output.display(),
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                // Other tests change the parent's cwd; set an isolated context after fork.
                if std::env::set_current_dir(dir.path()).is_err() {
                    unsafe { libc::_exit(126) }
                }
                let _ = command(&args(&spec()).unwrap().unwrap(), script.to_str().unwrap()).exec();
                unsafe { libc::_exit(127) }
            }
            ForkResult::Parent { child } => {
                assert_eq!(
                    nix::sys::wait::waitpid(child, None).unwrap(),
                    nix::sys::wait::WaitStatus::Exited(child, 0)
                );
                unsafe {
                    libc::close(fd);
                }
                let expected = format!(
                    "serve --addr :32123 --disable-ssh --adb-addr -\n{}\n{}\n{}\n{}",
                    unsafe { libc::getuid() },
                    dir.path().display(),
                    std::env::var("HOME").unwrap(),
                    fs::read_to_string("/proc/self/cgroup").unwrap(),
                );
                assert_eq!(fs::read_to_string(output).unwrap(), expected);
            }
        }
    }

    #[test]
    fn workload_output_closes_while_envd_and_its_child_are_alive() {
        use nix::{
            fcntl::OFlag,
            sys::{signal::kill, wait::waitpid},
            unistd::{dup2_stderr, dup2_stdout, pipe2, Pid},
        };
        use std::{
            io::{BufRead, BufReader, Read},
            net::Shutdown,
            os::unix::{io::IntoRawFd, net::UnixStream},
            time::Duration,
        };

        let (stdout, stdout_writer) = pipe2(OFlag::O_CLOEXEC | OFlag::O_NONBLOCK).unwrap();
        let (stderr, stderr_writer) = pipe2(OFlag::O_CLOEXEC | OFlag::O_NONBLOCK).unwrap();
        let (control, child_control) = UnixStream::pair().unwrap();
        control
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();

        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                drop(control);
                drop(stdout);
                drop(stderr);
                if dup2_stdout(&stdout_writer).is_err() || dup2_stderr(&stderr_writer).is_err() {
                    unsafe { libc::_exit(126) }
                }
                drop(stdout_writer);
                drop(stderr_writer);

                // A separate control socket keeps the fake daemon and its child
                // alive after all workload output writers have been dropped.
                let control_fd = child_control.into_raw_fd();
                if unsafe { libc::dup2(control_fd, 9) } < 0
                    || unsafe { libc::fcntl(9, libc::F_SETFD, 0) } < 0
                {
                    unsafe { libc::_exit(126) }
                }
                if control_fd != 9 {
                    unsafe { libc::close(control_fd) };
                }
                let args = vec![
                    "-c".to_string(),
                    "/bin/cat <&9 & child=$!; printf '%s\\n' \"$child\" >&9; wait \"$child\""
                        .to_string(),
                ];
                let _ = command(&args, "/bin/sh").exec();
                unsafe { libc::_exit(127) }
            }
            ForkResult::Parent { child } => {
                drop(child_control);
                drop(stdout_writer);
                drop(stderr_writer);
                defer! {
                    // EOF on the control socket lets cat exit; the shell reaps
                    // it before exiting, including if an assertion below fails.
                    let _ = control.shutdown(Shutdown::Write);
                    let _ = waitpid(child, None);
                }

                let mut descendant = String::new();
                BufReader::new(&control).read_line(&mut descendant).unwrap();
                let descendant = Pid::from_raw(descendant.trim().parse().unwrap());
                kill(child, None).unwrap();
                kill(descendant, None).unwrap();

                // Even with both processes still running, the workload's
                // output readers must observe EOF rather than WouldBlock.
                let mut byte = [0];
                assert_eq!(File::from(stdout).read(&mut byte).unwrap(), 0);
                assert_eq!(File::from(stderr).read(&mut byte).unwrap(), 0);
            }
        }
    }
}
