// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use lazy_static::lazy_static;
use nix::mount::{mount, MsFlags};

use kata_types::mount::Mount;

use crate::fs::get_bundle_path;
use crate::sl;

const MOUNT_PERM: u32 = 0o755;

const PROC_MOUNTS_FILE: &str = "/proc/mounts";
const PROC_FIELDS_PER_LINE: usize = 6;
const PROC_DEVICE_INDEX: usize = 0;
const PROC_PATH_INDEX: usize = 1;
const PROC_TYPE_INDEX: usize = 2;

lazy_static! {
    static ref PAGESIZE: usize =
        if let Ok(Some(v)) = nix::unistd::sysconf(nix::unistd::SysconfVar::PAGE_SIZE) {
            v as usize
        } else {
            4096
        };
    static ref MOUNT_OPTION_CLEAR_FLAGS: HashMap<String, MsFlags> = {
        let mut clear_flags: HashMap<String, MsFlags> = HashMap::new();
        clear_flags.insert("async".to_string(), MsFlags::MS_STRICTATIME);
        clear_flags.insert("atime".to_string(), MsFlags::MS_NOATIME);
        clear_flags.insert("dev".to_string(), MsFlags::MS_NODEV);
        clear_flags.insert("diratime".to_string(), MsFlags::MS_NODIRATIME);
        clear_flags.insert("exec".to_string(), MsFlags::MS_NOEXEC);
        clear_flags.insert("nomand".to_string(), MsFlags::MS_MANDLOCK);
        clear_flags.insert("rw".to_string(), MsFlags::MS_RDONLY);
        clear_flags.insert("norelatime".to_string(), MsFlags::MS_RELATIME);
        clear_flags.insert("suid".to_string(), MsFlags::MS_NOSUID);
        clear_flags
    };
    static ref MOUNT_OPTION_KEEP_FLAGS: HashMap<String, MsFlags> = {
        let mut keep_flags: HashMap<String, MsFlags> = HashMap::new();
        keep_flags.insert("nosuid".to_string(), MsFlags::MS_NOSUID);
        keep_flags.insert("rbind".to_string(), MsFlags::MS_BIND | MsFlags::MS_REC);
        keep_flags.insert("relatime".to_string(), MsFlags::MS_RELATIME);
        keep_flags.insert("remount".to_string(), MsFlags::MS_REMOUNT);
        keep_flags.insert("ro".to_string(), MsFlags::MS_RDONLY);
        keep_flags.insert("strictatime".to_string(), MsFlags::MS_STRICTATIME);
        keep_flags.insert("sync".to_string(), MsFlags::MS_SYNCHRONOUS);
        keep_flags.insert("bind".to_string(), MsFlags::MS_BIND);
        keep_flags.insert("dirsync".to_string(), MsFlags::MS_DIRSYNC);
        keep_flags.insert("mand".to_string(), MsFlags::MS_MANDLOCK);
        keep_flags.insert("noatime".to_string(), MsFlags::MS_NOATIME);
        keep_flags.insert("nodev".to_string(), MsFlags::MS_NODEV);
        keep_flags.insert("nodiratime".to_string(), MsFlags::MS_NODIRATIME);
        keep_flags.insert("noexec".to_string(), MsFlags::MS_NOEXEC);
        keep_flags
    };
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Can not bind mount {0} to {1}: {2}")]
    BindMount(String, String, nix::Error),
    #[error("Mount point can not be empty")]
    EmptyMountPoint,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Invalid mountpoint entry (expected {0} fields, got {1}) fields: {2}")]
    InvalidMountEntry(usize, usize, String),
    #[error("Invalid mount option: {0}")]
    InvalidMountOption(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Failure in waiting for thread: {0}")]
    Join(String),
    #[error("Can not mount {0} to {1}: {2}")]
    Mount(String, String, nix::Error),
    #[error("Mount option exceeds 4K size")]
    MountOptionTooBig,
    #[error("Can not read metadata of {0}, {1}")]
    ReadMetadata(String, io::Error),
    #[error("Can not find mountpoint for {0}")]
    NoMountEntry(String),
    #[error("Can not umount {0}, {1}")]
    Umount(String, nix::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

struct MountOption {
    flag: MsFlags,
    data: String,
}

/// Get the device and file system type of a mount point by parsing `/proc/mounts`.
pub fn get_device_path_and_fs_type(mount_point: &str) -> Result<(String, String)> {
    if mount_point.is_empty() {
        return Err(Error::EmptyMountPoint);
    }

    let mount_file = fs::File::open(PROC_MOUNTS_FILE)?;
    let lines = io::BufReader::new(mount_file).lines();

    for mount in lines.flatten() {
        let fields: Vec<&str> = mount.split(' ').collect();

        if fields.len() != PROC_FIELDS_PER_LINE {
            return Err(Error::InvalidMountEntry(
                PROC_FIELDS_PER_LINE,
                fields.len(),
                mount,
            ));
        }

        if mount_point == fields[PROC_PATH_INDEX] {
            return Ok((
                String::from(fields[PROC_DEVICE_INDEX]),
                String::from(fields[PROC_TYPE_INDEX]),
            ));
        }
    }

    Err(Error::NoMountEntry(mount_point.to_owned()))
}

/// Remount a bind mountpoint in readonly mode.
pub fn bind_remount_read_only<P: AsRef<Path>>(dst: P) -> Result<()> {
    let dst = dst.as_ref();
    if dst.as_os_str().is_empty() {
        return Err(Error::EmptyMountPoint);
    }
    let dst = dst
        .canonicalize()
        .map_err(|_e| Error::InvalidPath(dst.to_string_lossy().to_string()))?;

    mount(
        Some(""),
        &dst,
        Some("bind"),
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
        Some(""),
    )
    .map_err(|e| Error::BindMount("".to_string(), dst.to_string_lossy().to_string(), e))
}

/// Bind mount `src` to `dst` in slave mode.
pub fn bind_mount<S: AsRef<Path>, D: AsRef<Path>>(src: S, dst: D, read_only: bool) -> Result<()> {
    fail::fail_point!("bind_mount", |_| {
        Err(anyhow!("Bind mount fail point injection"))
    });

    let src = src.as_ref();
    let dst = dst.as_ref();
    if src.as_os_str().is_empty() {
        return Err(Error::EmptyMountPoint);
    }
    if dst.as_os_str().is_empty() {
        return Err(Error::EmptyMountPoint);
    }
    let abs_src = src
        .canonicalize()
        .map_err(|_e| Error::InvalidPath(src.to_string_lossy().to_string()))?;

    ensure_destination_exists(abs_src.as_path(), dst, "bind")?;

    // Bind mount source to target by MS_BIND flag.
    // Note: when the MS_BIND is specified, the remaining bits (other than MS_REC, described below)
    // in the mountflags argument are also ignored. However, there is a special case for remounting
    // as read-only. See comments below.
    mount(
        Some(&abs_src),
        dst,
        Some("bind"),
        MsFlags::MS_BIND,
        Some(""),
    )
    .map_err(|e| {
        Error::BindMount(
            abs_src.to_string_lossy().to_string(),
            dst.to_string_lossy().to_string(),
            e,
        )
    })?;

    mount(Some("none"), dst, Some(""), MsFlags::MS_SLAVE, Some(""))
        .map_err(|e| Error::Mount("".to_string(), dst.to_string_lossy().to_string(), e))?;

    // Since Linux 2.6.26, the MS_REMOUNT flag can be used with MS_BIND to modify only the
    // per-mount-point flags. This is particularly useful for setting or clearing the "read-only"
    // flag on a mount without changing the underlying filesystem. Specifying mountflags as:
    //            MS_REMOUNT | MS_BIND | MS_RDONLY
    // will make access through this mountpoint read-only, without affecting other mounts.
    if read_only {
        mount(
            Some(""),
            dst,
            Some("bind"),
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            Some(""),
        )
        .map_err(|e| Error::BindMount("".to_string(), dst.to_string_lossy().to_string(), e))?;
    }

    Ok(())
}

// Recursively create directories for a mount destination.
//
// If directories are created, their permissions are initialized to MountPerm.
pub fn ensure_destination_exists<S: AsRef<Path>, D: AsRef<Path>>(
    src: S,
    dst: D,
    fs_type: &str,
) -> Result<()> {
    let dst = dst.as_ref();
    let parent = dst
        .parent()
        .ok_or_else(|| Error::InvalidPath(dst.to_string_lossy().to_string()))?;
    let mut builder = fs::DirBuilder::new();
    builder.mode(MOUNT_PERM).recursive(true).create(parent)?;

    if fs_type != "bind" {
        if let Err(e) = builder.create(dst) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(e.into());
            }
            let md = fs::metadata(dst)
                .map_err(|e| Error::ReadMetadata(dst.to_string_lossy().to_string(), e))?;
            if !md.is_dir() {
                return Err(Error::InvalidPath(dst.to_string_lossy().to_string()));
            }
        }
    } else {
        // The source and destination for bind mounting must be the same type: file or directory.
        let file_info = fs::metadata(src)?;
        if file_info.is_dir() {
            if let Err(e) = builder.create(dst) {
                if e.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(e.into());
                }
                let md = fs::metadata(dst)
                    .map_err(|e| Error::ReadMetadata(dst.to_string_lossy().to_string(), e))?;
                if !md.is_dir() {
                    return Err(Error::InvalidPath(dst.to_string_lossy().to_string()));
                }
            }
        } else {
            fs::OpenOptions::new()
                .mode(MOUNT_PERM)
                .write(true)
                .create(true)
                .open(dst)?;
        }
    }

    Ok(())
}

