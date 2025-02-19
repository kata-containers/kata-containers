// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Utilities and helpers to execute mount operations on Linux systems.
//!
//! These utilities and helpers are specially designed and implemented to support container runtimes
//! on Linux systems, so they may not be generic enough.
//!
//! # Quotation from [mount(2)](https://man7.org/linux/man-pages/man2/mount.2.html)
//!
//! A call to mount() performs one of a number of general types of operation, depending on the bits
//! specified in mountflags. The choice of which operation to perform is determined by testing the
//! bits set in mountflags, with the tests being conducted in the order listed here:
//! - Remount an existing mount: mountflags includes MS_REMOUNT.
//! - Create a bind mount: mountflags includes MS_BIND.
//! - Change the propagation type of an existing mount: mountflags includes one of MS_SHARED,
//!   MS_PRIVATE, MS_SLAVE, or MS_UNBINDABLE.
//! - Move an existing mount to a new location: mountflags includes MS_MOVE.
//! - Create a new mount: mountflags includes none of the above flags.
//!
//! Since Linux 2.6.26, the MS_REMOUNT flag can be used with MS_BIND to modify only the
//! per-mount-point flags. This is particularly useful for setting or clearing the "read-only"
//! flag on a mount without changing the underlying filesystem. Specifying mountflags as:
//!            MS_REMOUNT | MS_BIND | MS_RDONLY
//! will make access through this mountpoint read-only, without affecting other mounts.
//!
//! # Safety
//!
//! Mount related operations are sensitive to security flaws, especially when dealing with symlinks.
//! There are several CVEs related to file path handling, for example
//! [CVE-2021-30465](https://github.com/opencontainers/runc/security/advisories/GHSA-c3xm-pvg7-gh7r).
//!
//! So some design rules are adopted here:
//! - all mount variants (`bind_remount_read_only()`, `bind_mount()`, `Mounter::mount()`) assume
//!   that all received paths are safe.
//! - the caller must ensure safe version of `PathBuf` are passed to mount variants.
//! - `create_mount_destination()` may be used to generated safe `PathBuf` for mount destinations.
//! - the `safe_path` crate should be used to generate safe `PathBuf` for general cases.

use std::fmt::Debug;
use std::fs;
use std::io::{self, BufRead};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use lazy_static::lazy_static;
use nix::mount::{mount, MntFlags, MsFlags};
use nix::{unistd, NixPath};
use oci_spec::runtime as oci;

use crate::fs::is_symlink;
use crate::sl;

/// Default permission for directories created for mountpoint.
const MOUNT_DIR_PERM: u32 = 0o755;
const MOUNT_FILE_PERM: u32 = 0o644;

pub const PROC_MOUNTS_FILE: &str = "/proc/mounts";
const PROC_FIELDS_PER_LINE: usize = 6;
const PROC_DEVICE_INDEX: usize = 0;
const PROC_PATH_INDEX: usize = 1;
const PROC_TYPE_INDEX: usize = 2;

lazy_static! {
    static ref MAX_MOUNT_PARAM_SIZE: usize =
        if let Ok(Some(v)) = unistd::sysconf(unistd::SysconfVar::PAGE_SIZE) {
            v as usize
        } else {
            panic!("cannot get PAGE_SIZE by sysconf()");
        };

// Propagation flags for mounting container volumes.
    static ref PROPAGATION_FLAGS: MsFlags =
        MsFlags::MS_SHARED | MsFlags::MS_PRIVATE | MsFlags::MS_SLAVE | MsFlags::MS_UNBINDABLE;

}

/// Errors related to filesystem mount operations.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Can not bind mount {0} to {1}: {2}")]
    BindMount(PathBuf, PathBuf, nix::Error),
    #[error("Failure injection: {0}")]
    FailureInject(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Invalid mountpoint entry (expected {0} fields, got {1}) fields: {2}")]
    InvalidMountEntry(usize, usize, String),
    #[error("Invalid mount option: {0}")]
    InvalidMountOption(String),
    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),
    #[error("Failure in waiting for thread: {0}")]
    Join(String),
    #[error("Can not mount {0} to {1}: {2}")]
    Mount(PathBuf, PathBuf, nix::Error),
    #[error("Mount option exceeds 4K size")]
    MountOptionTooBig,
    #[error("Path for mountpoint is null")]
    NullMountPointPath,
    #[error("Invalid Propagation type Flag")]
    InvalidPgMountFlag,
    #[error("Faile to open file {0} by path, {1}")]
    OpenByPath(PathBuf, io::Error),
    #[error("Can not read metadata of {0}, {1}")]
    ReadMetadata(PathBuf, io::Error),
    #[error("Can not remount {0}: {1}")]
    Remount(PathBuf, nix::Error),
    #[error("Can not find mountpoint for {0}")]
    NoMountEntry(String),
    #[error("Can not umount {0}, {1}")]
    Umount(PathBuf, io::Error),
}

