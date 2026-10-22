// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

//! Optional guest-image envd injection. No supervisor or readiness wait.

use std::{
    os::unix::process::CommandExt,
    process::{Command, Stdio},
};

use anyhow::{Context, Result};
use kata_types::annotations::{EnvdAnnotation, KATA_ANNO_CFG_RUNTIME_ENVD};
use nix::unistd::{fork, ForkResult};

use kata_types::mount::ENVD_CONTAINER_PATH;

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
    command.args(args).stdin(Stdio::null());
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
            let error = command(&args, ENVD_CONTAINER_PATH).exec();
            eprintln!("envd start failed: {}", error);
            unsafe { libc::_exit(127) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

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
             test \"$(readlink /proc/self/fd/0)\" = /dev/null\n\
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
}