/// Create a mount on Linux.
pub fn linux_mount<P: AsRef<Path>>(target: &str, m: &Mount) -> Result<()> {
    fail::fail_point!("linux_mount", |_| {
        Err(anyhow!("linux mount fail point injection"))
    });

    let f_type = &m.fs_type;
    // Follow the same algorithm as Containerd: reserve 512 bytes to avoid hitting one page limit
    // of mounting argument buffer.
    let (chdir, options) = if f_type == "overlay" && option_size(&m.options) >= *PAGESIZE - 512 {
        info!(
            sl!(),
            "overlay mount option too long, maybe failed to mount"
        );
        compact_lowerdir_option(&m.options)
    } else {
        ("".to_string(), m.options.clone())
    };

    let m_opts = parse_mount_option(&options)?;
    let flag = m_opts.flag;
    let propagation_types =
        MsFlags::MS_SHARED | MsFlags::MS_PRIVATE | MsFlags::MS_SLAVE | MsFlags::MS_UNBINDABLE;
    let o_flag = flag & (!propagation_types);
    let data = m_opts.data.as_str();

    // normal mount or remount to change fs-specific options
    if (flag & MsFlags::MS_REMOUNT) == MsFlags::empty() || !data.is_empty() {
        return mount_at(
            chdir.as_str(),
            &m.source,
            target,
            f_type.as_str(),
            o_flag,
            data,
        );
    }

    // remount to change propagation
    if (flag & propagation_types) != MsFlags::empty() {
        let propagation_flag = propagation_types | MsFlags::MS_REC | MsFlags::MS_SILENT;
        return mount(
            Some(""),
            target,
            Some(""),
            flag & propagation_flag,
            Some(""),
        )
        .map_err(|e| Error::Mount("".to_string(), target.to_string(), e));
    }

    let bro_flag = MsFlags::MS_BIND | MsFlags::MS_RDONLY;
    if (o_flag & bro_flag) == bro_flag {
        return mount(
            Some(""),
            target,
            Some(""),
            o_flag & MsFlags::MS_REMOUNT,
            Some(""),
        )
        .map_err(|e| Error::BindMount("".to_string(), target.to_string(), e));
    }

    // the rest of remounts, e.g. remount from ro back to rw
    mount(Some(""), target, Some(""), flag, Some(""))
        .map_err(|e| Error::Mount("".to_string(), target.to_string(), e))
}