/// A specialized version of `std::result::Result` for mount operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Information of mount record from `/proc/mounts`.
pub struct LinuxMountInfo {
    /// Source of mount, first field of records from `/proc/mounts`.
    pub device: String,
    /// Destination of mount, second field of records from `/proc/mounts`.
    pub path: String,
    /// Filesystem type of mount, third field of records from `/proc/mounts`.
    pub fs_type: String,
}

/// Get the device and file system type of a mount point by parsing `/proc/mounts`.
pub fn get_linux_mount_info(mount_point: &str) -> Result<LinuxMountInfo> {
    let mount_file = fs::File::open(PROC_MOUNTS_FILE)?;
    let reader = io::BufReader::new(mount_file);

    for line in reader.lines() {
        let mount = line?;
        let fields: Vec<&str> = mount.split(' ').collect();

        if fields.len() != PROC_FIELDS_PER_LINE {
            return Err(Error::InvalidMountEntry(
                PROC_FIELDS_PER_LINE,
                fields.len(),
                mount,
            ));
        }

        if mount_point == fields[PROC_PATH_INDEX] {
            return Ok(LinuxMountInfo {
                device: fields[PROC_DEVICE_INDEX].to_string(),
                path: fields[PROC_PATH_INDEX].to_string(),
                fs_type: fields[PROC_TYPE_INDEX].to_string(),
            });
        }
    }

    Err(Error::NoMountEntry(mount_point.to_owned()))
}

/// Recursively create destination for a mount.
///
/// For a normal mount, the destination will always be a directory. For bind mount, the destination
/// must be a directory if the source is a directory, otherwise the destination must be a normal
/// file. If directories are created, their permissions are initialized to MountPerm.
///
/// # Safety
///
/// Every container has a root filesystems `rootfs`. When creating bind mounts for a container,
/// the destination should always be within the container's `rootfs`. Otherwise it's a serious
/// security flaw for container to read/override host side filesystem contents. Please refer to
/// following CVEs for example:
/// - [CVE-2021-30465](https://github.com/opencontainers/runc/security/advisories/GHSA-c3xm-pvg7-gh7r)
///
/// To ensure security, the `create_mount_destination()` function takes an extra parameter `root`,
/// which is used to ensure that `dst` is within the specified directory. And a safe version of
/// `PathBuf` is returned to avoid TOCTTOU type of flaws.
pub fn create_mount_destination<S: AsRef<Path>, D: AsRef<Path>, R: AsRef<Path>>(
    src: S,
    dst: D,
    _root: R,
    fs_type: &str,
) -> Result<impl AsRef<Path> + Debug> {
    // TODO: https://github.com/kata-containers/kata-containers/issues/3473
    let dst = dst.as_ref();
    let parent = dst
        .parent()
        .ok_or_else(|| Error::InvalidPath(dst.to_path_buf()))?;
    let mut builder = fs::DirBuilder::new();
    builder
        .mode(MOUNT_DIR_PERM)
        .recursive(true)
        .create(parent)?;

    if fs_type == "bind" {
        // The source and destination for bind mounting must be the same type: file or directory.
        if !src.as_ref().is_dir() {
            fs::OpenOptions::new()
                .mode(MOUNT_FILE_PERM)
                .write(true)
                .create(true)
                .open(dst)?;
            return Ok(dst.to_path_buf());
        }
    }

    if let Err(e) = builder.create(dst) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(e.into());
        }
    }
    if !dst.is_dir() {
        Err(Error::InvalidPath(dst.to_path_buf()))
    } else {
        Ok(dst.to_path_buf())
    }
}

/// Remount a bind mount
///
/// # Safety
/// Caller needs to ensure safety of the `dst` to avoid possible file path based attacks.
pub fn bind_remount<P: AsRef<Path>>(dst: P, readonly: bool) -> Result<()> {
    let dst = dst.as_ref();
    if dst.is_empty() {
        return Err(Error::NullMountPointPath);
    }
    let dst = dst
        .canonicalize()
        .map_err(|_e| Error::InvalidPath(dst.to_path_buf()))?;

    do_rebind_mount(dst, readonly, MsFlags::empty())
}

/// Bind mount `src` to `dst` with a custom propagation type, optionally in readonly mode if
/// `readonly` is true.
///
/// Propagation type: MsFlags::MS_SHARED or MsFlags::MS_SLAVE
/// MsFlags::MS_SHARED is used to bind mount the sandbox path to enable `exec` (in case of FC
/// jailer).
/// MsFlags::MS_SLAVE is used on all other cases.
///
/// # Safety
/// Caller needs to ensure:
/// - `src` exists.
/// - `dst` exists, and is suitable as destination for bind mount.
/// - `dst` is free of file path based attacks.
pub fn bind_mount_unchecked<S: AsRef<Path>, D: AsRef<Path>>(
    src: S,
    dst: D,
    readonly: bool,
    pgflag: MsFlags,
) -> Result<()> {
    fail::fail_point!("bind_mount", |_| {
        Err(Error::FailureInject(
            "Bind mount fail point injection".to_string(),
        ))
    });

    let src = src.as_ref();
    let dst = dst.as_ref();
    if src.is_empty() {
        return Err(Error::NullMountPointPath);
    }
    if dst.is_empty() {
        return Err(Error::NullMountPointPath);
    }
    let abs_src = src
        .canonicalize()
        .map_err(|_e| Error::InvalidPath(src.to_path_buf()))?;

    create_mount_destination(src, dst, "/", "bind")?;
    // Bind mount `src` to `dst`.
    mount(
        Some(&abs_src),
        dst,
        Some("bind"),
        MsFlags::MS_BIND,
        Some(""),
    )
    .map_err(|e| Error::BindMount(abs_src, dst.to_path_buf(), e))?;

    // Change into the chosen propagation mode.
    if !(pgflag == MsFlags::MS_SHARED || pgflag == MsFlags::MS_SLAVE) {
        return Err(Error::InvalidPgMountFlag);
    }
    mount(Some(""), dst, Some(""), pgflag, Some(""))
        .map_err(|e| Error::Mount(PathBuf::new(), dst.to_path_buf(), e))?;

    // Optionally rebind into readonly mode.
    if readonly {
        do_rebind_mount(dst, readonly, MsFlags::empty())?;
    }

    Ok(())
}

