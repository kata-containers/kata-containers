// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use kata_types::rootless::is_rootless;

const SELINUX_XATTR: &[u8] = b"security.selinux\0";

/// Check if SELinux is enabled on the system
pub fn is_selinux_enabled() -> bool {
    fs::read_to_string("/proc/mounts")
        .map(|buf| buf.contains("selinuxfs"))
        .unwrap_or_default()
}

/// Set the SELinux label the child will use when it executes the command.
/// Apply it after fork so the shim thread does not pass the VM label to other
/// programs, such as userdel. If setting the label fails, abort the launch.
pub fn set_exec_label_for_command(command: &mut Command, label: &str) -> io::Result<()> {
    // Allocate the label before fork. In the child, allocating or logging
    // could deadlock on a lock held by another thread in the parent.
    let label = CString::new(label)?;
    unsafe {
        command.pre_exec(move || {
            let mut fd = libc::open(
                b"/proc/thread-self/attr/exec\0".as_ptr().cast(),
                libc::O_WRONLY | libc::O_CLOEXEC,
            );
            if fd < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
                // Older kernels may not provide /proc/thread-self. The child
                // has only one thread, so /proc/self/attr/exec sets its label too.
                fd = libc::open(
                    b"/proc/self/attr/exec\0".as_ptr().cast(),
                    libc::O_WRONLY | libc::O_CLOEXEC,
                );
            }
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }

            let bytes = label.as_bytes();
            let result = loop {
                let written = libc::write(fd, bytes.as_ptr().cast(), bytes.len());
                if written < 0 {
                    let err = io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::EINTR) {
                        continue;
                    }
                    break Err(err);
                }
                if written as usize != bytes.len() {
                    break Err(io::Error::from_raw_os_error(libc::EIO));
                }
                break Ok(());
            };
            libc::close(fd);
            result
        });
    }
    Ok(())
}

/// Make the child spawned by `command` run with `label`, if one is given.
/// `name` identifies the program in error messages.
pub fn apply_exec_label(
    command: &mut tokio::process::Command,
    label: Option<&str>,
    name: &str,
) -> Result<()> {
    if let Some(label) = label {
        set_exec_label_for_command(command.as_std_mut(), label)
            .with_context(|| format!("configure {name} SELinux process label"))?;
    }
    Ok(())
}

fn sandbox_mcs_level(config: &HypervisorConfig) -> Option<&str> {
    if config.disable_selinux || !is_selinux_enabled() {
        return None;
    }
    config
        .security_info
        .selinux_label
        .as_deref()
        .and_then(context_level)
}

/// Return the MLS/MCS range of an SELinux context, e.g. `s0:c1,c2` for
/// `system_u:system_r:container_kvm_t:s0:c1,c2`.
pub fn context_level(context: &str) -> Option<&str> {
    context
        .splitn(4, ':')
        .nth(3)
        .filter(|level| !level.is_empty())
}

fn context_with_level(context: &str, level: &str) -> Result<String> {
    let mut fields = context.splitn(4, ':');
    match (fields.next(), fields.next(), fields.next()) {
        (Some(user), Some(role), Some(ty))
            if !user.is_empty() && !role.is_empty() && !ty.is_empty() =>
        {
            Ok(format!("{user}:{role}:{ty}:{level}"))
        }
        _ => Err(anyhow!("invalid SELinux context {context:?}")),
    }
}

fn path_cstring(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .with_context(|| format!("invalid path {}", path.display()))
}