fn option_size(opts: &[String]) -> usize {
    opts.iter().map(|v| v.len() + 1).sum()
}

fn parse_mount_option(options: &[String]) -> Result<MountOption> {
    let mut flag: MsFlags = MsFlags::empty();
    let mut data: Vec<String> = Vec::new();

    for opt in options.iter() {
        if let Some(v) = MOUNT_OPTION_CLEAR_FLAGS.get(opt.as_str()) {
            flag &= !*v;
        } else if let Some(v) = MOUNT_OPTION_KEEP_FLAGS.get(opt.as_str()) {
            flag |= *v;
        } else {
            data.push(opt.clone());
        }
    }

    let data = data.join(",");
    if data.len() > *PAGESIZE {
        return Err(Error::MountOptionTooBig);
    }

    Ok(MountOption { flag, data })
}

// Do mount, optionally change current working directory if `chdir` is not empty.
fn mount_at(
    chdir: &str,
    source: &str,
    target: &str,
    fstype: &str,
    flags: MsFlags,
    data: &str,
) -> Result<()> {
    if chdir.is_empty() {
        return mount(Some(source), target, Some(fstype), flags, Some(data))
            .map_err(|e| Error::Mount("".to_string(), target.to_string(), e));
    }

    info!(
        sl!(),
        "mount_at: chdir {}, source {}, target {} , fstype {}, data {}",
        chdir,
        source,
        target,
        fstype,
        data
    );

    match std::fs::metadata(chdir) {
        Ok(f) => {
            if !f.is_dir() {
                return Err(Error::InvalidPath(chdir.to_string()));
            }
        }
        Err(e) => return Err(Error::ReadMetadata(chdir.to_string(), e)),
    }

    // cut off lower layer common prefix, make option less, like lowerdir=/xxxx/61538/fs
    // change to lowerdir=61538/fs, chdir to /xxxx, and mount with changed option
    let src = String::from(source);
    let tgt = String::from(target);
    let ftype = String::from(fstype);
    let d = String::from(data);
    let bundle = get_bundle_path()?;
    let cwd = bundle.to_string_lossy().to_string();
    let chdir_path = String::from(chdir);
    let rx = Arc::new(AtomicBool::new(false));
    let tx = rx.clone();

    // A working thread is spawned to ease error handling.
    let child = std::thread::Builder::new()
        .name("async_mount".to_string())
        .spawn(move || {
            match nix::unistd::chdir(chdir_path.as_str()) {
                Ok(_) => info!(sl!(), "chdir from {} to {}", &cwd, &chdir_path),
                Err(e) => {
                    error!(
                        sl!(),
                        "failed to chdir from {} to {} error {:?}", &cwd, &chdir_path, e
                    );
                    return;
                }
            }
            match mount(
                Some(src.as_str()),
                tgt.as_str(),
                Some(ftype.as_str()),
                flags,
                Some(d.as_str()),
            ) {
                Ok(_) => tx.store(true, Ordering::Release),
                Err(e) => error!(sl!(), "failed to mount in chdir {}: {}", chdir_path, e),
            }
            match nix::unistd::chdir(&bundle) {
                Ok(_) => info!(sl!(), "chdir from {} to {}", &chdir_path, &cwd),
                Err(e) => {
                    error!(
                        sl!(),
                        "failed to chdir from {} to {} error {:?}", &chdir_path, &cwd, e
                    );
                }
            }
        })?;
    child.join().map_err(|e| Error::Join(format!("{:?}", e)))?;

    if !rx.load(Ordering::Acquire) {
        Err(Error::Mount(
            String::from(source),
            String::from(target),
            nix::Error::EIO,
        ))
    } else {
        Ok(())
    }
}