/// Trait to mount a `kata_types::mount::Mount`.
pub trait Mounter {
    /// Mount to the specified `target`.
    ///
    /// # Safety
    /// Caller needs to ensure:
    /// - `target` exists, and is suitable as destination for mount.
    /// - `target` is free of file path based attacks.
    fn mount<P: AsRef<Path>>(&self, target: P) -> Result<()>;
}

impl Mounter for kata_types::mount::Mount {
    // This function is modelled after
    // [Mount::Mount()](https://github.com/containerd/containerd/blob/main/mount/mount_linux.go)
    // from [Containerd](https://github.com/containerd/containerd) project.
    fn mount<P: AsRef<Path>>(&self, target: P) -> Result<()> {
        fail::fail_point!("Mount::mount", |_| {
            Err(Error::FailureInject(
                "Mount::mount() fail point injection".to_string(),
            ))
        });

        let target = target.as_ref().to_path_buf();
        let (chdir, (flags, data)) =
            // Follow the same algorithm as Containerd: reserve 512 bytes to avoid hitting one page
            // limit of mounting argument buffer.
            if self.fs_type == "overlay" && self.option_size() >= *MAX_MOUNT_PARAM_SIZE - 512 {
                info!(
                    sl!(),
                    "overlay mount option too long, maybe failed to mount"
                );
                let (chdir, options) = compact_lowerdir_option(&self.options);
                (chdir, parse_mount_options(&options)?)
            } else {
                (None, parse_mount_options(&self.options)?)
            };

        // Ensure propagation type change flags aren't included in other calls.
        let o_flag = flags & (!*PROPAGATION_FLAGS);

        // - Normal mount without MS_REMOUNT flag
        // - In the case of remounting with changed data (data != ""), need to call mount
        if (flags & MsFlags::MS_REMOUNT) == MsFlags::empty() || !data.is_empty() {
            mount_at(
                chdir,
                &self.source,
                target.clone(),
                &self.fs_type,
                o_flag,
                &data,
            )?;
        }

        // Change mount propagation type.
        if (flags & *PROPAGATION_FLAGS) != MsFlags::empty() {
            let propagation_flag = *PROPAGATION_FLAGS | MsFlags::MS_REC | MsFlags::MS_SILENT;
            debug!(
                sl!(),
                "Change mount propagation flags to: 0x{:x}",
                propagation_flag.bits()
            );
            mount(
                Some(""),
                &target,
                Some(""),
                flags & propagation_flag,
                Some(""),
            )
            .map_err(|e| Error::Mount(PathBuf::new(), target.clone(), e))?;
        }

        // Bind mount readonly.
        let bro_flag = MsFlags::MS_BIND | MsFlags::MS_RDONLY;
        if (o_flag & bro_flag) == bro_flag {
            do_rebind_mount(target, true, o_flag)?;
        }

        Ok(())
    }
}

#[inline]
fn do_rebind_mount<P: AsRef<Path>>(path: P, readonly: bool, flags: MsFlags) -> Result<()> {
    mount(
        Some(""),
        path.as_ref(),
        Some(""),
        if readonly {
            flags | MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY
        } else {
            flags | MsFlags::MS_BIND | MsFlags::MS_REMOUNT
        },
        Some(""),
    )
    .map_err(|e| Error::Remount(path.as_ref().to_path_buf(), e))
}

/// Take fstab style mount options and parses them for use with a standard mount() syscall.
pub fn parse_mount_options<T: AsRef<str>>(options: &[T]) -> Result<(MsFlags, String)> {
    let mut flags: MsFlags = MsFlags::empty();
    let mut data: Vec<String> = Vec::new();

    for opt in options.iter() {
        if opt.as_ref() == "loop" {
            return Err(Error::InvalidMountOption("loop".to_string()));
        } else if let Some(v) = parse_mount_flags(flags, opt.as_ref()) {
            flags = v;
        } else {
            data.push(opt.as_ref().to_string());
        }
    }

    let data = data.join(",");
    if data.len() > *MAX_MOUNT_PARAM_SIZE {
        return Err(Error::MountOptionTooBig);
    }

    Ok((flags, data))
}