/// Read the SELinux label of `path` without following symlinks.
pub fn get_file_label(path: &Path) -> Result<String> {
    let c_path = path_cstring(path)?;
    let mut buf = vec![0u8; 256];
    loop {
        // SAFETY: c_path and the attribute name are NUL-terminated and buf
        // is valid for buf.len() bytes.
        let ret = unsafe {
            libc::lgetxattr(
                c_path.as_ptr(),
                SELINUX_XATTR.as_ptr().cast(),
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        if ret >= 0 {
            buf.truncate(ret as usize);
            break;
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ERANGE) && buf.len() < 64 * 1024 {
            buf.resize(buf.len() * 2, 0);
            continue;
        }
        return Err(err).with_context(|| format!("get SELinux label of {}", path.display()));
    }
    while buf.last() == Some(&0) {
        buf.pop();
    }
    String::from_utf8(buf).with_context(|| format!("SELinux label of {}", path.display()))
}

/// Set the SELinux label of `path` without following symlinks.
pub fn set_file_label(path: &Path, label: &str) -> Result<()> {
    let c_path = path_cstring(path)?;
    // SAFETY: c_path and the attribute name are NUL-terminated and label is
    // valid for label.len() bytes.
    let ret = unsafe {
        libc::lsetxattr(
            c_path.as_ptr(),
            SELINUX_XATTR.as_ptr().cast(),
            label.as_ptr().cast(),
            label.len(),
            0,
        )
    };
    if ret != 0 {
        return Err(io::Error::last_os_error())
            .with_context(|| format!("set SELinux label {label:?} on {}", path.display()));
    }
    Ok(())
}

/// Replace the MLS/MCS range of the label on `path` with `level`, keeping
/// its SELinux user, role and type.
pub fn set_file_level(path: &Path, level: &str) -> Result<()> {
    let current = get_file_label(path)?;
    set_file_label(path, &context_with_level(&current, level)?)
}

/// Give `paths` the sandbox's MCS level, taken from its SELinux process label.
///
/// Rootful VMMs of all sandboxes run with the same UID, so MCS is what keeps
/// the VMM of one sandbox from reaching the host-side sockets of another.
/// MCS does not constrain unix socket connectto, but it does constrain
/// searching a directory and writing to a socket file, which connecting by
/// path requires. In rootless mode a failure is only logged, as the
/// sandbox's files are already private to the rootless user.
pub fn apply_sandbox_mcs_level(config: &HypervisorConfig, paths: &[&Path]) -> Result<()> {
    let level = match sandbox_mcs_level(config) {
        Some(level) => level,
        None => return Ok(()),
    };
    for path in paths {
        match set_file_level(path, level) {
            Err(err) if is_rootless() => warn!(
                sl!(),
                "leaving {} without the sandbox MCS level: {err:#}",
                path.display()
            ),
            result => {
                result.with_context(|| format!("apply sandbox MCS level to {}", path.display()))?
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_rejects_label_with_nul() {
        let mut command = Command::new("true");
        let err = set_exec_label_for_command(&mut command, "invalid\0label").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn command_label_does_not_leak_to_parent_threads() {
        if !is_selinux_enabled() {
            return;
        }
        // Reuse the test process's label instead of assuming the host policy
        // provides a particular label. Configure the command here, then launch
        // it on other threads to check that the label follows the command when
        // Tokio moves work between threads.
        let label = fs::read_to_string("/proc/self/attr/current").unwrap();
        let label = label.trim_end_matches(['\0', '\n']);
        let mut command = Command::new("cat");
        command.arg("/proc/self/attr/current");
        set_exec_label_for_command(&mut command, label).unwrap();

        for _ in 0..2 {
            command = std::thread::spawn(move || {
                let pending = fs::read("/proc/thread-self/attr/exec").unwrap();
                let output = command.output().unwrap();
                assert!(output.status.success());
                assert_eq!(fs::read("/proc/thread-self/attr/exec").unwrap(), pending);
                let current = String::from_utf8(output.stdout).unwrap();
                let parent = fs::read_to_string("/proc/thread-self/attr/current").unwrap();
                assert_eq!(
                    current.trim_end_matches(['\0', '\n']),
                    parent.trim_end_matches(['\0', '\n'])
                );
                command
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn command_aborts_on_label_failure() {
        if !is_selinux_enabled() {
            return;
        }
        let pending = fs::read("/proc/thread-self/attr/exec").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("executed");
        let mut command = Command::new("touch");
        command.arg(&marker);
        set_exec_label_for_command(&mut command, "invalid-selinux-context").unwrap();
        assert!(command.spawn().is_err());
        assert!(!marker.exists());
        assert_eq!(fs::read("/proc/thread-self/attr/exec").unwrap(), pending);
    }

    #[test]
    fn test_context_level() {
        assert_eq!(
            context_level("system_u:system_r:container_kvm_t:s0:c1,c2"),
            Some("s0:c1,c2")
        );
        assert_eq!(
            context_level("system_u:system_r:spc_t:s0-s0:c0.c1023"),
            Some("s0-s0:c0.c1023")
        );
        assert_eq!(context_level("system_u:system_r:container_kvm_t"), None);
        assert_eq!(context_level("system_u:system_r:container_kvm_t:"), None);
    }

    #[test]
    fn test_context_with_level() {
        assert_eq!(
            context_with_level("system_u:object_r:container_var_run_t:s0", "s0:c1,c2").unwrap(),
            "system_u:object_r:container_var_run_t:s0:c1,c2"
        );
        assert_eq!(
            context_with_level("system_u:object_r:container_var_run_t:s0:c3,c4", "s0:c1,c2")
                .unwrap(),
            "system_u:object_r:container_var_run_t:s0:c1,c2"
        );
        assert!(context_with_level("system_u:object_r", "s0:c1,c2").is_err());
        assert!(context_with_level("", "s0:c1,c2").is_err());
    }
}