fn compact_lowerdir_option(opts: &[String]) -> (String, Vec<String>) {
    let mut n_opts = opts.to_vec();
    let (idx, lower_opts) = find_overlay_lowerdirs(opts);
    if idx <= 1 {
        return ("".to_string(), n_opts);
    }

    let idx = idx as usize;
    let common_dir = get_longest_common_prefix(&lower_opts);
    if common_dir.is_empty() || common_dir == "/" {
        return ("".to_string(), n_opts);
    }

    info!(
        sl!(),
        "compact_lowerdir_option get common prefix: {}", &common_dir
    );
    let lower: Vec<String> = lower_opts
        .iter()
        .map(|c| c.replace(common_dir.as_str(), ""))
        .collect();
    n_opts[idx as usize] = format!("lowerdir={}", lower.join(":"));
    (common_dir, n_opts)
}

fn find_overlay_lowerdirs(opts: &[String]) -> (isize, Vec<String>) {
    for (idx, o) in opts.iter().enumerate() {
        if let Some(lower) = o.strip_prefix("lowerdir=") {
            if !lower.is_empty() {
                let c_opts: Vec<String> = lower.split(':').map(|c| c.to_string()).collect();
                return (idx as isize, c_opts);
            }
        }
    }

    (-1, Vec::new())
}