fn parse_mount_flags(mut flags: MsFlags, flag_str: &str) -> Option<MsFlags> {
    // Following mount options are applicable to fstab only.
    // - _netdev: The filesystem resides on a device that requires network access (used to prevent
    //   the system from attempting to mount these filesystems until the network has been enabled
    //   on the system).
    // - auto: Can be mounted with the -a option.
    // - group: Allow an ordinary user to mount the filesystem if one of that user’s groups matches
    //   the group of the device. This option implies the options nosuid and nodev (unless
    //    overridden by subsequent options, as in the option line group,dev,suid).
    // - noauto: Can only be mounted explicitly (i.e., the -a option will not cause the filesystem
    //   to be mounted).
    // - nofail: Do not report errors for this device if it does not exist.
    // - owner: Allow an ordinary user to mount the filesystem if that user is the owner of the
    //   device. This option implies the options nosuid and nodev (unless overridden by subsequent
    //   options, as in the option line owner,dev,suid).
    // - user: Allow an ordinary user to mount the filesystem. The name of the mounting user is
    //   written to the mtab file (or to the private libmount file in /run/mount on systems without
    //   a regular mtab) so that this same user can unmount the filesystem again. This option
    //   implies the options noexec, nosuid, and nodev (unless overridden by subsequent options,
    //   as in the option line user,exec,dev,suid).
    // - nouser: Forbid an ordinary user to mount the filesystem. This is the default; it does not
    //   imply any other options.
    // - users: Allow any user to mount and to unmount the filesystem, even when some other ordinary
    //   user mounted it. This option implies the options noexec, nosuid, and nodev (unless
    //   overridden by subsequent options, as in the option line users,exec,dev,suid).
    match flag_str {
        // Clear flags
        "defaults" => {}
        "async" => flags &= !MsFlags::MS_SYNCHRONOUS,
        "atime" => flags &= !MsFlags::MS_NOATIME,
        "dev" => flags &= !MsFlags::MS_NODEV,
        "diratime" => flags &= !MsFlags::MS_NODIRATIME,
        "exec" => flags &= !MsFlags::MS_NOEXEC,
        "loud" => flags &= !MsFlags::MS_SILENT,
        "noiversion" => flags &= !MsFlags::MS_I_VERSION,
        "nomand" => flags &= !MsFlags::MS_MANDLOCK,
        "norelatime" => flags &= !MsFlags::MS_RELATIME,
        "nostrictatime" => flags &= !MsFlags::MS_STRICTATIME,
        "rw" => flags &= !MsFlags::MS_RDONLY,
        "suid" => flags &= !MsFlags::MS_NOSUID,
        // Set flags
        "bind" => flags |= MsFlags::MS_BIND,
        "dirsync" => flags |= MsFlags::MS_DIRSYNC,
        "iversion" => flags |= MsFlags::MS_I_VERSION,
        "mand" => flags |= MsFlags::MS_MANDLOCK,
        "noatime" => flags |= MsFlags::MS_NOATIME,
        "nodev" => flags |= MsFlags::MS_NODEV,
        "nodiratime" => flags |= MsFlags::MS_NODIRATIME,
        "noexec" => flags |= MsFlags::MS_NOEXEC,
        "nosuid" => flags |= MsFlags::MS_NOSUID,
        "rbind" => flags |= MsFlags::MS_BIND | MsFlags::MS_REC,
        "unbindable" => flags |= MsFlags::MS_UNBINDABLE,
        "runbindable" => flags |= MsFlags::MS_UNBINDABLE | MsFlags::MS_REC,
        "private" => flags |= MsFlags::MS_PRIVATE,
        "rprivate" => flags |= MsFlags::MS_PRIVATE | MsFlags::MS_REC,
        "shared" => flags |= MsFlags::MS_SHARED,
        "rshared" => flags |= MsFlags::MS_SHARED | MsFlags::MS_REC,
        "slave" => flags |= MsFlags::MS_SLAVE,
        "rslave" => flags |= MsFlags::MS_SLAVE | MsFlags::MS_REC,
        "relatime" => flags |= MsFlags::MS_RELATIME,
        "remount" => flags |= MsFlags::MS_REMOUNT,
        "ro" => flags |= MsFlags::MS_RDONLY,
        "silent" => flags |= MsFlags::MS_SILENT,
        "strictatime" => flags |= MsFlags::MS_STRICTATIME,
        "sync" => flags |= MsFlags::MS_SYNCHRONOUS,
        flag_str => {
            warn!(sl!(), "BUG: unknown mount flag: {:?}", flag_str);
            return None;
        }
    }
    Some(flags)
}

