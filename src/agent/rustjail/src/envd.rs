// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

//! Launch a guest-provided envd in the workload's context.

use std::{
    convert::Infallible,
    ffi::CString,
    fs::File,
    io::Write,
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::ffi::OsStringExt,
    },
    path::Path,
};

use anyhow::{ensure, Context, Result};
use kata_types::annotations::KATA_ANNO_CFG_RUNTIME_ENVD;
use nix::{
    errno::Errno,
    fcntl::{fcntl, open, AtFlags, FcntlArg, OFlag},
    sys::stat::{fstat, Mode, SFlag},
    unistd::{dup2_stderr, dup2_stdin, dup2_stdout, execveat, fork, ForkResult},
};

use serde::Deserialize;

const ENVD_EXTENSION_PATH: &str = "/run/kata-extensions/kata-envd/bin/kata-envd";
const ENVD_GUEST_PATH: &str = "/usr/libexec/kata-containers/kata-envd";

/// Configuration for a sandbox's envd process.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvdAnnotation {
    /// Arguments passed to envd, excluding the executable name.
    args: Vec<String>,
}

impl EnvdAnnotation {
    /// Parse and validate an envd sandbox annotation.
    fn parse(raw: &str) -> Result<Self> {
        ensure!(
            raw.trim_start().starts_with('{'),
            "envd annotation must be a JSON object"
        );
        let config: Self = serde_json::from_str(raw).context("invalid envd annotation")?;
        ensure!(
            config.args.iter().all(|arg| !arg.contains('\0')),
            "envd args must not contain NUL"
        );
        Ok(config)
    }
}

pub(crate) struct Envd {
    binary_fd: OwnedFd,
    args: Vec<String>,
}

impl Envd {
    /// Open and validate the executable before switching to the container root.
    pub(crate) fn prepare_if_requested(spec: &oci_spec::runtime::Spec) -> Result<Option<Self>> {
        let Some(raw) = spec
            .annotations()
            .as_ref()
            .and_then(|annotations| annotations.get(KATA_ANNO_CFG_RUNTIME_ENVD))
        else {
            return Ok(None);
        };
        let config = EnvdAnnotation::parse(raw)?;
        let binary_fd =
            Self::open_binary(Path::new(ENVD_EXTENSION_PATH), Path::new(ENVD_GUEST_PATH))?;
        Ok(Some(Self {
            binary_fd,
            args: config.args,
        }))
    }

    fn open_binary(extension_path: &Path, image_path: &Path) -> Result<OwnedFd> {
        let flags = OFlag::O_PATH | OFlag::O_CLOEXEC;
        let (path, open_result) = match open(extension_path, flags, Mode::empty()) {
            Err(Errno::ENOENT) => (image_path, open(image_path, flags, Mode::empty())),
            result => (extension_path, result),
        };
        let binary_fd = open_result
            .with_context(|| format!("open envd guest executable {}", path.display()))?;
        let metadata = fstat(&binary_fd)
            .with_context(|| format!("inspect envd guest executable {}", path.display()))?;
        ensure!(
            SFlag::from_bits_truncate(metadata.st_mode) == SFlag::S_IFREG
                && metadata.st_mode & 0o111 != 0,
            "envd guest source must be a regular executable file: {}",
            path.display()
        );
        Ok(binary_fd)
    }