// assume lower always with same prefix
fn get_longest_common_prefix(opts: &[String]) -> String {
    //let paths: Vec<Path> = opts.iter().map(|v| Path::new(v)).collect();
    if opts.is_empty() {
        return String::new();
    }
    if opts.len() == 1 {
        return opts[0].clone();
    }

    // FIXME: get prefix
    let strs = opts[0].clone();
    let split_l: Vec<&str> = strs.split('/').collect();

    //let mut prefix = &split_l[..1].join("/");
    let mut prefix = "/".to_string();
    let mut idx: usize = 0;
    for i in 1..split_l.len() {
        let tmp_l = &split_l[..i];
        let tmp_ll = tmp_l.join("/");
        if !tmp_ll.is_empty() {
            prefix = tmp_ll.to_string();
        }

        let contain = || -> bool {
            for opt in opts.iter() {
                if !opt.contains(&prefix) {
                    return false;
                }
            }
            true
        };

        if !contain() {
            break;
        }
        idx = i;
    }

    let tmp_l = &split_l[..idx];
    let tmp_ll = tmp_l.join("/");
    prefix = tmp_ll + "/";

    prefix
}

/// Umount a mountpoint with timeout.
pub fn umount_timeout(path: &str, timeout: u64) -> Result<()> {
    let parent = Path::new(path)
        .parent()
        .ok_or_else(|| Error::InvalidPath(path.to_owned()))?;
    let parent_meta = fs::metadata(&parent).map_err(|e| Error::ReadMetadata(path.to_owned(), e))?;
    let meta = fs::metadata(path).map_err(|e| Error::ReadMetadata(path.to_owned(), e))?;
    if meta.file_type().is_symlink() || parent_meta.file_type().is_symlink() {
        warn!(sl!(), "unable to umount {} which is a symbol link", &path);
        return Ok(());
    }

    if timeout == 0 {
        // Lazy unmounting the mountpoint with the MNT_DETACH flag.
        nix::mount::umount2(path, nix::mount::MntFlags::MNT_DETACH)
            .map_err(|e| Error::Umount(path.to_owned(), e))?;
        info!(sl!(), "lazy umount for {}", path);
    } else {
        let start_time = std::time::Instant::now();
        while let Err(e) = nix::mount::umount(path) {
            match e {
                // The mountpoint has been concurrently unmounted by other threads.
                nix::errno::Errno::EINVAL => break,
                nix::errno::Errno::EBUSY => {
                    let time_now = std::time::Instant::now();
                    if time_now.duration_since(start_time).as_millis() > timeout as u128 {
                        warn!(sl!(),
                                  "failed to umount {} in {} ms because of EBUSY, try again with lazy umount",
                                  path,
                                  std::time::Instant::now().duration_since(start_time).as_millis());
                        return nix::mount::umount2(path, nix::mount::MntFlags::MNT_DETACH)
                            .map_err(|e| Error::Umount(path.to_owned(), e));
                    }
                }
                _ => return Err(Error::Umount(path.to_owned(), e)),
            }
        }

        info!(
            sl!(),
            "umount {} in {} ms",
            path,
            std::time::Instant::now()
                .duration_since(start_time)
                .as_millis()
        );
    }

    Ok(())
}