// Do mount, optionally change current working directory if `chdir` is not empty.
fn mount_at<P: AsRef<Path>>(
    chdir: Option<PathBuf>,
    source: P,
    target: PathBuf,
    fstype: &str,
    flags: MsFlags,
    data: &str,
) -> Result<()> {
    let chdir = match chdir {
        Some(v) => v,
        None => {
            return mount(
                Some(source.as_ref()),
                &target,
                Some(fstype),
                flags,
                Some(data),
            )
            .map_err(|e| Error::Mount(PathBuf::new(), target, e));
        }
    };

    info!(
        sl!(),
        "mount_at: chdir {}, source {}, target {} , fstype {}, data {}",
        chdir.display(),
        source.as_ref().display(),
        target.display(),
        fstype,
        data
    );

    // TODO: https://github.com/kata-containers/kata-containers/issues/3473
    let o_flags = nix::fcntl::OFlag::O_PATH | nix::fcntl::OFlag::O_CLOEXEC;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(o_flags.bits())
        .open(&chdir)
        .map_err(|e| Error::OpenByPath(chdir.to_path_buf(), e))?;
    match file.metadata() {
        Ok(md) => {
            if !md.is_dir() {
                return Err(Error::InvalidPath(chdir));
            }
        }
        Err(e) => return Err(Error::ReadMetadata(chdir, e)),
    }

    let cwd = unistd::getcwd().map_err(|e| Error::Io(io::Error::from_raw_os_error(e as i32)))?;
    let src = source.as_ref().to_path_buf();
    let tgt = target.clone();
    let ftype = String::from(fstype);
    let d = String::from(data);
    let rx = Arc::new(AtomicBool::new(false));
    let tx = rx.clone();

    // A working thread is spawned to ease error handling.
    let child = std::thread::Builder::new()
        .name("async_mount".to_string())
        .spawn(move || {
            match unistd::fchdir(file.as_raw_fd()) {
                Ok(_) => info!(sl!(), "chdir from {} to {}", cwd.display(), chdir.display()),
                Err(e) => {
                    error!(
                        sl!(),
                        "failed to chdir from {} to {} error {:?}",
                        cwd.display(),
                        chdir.display(),
                        e
                    );
                    return;
                }
            }
            match mount(
                Some(src.as_path()),
                &tgt,
                Some(ftype.as_str()),
                flags,
                Some(d.as_str()),
            ) {
                Ok(_) => tx.store(true, Ordering::Release),
                Err(e) => error!(sl!(), "failed to mount in chdir {}: {}", chdir.display(), e),
            }
            match unistd::chdir(&cwd) {
                Ok(_) => info!(sl!(), "chdir from {} to {}", chdir.display(), cwd.display()),
                Err(e) => {
                    error!(
                        sl!(),
                        "failed to chdir from {} to {} error {:?}",
                        chdir.display(),
                        cwd.display(),
                        e
                    );
                }
            }
        })?;
    child.join().map_err(|e| Error::Join(format!("{:?}", e)))?;

    if !rx.load(Ordering::Acquire) {
        Err(Error::Mount(
            source.as_ref().to_path_buf(),
            target,
            nix::Error::EIO,
        ))
    } else {
        Ok(())
    }
}

/// When the size of mount options is bigger than one page, try to reduce the size by compressing
/// the `lowerdir` option for overlayfs. The assumption is that lower directories for overlayfs
/// often have a common prefix.
fn compact_lowerdir_option(opts: &[String]) -> (Option<PathBuf>, Vec<String>) {
    let mut n_opts = opts.to_vec();
    // No need to compact if there is no overlay or only one lowerdir
    let (idx, lower_opts) = match find_overlay_lowerdirs(opts) {
        None => return (None, n_opts),
        Some(v) => {
            if v.1.len() <= 1 {
                return (None, n_opts);
            }
            v
        }
    };

    let common_dir = match get_longest_common_prefix(&lower_opts) {
        None => return (None, n_opts),
        Some(v) => {
            if v.is_absolute() && v.parent().is_none() {
                return (None, n_opts);
            }
            v
        }
    };
    let common_prefix = match common_dir.as_os_str().to_str() {
        None => return (None, n_opts),
        Some(v) => {
            let mut p = v.to_string();
            p.push('/');
            p
        }
    };

    info!(
        sl!(),
        "compact_lowerdir_option get common prefix: {}",
        common_dir.display()
    );
    let lower: Vec<String> = lower_opts
        .iter()
        .map(|c| c.replace(&common_prefix, ""))
        .collect();
    n_opts[idx] = format!("lowerdir={}", lower.join(":"));

    (Some(common_dir), n_opts)
}

fn find_overlay_lowerdirs(opts: &[String]) -> Option<(usize, Vec<String>)> {
    for (idx, o) in opts.iter().enumerate() {
        if let Some(lower) = o.strip_prefix("lowerdir=") {
            if !lower.is_empty() {
                let c_opts: Vec<String> = lower.split(':').map(|c| c.to_string()).collect();
                return Some((idx, c_opts));
            }
        }
    }

    None
}

fn get_longest_common_prefix(opts: &[String]) -> Option<PathBuf> {
    if opts.is_empty() {
        return None;
    }

    let mut paths = Vec::with_capacity(opts.len());
    for opt in opts.iter() {
        match Path::new(opt).parent() {
            None => return None,
            Some(v) => paths.push(v),
        }
    }

    let mut path = PathBuf::new();
    paths.sort_unstable();
    for (first, last) in paths[0]
        .components()
        .zip(paths[paths.len() - 1].components())
    {
        if first != last {
            break;
        }
        path.push(first);
    }

    Some(path)
}