    /// Fork from the single-threaded init helper after container setup.
    pub(crate) fn start(self) {
        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => (),
            Err(err) => eprintln!("envd fork failed: {}", err),
            Ok(ForkResult::Child) => {
                // Keep exec failures visible while closing workload streams on exec.
                let error_output = fcntl(std::io::stderr(), FcntlArg::F_DUPFD_CLOEXEC(3))
                    .ok()
                    .map(|fd| unsafe { File::from_raw_fd(fd) });
                let error = self.exec().unwrap_err();
                if let Some(mut output) = error_output {
                    let _ = writeln!(output, "envd start failed: {error:#}");
                }
                unsafe { libc::_exit(127) }
            }
        }
    }

    fn exec(&self) -> Result<Infallible> {
        let mut argv = Vec::with_capacity(self.args.len() + 1);
        argv.push(CString::new("kata-envd")?);
        for arg in &self.args {
            argv.push(CString::new(arg.as_str())?);
        }
        let env: Vec<CString> = std::env::vars_os()
            .map(|(mut key, value)| {
                key.push("=");
                key.push(value);
                CString::new(key.into_vec())
            })
            .collect::<Result<_, _>>()?;

        let null = File::options().read(true).write(true).open("/dev/null")?;
        dup2_stdin(&null)?;
        dup2_stdout(&null)?;
        dup2_stderr(&null)?;

        execveat(
            &self.binary_fd,
            CString::default().as_c_str(),
            &argv,
            &env,
            AtFlags::AT_EMPTY_PATH,
        )
        .context("execute envd by fd")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt},
    };

    fn test_envd(path: &Path, args: Vec<String>) -> Envd {
        Envd {
            binary_fd: open(path, OFlag::O_PATH | OFlag::O_CLOEXEC, Mode::empty()).unwrap(),
            args,
        }
    }

    #[test]
    fn rejects_malformed_or_incomplete_requests() {
        for raw in [
            "",
            "null",
            "[]",
            "{",
            "{}",
            r#"{"port":49983,"args":[]}"#,
            r#"{"args":null}"#,
            r#"{"args":"serve"}"#,
            r#"{"args":[null]}"#,
            r#"{"args":["\u0000"]}"#,
            r#"{"args":[],"guest_path":"/opt/custom/envd"}"#,
        ] {
            assert!(EnvdAnnotation::parse(raw).is_err(), "{}", raw);
        }
    }

    #[test]
    fn falls_back_to_guest_image_only_if_extension_executable_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let extension = dir.path().join("extension-envd");
        let image = dir.path().join("image-envd");
        fs::write(&image, "image").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o755)).unwrap();

        let binary_fd = Envd::open_binary(&extension, &image).unwrap();
        assert_eq!(
            fstat(&binary_fd).unwrap().st_ino,
            fs::metadata(&image).unwrap().ino()
        );

        fs::write(&extension, "extension").unwrap();
        fs::set_permissions(&extension, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Envd::open_binary(&extension, &image).is_err());

        fs::set_permissions(&extension, fs::Permissions::from_mode(0o755)).unwrap();
        let binary_fd = Envd::open_binary(&extension, &image).unwrap();
        assert_eq!(
            fstat(&binary_fd).unwrap().st_ino,
            fs::metadata(&extension).unwrap().ino()
        );
    }

    #[test]
    fn guest_source_must_be_a_regular_executable() {
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
            assert!(Envd::open_binary(path, &missing).is_err(), "{:?}", path);
        }
    }

    #[test]
    fn skips_when_annotation_is_missing() {
        assert!(
            Envd::prepare_if_requested(&oci_spec::runtime::Spec::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn executes_the_original_file_after_its_path_is_replaced() {
        use nix::sys::wait::{waitpid, WaitStatus};
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("envd");
        symlink("/bin/true", &source).unwrap();
        let envd = test_envd(&source, vec![]);
        fs::remove_file(&source).unwrap();
        symlink("/bin/false", &source).unwrap();

        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                let _ = envd.exec();
                unsafe { libc::_exit(127) }
            }
            ForkResult::Parent { child } => {
                assert_eq!(waitpid(child, None).unwrap(), WaitStatus::Exited(child, 0));
            }
        }
    }

    #[test]
    fn child_exec_failure_exits_without_returning() {
        use nix::sys::wait::{waitpid, WaitStatus};

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("invalid-elf");
        fs::write(&file, "invalid executable").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        let envd = test_envd(&file, vec![]);

        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                envd.start();
                let exited = matches!(waitpid(None, None), Ok(WaitStatus::Exited(_, 127)));
                unsafe { libc::_exit(if exited { 0 } else { 1 }) }
            }
            ForkResult::Parent { child } => {
                assert_eq!(waitpid(child, None).unwrap(), WaitStatus::Exited(child, 0));
            }
        }
    }

    #[test]
    fn fd_exec_preserves_context_and_argument_boundaries() {
        use nix::sys::wait::{waitpid, WaitStatus};
        use std::os::fd::AsRawFd;

        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("result");
        let mut envd = test_envd(Path::new("/bin/sh"), vec![]);
        let control_fd = fcntl(&envd.binary_fd, FcntlArg::F_DUPFD_CLOEXEC(200)).unwrap();
        envd.binary_fd = unsafe { OwnedFd::from_raw_fd(control_fd) };
        envd.args = vec![
            "-eu".to_string(),
            "-c".to_string(),
            format!(
                "test ! -e /proc/$$/fd/{fd}\n\
                 for stream in 0 1 2; do\n\
                     test \"$(readlink /proc/$$/fd/$stream)\" = /dev/null\n\
                 done\n\
                 out=$1; shift\n\
                 printf '<%s>\\n' \"$@\" > \"$out\"\n\
                 printf '%s\\n' \"$(id -u)\" \"$(pwd)\" \"$HOME\" >> \"$out\"\n\
                 cat /proc/self/cgroup >> \"$out\"\n",
                fd = envd.binary_fd.as_raw_fd()
            ),
            "test-envd".to_string(),
            output.to_str().unwrap().to_string(),
            "serve".to_string(),
            "two words".to_string(),
            "$(literal)".to_string(),
            "".to_string(),
        ];

        match unsafe { fork() }.unwrap() {
            ForkResult::Child => {
                // Other tests change the parent's cwd.
                if std::env::set_current_dir(dir.path()).is_err() {
                    unsafe { libc::_exit(126) }
                }
                let _ = envd.exec();
                unsafe { libc::_exit(127) }
            }
            ForkResult::Parent { child } => {
                assert_eq!(waitpid(child, None).unwrap(), WaitStatus::Exited(child, 0));
                let expected = format!(
                    "<serve>\n<two words>\n<$(literal)>\n<>\n{}\n{}\n{}\n{}",
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
                let _ = test_envd(Path::new("/bin/sh"), args).exec();
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