/// Umount all mounted filesystems at the mountpoint.
pub fn umount_all(mountpoint: &Path) -> Result<()> {
    loop {
        match nix::mount::umount(mountpoint) {
            Err(e) => {
                // EINVAL is returned if the target is not a mount point, indicating that we are
                // done. It can also indicate a few other things (such as invalid flags) which we
                // unfortunately end up squelching here too.
                if e == nix::errno::Errno::EINVAL {
                    break;
                } else {
                    return Err(Error::Umount(mountpoint.to_string_lossy().to_string(), e));
                }
            }
            Ok(()) => (),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_device_path_and_fs_type() {
        let (dev_path, fs_type) = get_device_path_and_fs_type("/sys/fs/cgroup").unwrap();

        assert_eq!(fs_type, "tmpfs");
        assert_eq!(dev_path, "tmpfs");
    }

    #[test]
    fn test_bind_remount_read_only() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir2 = tempfile::tempdir().unwrap();
        tmpdir.path().canonicalize().unwrap();
        bind_mount(tmpdir2.path(), tmpdir.path(), true).unwrap();
        bind_remount_read_only(tmpdir.path()).unwrap();
        umount_timeout(tmpdir.path().to_str().unwrap(), 0).unwrap();

        bind_remount_read_only(&PathBuf::from("")).unwrap_err();
        bind_remount_read_only(&PathBuf::from("../______doesn't____exist____nnn")).unwrap_err();
    }

    #[test]
    fn test_bind_mount() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdir2 = tempfile::tempdir().unwrap();
        let mut src = tmpdir.path().to_owned();
        src.push("src");
        let mut dst = tmpdir.path().to_owned();
        dst.push("src");

        bind_mount(Path::new(""), Path::new(""), false).unwrap_err();
        bind_mount(tmpdir2.path(), Path::new(""), false).unwrap_err();
        bind_mount(tmpdir2.path(), &dst, true).unwrap();
        umount_timeout(dst.to_str().unwrap(), 0).unwrap();
        bind_mount(&src, &dst, false).unwrap();
        umount_timeout(dst.to_str().unwrap(), 0).unwrap();
        bind_mount(Path::new("/tmp"), Path::new("/"), false).unwrap_err();
    }

    #[test]
    fn test_ensure_exist() {
        let tmpdir = tempfile::tempdir().unwrap();
        let src = Path::new("/proc/mounts");
        let mut dst = tmpdir.path().to_owned();
        dst.push("proc");
        dst.push("mounts");
        ensure_destination_exists(src, dst.as_path(), "bind").unwrap();
        dst.canonicalize().unwrap();

        let dst = Path::new("/");
        ensure_destination_exists(src, dst, "bind").unwrap_err();

        let src = Path::new("/proc");
        let dst = Path::new("/proc/mounts");
        ensure_destination_exists(src, dst, "bind").unwrap_err();
    }

    #[test]
    fn test_compact_overlay_lowerdirs() {
        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
            "lowerdir=/a/b/c/xxxx/1l:/a/b/c/xxxx/2l:/a/b/c/xxxx/3l:/a/b/c/xxxx/4l".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert_eq!(prefix, "/a/b/c/xxxx/");
        assert_eq!(n_options.len(), 3);
        assert_eq!(n_options[2], "lowerdir=1l:2l:3l:4l");

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
            "lowerdir=/1l:/2l:/3l:/4l".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert_eq!(prefix, "");
        assert_eq!(n_options, options);

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert_eq!(prefix, "");
        assert_eq!(n_options, options);

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "lowerdir=".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        let (prefix, n_options) = compact_lowerdir_option(&options);
        assert_eq!(prefix, "");
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

        let (idx, lower) = find_overlay_lowerdirs(&options);
        assert_eq!(idx, 2);
        assert_eq!(lower, lower_expect);

        let common_prefix = get_longest_common_prefix(&lower);
        assert_eq!("/a/b/c/xxxx/".to_string(), common_prefix);

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        let (idx, lower) = find_overlay_lowerdirs(&options);
        assert_eq!(idx, -1);
        assert!(lower.is_empty());

        let options = vec![
            "workdir=/a/b/c/xxxx/workdir".to_string(),
            "lowerdir=".to_string(),
            "upperdir=/a/b/c/xxxx/upper".to_string(),
        ];
        let (idx, lower) = find_overlay_lowerdirs(&options);
        assert_eq!(idx, -1);
        assert!(lower.is_empty());
    }

    #[test]
    fn test_get_common_prefix() {
        let lower1 = vec![
            "/a/b/c/xxxx/1l/fs".to_string(),
            "/a/b/c/xxxx/11l/fs".to_string(),
            "/a/b/c/xxxx/13l/fs".to_string(),
            "/a/b/c/xxxx/14l/fs".to_string(),
        ];
        let common_prefix = get_longest_common_prefix(&lower1);
        assert_eq!("/a/b/c/xxxx/".to_string(), common_prefix);

        let lower2 = vec![
            "/fs".to_string(),
            "/s".to_string(),
            "/sa".to_string(),
            "/s".to_string(),
        ];
        let common_prefix = get_longest_common_prefix(&lower2);
        assert_eq!("/", &common_prefix);

        let lower3 = vec!["".to_string(), "".to_string()];
        let common_prefix = get_longest_common_prefix(&lower3);
        assert_eq!("/", &common_prefix);

        let lower = vec!["/".to_string(), "/".to_string()];
        let common_prefix = get_longest_common_prefix(&lower);
        assert_eq!("/", &common_prefix);

        let lower = vec![
            "/a/b/c".to_string(),
            "/a/b/c/d".to_string(),
            "/a/b///c".to_string(),
        ];
        let common_prefix = get_longest_common_prefix(&lower);
        assert_eq!("/a/b/", &common_prefix);

        let lower = vec!["a/b/c/e".to_string(), "a/b/c/d".to_string()];
        let common_prefix = get_longest_common_prefix(&lower);
        assert_eq!("a/b/c/", &common_prefix);

        let lower = vec!["a/b/c".to_string(), "a/b/c/d".to_string()];
        let common_prefix = get_longest_common_prefix(&lower);
        assert_eq!("a/b/", &common_prefix);

        let lower = vec!["/test".to_string()];
        let common_prefix = get_longest_common_prefix(&lower);
        assert_eq!("/", &common_prefix);

        let lower = vec![];
        let common_prefix = get_longest_common_prefix(&lower);
        assert_eq!("", &common_prefix);
    }

    #[test]
    fn test_parse_mount_options() {
        let options = vec![];
        let mo = parse_mount_option(&options).unwrap();
        assert!(mo.flag.is_empty());
        assert!(mo.data.is_empty());

        let mut options = vec![
            "dev".to_string(),
            "ro".to_string(),
            "data-option".to_string(),
        ];
        let mo = parse_mount_option(&options).unwrap();
        assert_eq!(mo.flag, MsFlags::MS_RDONLY);
        assert_eq!(&mo.data, "data-option");

        options.push(" ".repeat(4097));
        assert!(parse_mount_option(&options).is_err());
    }

    #[test]
    fn test_mount_at() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().to_str().unwrap();
        mount_at(
            path,
            "/___does_not_exist____a___",
            "/tmp/etc/host.conf",
            "",
            MsFlags::empty(),
            "",
        )
        .unwrap_err();

        mount_at(
            "/___does_not_exist____a___",
            "/etc/host.conf",
            "/tmp/etc/host.conf",
            "",
            MsFlags::empty(),
            "",
        )
        .unwrap_err();
    }
}