/// Umount a mountpoint with timeout.
///
/// # Safety
/// Caller needs to ensure safety of the `path` to avoid possible file path based attacks.
pub fn umount_timeout<P: AsRef<Path>>(path: P, timeout: u64) -> Result<()> {
    // Protect from symlink based attacks, please refer to:
    // https://github.com/kata-containers/runtime/issues/2474
    // For Kata specific, we do extra protection for parent directory too.
    let path = path.as_ref();
    let parent = path
        .parent()
        .ok_or_else(|| Error::InvalidPath(path.to_path_buf()))?;
    // TODO: https://github.com/kata-containers/kata-containers/issues/3473
    if is_symlink(path).map_err(|e| Error::ReadMetadata(path.to_owned(), e))?
        || is_symlink(parent).map_err(|e| Error::ReadMetadata(path.to_owned(), e))?
    {
        warn!(
            sl!(),
            "unable to umount {} which is a symbol link",
            path.display()
        );
        return Ok(());
    }

    if timeout == 0 {
        // Lazy unmounting the mountpoint with the MNT_DETACH flag.
        umount2(path, true).map_err(|e| Error::Umount(path.to_owned(), e))?;
        info!(sl!(), "lazy umount for {}", path.display());
    } else {
        let start_time = Instant::now();
        while let Err(e) = umount2(path, false) {
            match e.kind() {
                // The mountpoint has been concurrently unmounted by other threads.
                io::ErrorKind::InvalidInput => break,
                io::ErrorKind::WouldBlock => {
                    let time_now = Instant::now();
                    if time_now.duration_since(start_time).as_millis() > timeout as u128 {
                        warn!(sl!(),
                                  "failed to umount {} in {} ms because of EBUSY, try again with lazy umount",
                                  path.display(),
                                  Instant::now().duration_since(start_time).as_millis());
                        return umount2(path, true).map_err(|e| Error::Umount(path.to_owned(), e));
                    }
                }
                _ => return Err(Error::Umount(path.to_owned(), e)),
            }
        }

        info!(
            sl!(),
            "umount {} in {} ms",
            path.display(),
            Instant::now().duration_since(start_time).as_millis()
        );
    }

    Ok(())
}

/// Umount all filesystems mounted at the `mountpoint`.
///
/// If `mountpoint` is empty or doesn't exist, `umount_all()` is a noop. Otherwise it will try to
/// unmount all filesystems mounted at `mountpoint` repeatedly. For example:
/// - bind mount /dev/sda to /tmp/mnt
/// - bind mount /tmp/b to /tmp/mnt
/// - umount_all("tmp/mnt") will umount both /tmp/b and /dev/sda
///
/// # Safety
/// Caller needs to ensure safety of the `path` to avoid possible file path based attacks.
pub fn umount_all<P: AsRef<Path>>(mountpoint: P, lazy_umount: bool) -> Result<()> {
    if mountpoint.as_ref().is_empty() || !mountpoint.as_ref().exists() {
        return Ok(());
    }

    loop {
        if let Err(e) = umount2(mountpoint.as_ref(), lazy_umount) {
            // EINVAL is returned if the target is not a mount point, indicating that we are
            // done. It can also indicate a few other things (such as invalid flags) which we
            // unfortunately end up squelching here too.
            if e.kind() == io::ErrorKind::InvalidInput {
                break;
            } else {
                return Err(Error::Umount(mountpoint.as_ref().to_path_buf(), e));
            }
        }
    }

    Ok(())
}

// Counterpart of nix::umount2, with support of `UMOUNT_FOLLOW`.
fn umount2<P: AsRef<Path>>(path: P, lazy_umount: bool) -> std::io::Result<()> {
    let mut flags = MntFlags::UMOUNT_NOFOLLOW;
    if lazy_umount {
        flags |= MntFlags::MNT_DETACH;
    }
    nix::mount::umount2(path.as_ref(), flags).map_err(io::Error::from)
}

pub fn get_mount_path(p: &Option<PathBuf>) -> String {
    p.clone().unwrap_or_default().display().to_string()
}

pub fn get_mount_options(options: &Option<Vec<String>>) -> Vec<String> {
    match options {
        Some(o) => o.to_vec(),
        None => vec![],
    }
}

pub fn get_mount_type(m: &oci::Mount) -> String {
    m.typ()
        .clone()
        .map(|typ| {
            if typ.as_str() == "none" {
                if let Some(opts) = m.options() {
                    if opts.iter().any(|opt| opt == "bind" || opt == "rbind") {
                        return "bind".to_string();
                    }
                }
            }
            typ
        })
        .unwrap_or("bind".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_linux_mount_info() {
        let info = get_linux_mount_info("/sys/fs/cgroup").unwrap();

        assert_eq!(&info.device, "tmpfs");
        assert_eq!(&info.fs_type, "tmpfs");
        assert_eq!(&info.path, "/sys/fs/cgroup");

        assert!(matches!(
            get_linux_mount_info(""),
            Err(Error::NoMountEntry(_))
        ));
        assert!(matches!(
            get_linux_mount_info("/sys/fs/cgroup/do_not_exist/____hi"),
            Err(Error::NoMountEntry(_))
        ));
    }

    #[test]
    fn test_create_mount_destination() {
        let tmpdir = tempfile::tempdir().unwrap();
        let src = Path::new("/proc/mounts");
        let mut dst = tmpdir.path().to_owned();
        dst.push("proc");
        dst.push("mounts");
        let dst = create_mount_destination(src, dst.as_path(), tmpdir.path(), "bind").unwrap();
        let abs_dst = dst.as_ref().canonicalize().unwrap();
        assert!(abs_dst.is_file());

        let dst = Path::new("/");
        assert!(matches!(
            create_mount_destination(src, dst, "/", "bind"),
            Err(Error::InvalidPath(_))
        ));

        let src = Path::new("/proc");
        let dst = Path::new("/proc/mounts");
        assert!(matches!(
            create_mount_destination(src, dst, "/", "bind"),
            Err(Error::InvalidPath(_))
        ));
    }

    #[test]
    #[ignore]
    fn test_bind_remount() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir2 = tempfile::tempdir().unwrap();

        assert!(matches!(
            bind_remount(PathBuf::from(""), true),
            Err(Error::NullMountPointPath)
        ));
        assert!(matches!(
            bind_remount(PathBuf::from("../______doesn't____exist____nnn"), true),
            Err(Error::InvalidPath(_))
        ));

        bind_mount_unchecked(tmpdir2.path(), tmpdir.path(), true, MsFlags::MS_SLAVE).unwrap();
        bind_remount(tmpdir.path(), true).unwrap();
        umount_timeout(tmpdir.path().to_str().unwrap(), 0).unwrap();
    }

    #[test]
    #[ignore]
    fn test_bind_mount() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir2 = tempfile::tempdir().unwrap();
        let mut src = tmpdir.path().to_owned();
        src.push("src");
        let mut dst = tmpdir.path().to_owned();
        dst.push("src");

        assert!(matches!(
            bind_mount_unchecked(Path::new(""), Path::new(""), false, MsFlags::MS_SLAVE),
            Err(Error::NullMountPointPath)
        ));
        assert!(matches!(
            bind_mount_unchecked(tmpdir2.path(), Path::new(""), false, MsFlags::MS_SLAVE),
            Err(Error::NullMountPointPath)
        ));
        assert!(matches!(
            bind_mount_unchecked(
                Path::new("/_does_not_exist_/___aahhhh"),
                Path::new("/tmp/_does_not_exist/___bbb"),
                false,
                MsFlags::MS_SLAVE
            ),
            Err(Error::InvalidPath(_))
        ));

        let dst = create_mount_destination(tmpdir2.path(), &dst, tmpdir.path(), "bind").unwrap();
        bind_mount_unchecked(tmpdir2.path(), dst.as_ref(), true, MsFlags::MS_SLAVE).unwrap();
        bind_mount_unchecked(&src, dst.as_ref(), false, MsFlags::MS_SLAVE).unwrap();
        umount_all(dst.as_ref(), false).unwrap();

        let mut src = tmpdir.path().to_owned();
        src.push("file");
        fs::write(&src, "test").unwrap();
        let mut dst = tmpdir.path().to_owned();
        dst.push("file");
        let dst = create_mount_destination(&src, &dst, tmpdir.path(), "bind").unwrap();
        bind_mount_unchecked(&src, dst.as_ref(), false, MsFlags::MS_SLAVE).unwrap();
        assert!(dst.as_ref().is_file());
        umount_timeout(dst.as_ref(), 0).unwrap();
    }

    #[test]
    fn test_compact_overlay_lowerdirs() {
        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
            "lowerdir=/a/b/c/xxxx/1l:/a/b/c/xxxx/2l:/a/b/c/xxxx/3l:/a/b/c/xxxx/4l".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert_eq!(&prefix.unwrap(), Path::new("/a/b/c/xxxx/"));
        assert_eq!(n_options.len(), 3);
        assert_eq!(n_options[2], "lowerdir=1l:2l:3l:4l");

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
            "lowerdir=/a/b/c/xxxx:/a/b/c/xxxx/2l:/a/b/c/xxxx/3l:/a/b/c/xxxx/4l".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert_eq!(&prefix.unwrap(), Path::new("/a/b/c/"));
        assert_eq!(n_options.len(), 3);
        assert_eq!(n_options[2], "lowerdir=xxxx:xxxx/2l:xxxx/3l:xxxx/4l");

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
            "lowerdir=/1l:/2l:/3l:/4l".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert!(prefix.is_none());
        assert_eq!(n_options, options);

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert!(prefix.is_none());
        assert_eq!(n_options, options);

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "lowerdir=".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert!(prefix.is_none());
        assert_eq!(n_options, options);
    }

    #[test]
    fn test_find_overlay_lowerdirs() {
        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
            "lowerdir=/a/b/c/xxxx/1l:/a/b/c/xxxx/2l:/a/b/c/xxxx/3l:/a/b/c/xxxx/4l".to_string(),
        ];
        let lower_expect = vec![
            "/a/b/c/xxxx/1l".to_string(),
            "/a/b/c/xxxx/2l".to_string(),
            "/a/b/c/xxxx/3l".to_string(),
            "/a/b/c/xxxx/4l".to_string(),
        ];

        let (idx, lower) = find_overlay_lowerdirs(&options).unwrap();
        assert_eq!(idx, 2);
        assert_eq!(lower, lower_expect);

        let common_prefix = get_longest_common_prefix(&lower).unwrap();
        assert_eq!(Path::new("/a/b/c/xxxx/"), &common_prefix);

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        let v = find_overlay_lowerdirs(&options);
        assert!(v.is_none());

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "lowerdir=".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        find_overlay_lowerdirs(&options);
        assert!(v.is_none());
    }

    #[test]
    fn test_get_common_prefix() {
        let lower1 = vec![
            "/a/b/c/xxxx/1l/fs".to_string(),
            "/a/b/c/////xxxx/11l/fs".to_string(),
            "/a/b/c/././xxxx/13l/fs".to_string(),
            "/a/b/c/.////xxxx/14l/fs".to_string(),
        ];
        let common_prefix = get_longest_common_prefix(&lower1).unwrap();
        assert_eq!(Path::new("/a/b/c/xxxx/"), &common_prefix);

        let lower2 = vec![
            "/fs".to_string(),
            "/s".to_string(),
            "/sa".to_string(),
            "/s".to_string(),
        ];
        let common_prefix = get_longest_common_prefix(&lower2).unwrap();
        assert_eq!(Path::new("/"), &common_prefix);

        let lower3 = vec!["".to_string(), "".to_string()];
        let common_prefix = get_longest_common_prefix(&lower3);
        assert!(common_prefix.is_none());

        let lower = vec!["/".to_string(), "/".to_string()];
        let common_prefix = get_longest_common_prefix(&lower);
        assert!(common_prefix.is_none());

        let lower = vec![
            "/a/b/c".to_string(),
            "/a/b/c/d".to_string(),
            "/a/b///c".to_string(),
        ];
        let common_prefix = get_longest_common_prefix(&lower).unwrap();
        assert_eq!(Path::new("/a/b"), &common_prefix);

        let lower = vec!["a/b/c/e".to_string(), "a/b/c/d".to_string()];
        let common_prefix = get_longest_common_prefix(&lower).unwrap();
        assert_eq!(Path::new("a/b/c"), &common_prefix);

        let lower = vec!["a/b/c".to_string(), "a/b/c/d".to_string()];
        let common_prefix = get_longest_common_prefix(&lower).unwrap();
        assert_eq!(Path::new("a/b"), &common_prefix);

        let lower = vec!["/test".to_string()];
        let common_prefix = get_longest_common_prefix(&lower).unwrap();
        assert_eq!(Path::new("/"), &common_prefix);

        let lower = vec![];
        let common_prefix = get_longest_common_prefix(&lower);
        assert!(&common_prefix.is_none());
    }

    #[test]
    fn test_parse_mount_options() {
        let options: Vec<&str> = vec![];
        let (flags, data) = parse_mount_options(&options).unwrap();
        assert!(flags.is_empty());
        assert!(data.is_empty());

        let mut options = vec![
            "dev".to_string(),
            "ro".to_string(),
            "defaults".to_string(),
            "data-option".to_string(),
        ];
        let (flags, data) = parse_mount_options(&options).unwrap();
        assert_eq!(flags, MsFlags::MS_RDONLY);
        assert_eq!(&data, "data-option");

        options.push("loop".to_string());
        assert!(parse_mount_options(&options).is_err());

        let idx = options.len() - 1;
        options[idx] = " ".repeat(4097);
        assert!(parse_mount_options(&options).is_err());
    }

    #[test]
    #[ignore]
    fn test_mount_at() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().to_path_buf();
        mount_at(
            Some(path.clone()),
            "/___does_not_exist____a___",
            PathBuf::from("/tmp/etc/host.conf"),
            "",
            MsFlags::empty(),
            "",
        )
        .unwrap_err();

        mount_at(
            Some(PathBuf::from("/___does_not_exist____a___")),
            "/etc/host.conf",
            PathBuf::from("/tmp/etc/host.conf"),
            "",
            MsFlags::empty(),
            "",
        )
        .unwrap_err();

        let src = path.join("src");
        fs::write(src, "test").unwrap();
        let dst = path.join("dst");
        fs::write(&dst, "test1").unwrap();
        mount_at(
            Some(path),
            "src",
            PathBuf::from("dst"),
            "bind",
            MsFlags::MS_BIND,
            "",
        )
        .unwrap();
        let content = fs::read_to_string(&dst).unwrap();
        assert_eq!(&content, "test");
    }
}
