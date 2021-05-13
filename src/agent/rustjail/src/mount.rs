// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, bail, Context, Result};
use libc::uid_t;
use nix::errno::Errno;
use nix::fcntl::{self, OFlag};
#[cfg(not(test))]
use nix::mount;
use nix::mount::{MntFlags, MsFlags};
use nix::sys::stat::{self, Mode, SFlag};
use nix::unistd::{self, Gid, Uid};
use nix::NixPath;
use oci::{LinuxDevice, Mount, Spec};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::mem::MaybeUninit;
use std::os::unix;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

use path_absolutize::*;
use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::container::DEFAULT_DEVICES;
use crate::sync::write_count;
use std::string::ToString;

use crate::log_child;

// Info reveals information about a particular mounted filesystem. This
// struct is populated from the content in the /proc/<pid>/mountinfo file.
#[derive(std::fmt::Debug)]
pub struct Info {
    id: i32,
    parent: i32,
    major: i32,
    minor: i32,
    root: String,
    mount_point: String,
    opts: String,
    optional: String,
    fstype: String,
    source: String,
    vfs_opts: String,
}

const MOUNTINFOFORMAT: &str = "{d} {d} {d}:{d} {} {} {} {}";
const PROC_PATH: &str = "/proc";

// since libc didn't defined this const for musl, thus redefined it here.
#[cfg(all(target_os = "linux", target_env = "gnu", not(target_arch = "s390x")))]
const PROC_SUPER_MAGIC: libc::c_long = 0x00009fa0;
#[cfg(all(target_os = "linux", target_env = "musl"))]
const PROC_SUPER_MAGIC: libc::c_ulong = 0x00009fa0;
#[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "s390x"))]
const PROC_SUPER_MAGIC: libc::c_uint = 0x00009fa0;

lazy_static! {
    static ref PROPAGATION: HashMap<&'static str, MsFlags> = {
        let mut m = HashMap::new();
        m.insert("shared", MsFlags::MS_SHARED);
        m.insert("rshared", MsFlags::MS_SHARED | MsFlags::MS_REC);
        m.insert("private", MsFlags::MS_PRIVATE);
        m.insert("rprivate", MsFlags::MS_PRIVATE | MsFlags::MS_REC);
        m.insert("slave", MsFlags::MS_SLAVE);
        m.insert("rslave", MsFlags::MS_SLAVE | MsFlags::MS_REC);
        m.insert("unbindable", MsFlags::MS_UNBINDABLE);
        m.insert("runbindable", MsFlags::MS_UNBINDABLE | MsFlags::MS_REC);
        m
    };
    static ref OPTIONS: HashMap<&'static str, (bool, MsFlags)> = {
        let mut m = HashMap::new();
        m.insert("defaults", (false, MsFlags::empty()));
        m.insert("ro", (false, MsFlags::MS_RDONLY));
        m.insert("rw", (true, MsFlags::MS_RDONLY));
        m.insert("suid", (true, MsFlags::MS_NOSUID));
        m.insert("nosuid", (false, MsFlags::MS_NOSUID));
        m.insert("dev", (true, MsFlags::MS_NODEV));
        m.insert("nodev", (false, MsFlags::MS_NODEV));
        m.insert("exec", (true, MsFlags::MS_NOEXEC));
        m.insert("noexec", (false, MsFlags::MS_NOEXEC));
        m.insert("sync", (false, MsFlags::MS_SYNCHRONOUS));
        m.insert("async", (true, MsFlags::MS_SYNCHRONOUS));
        m.insert("dirsync", (false, MsFlags::MS_DIRSYNC));
        m.insert("remount", (false, MsFlags::MS_REMOUNT));
        m.insert("mand", (false, MsFlags::MS_MANDLOCK));
        m.insert("nomand", (true, MsFlags::MS_MANDLOCK));
        m.insert("atime", (true, MsFlags::MS_NOATIME));
        m.insert("noatime", (false, MsFlags::MS_NOATIME));
        m.insert("diratime", (true, MsFlags::MS_NODIRATIME));
        m.insert("nodiratime", (false, MsFlags::MS_NODIRATIME));
        m.insert("bind", (false, MsFlags::MS_BIND));
        m.insert("rbind", (false, MsFlags::MS_BIND | MsFlags::MS_REC));
        m.insert("relatime", (false, MsFlags::MS_RELATIME));
        m.insert("norelatime", (true, MsFlags::MS_RELATIME));
        m.insert("strictatime", (false, MsFlags::MS_STRICTATIME));
        m.insert("nostrictatime", (true, MsFlags::MS_STRICTATIME));
        m
    };
}

#[inline(always)]
#[allow(unused_variables)]
pub fn mount<
    P1: ?Sized + NixPath,
    P2: ?Sized + NixPath,
    P3: ?Sized + NixPath,
    P4: ?Sized + NixPath,
>(
    source: Option<&P1>,
    target: &P2,
    fstype: Option<&P3>,
    flags: MsFlags,
    data: Option<&P4>,
) -> std::result::Result<(), nix::Error> {
    #[cfg(not(test))]
    return mount::mount(source, target, fstype, flags, data);
    #[cfg(test)]
    return Ok(());
}

#[inline(always)]
#[allow(unused_variables)]
pub fn umount2<P: ?Sized + NixPath>(
    target: &P,
    flags: MntFlags,
) -> std::result::Result<(), nix::Error> {
    #[cfg(not(test))]
    return mount::umount2(target, flags);
    #[cfg(test)]
    return Ok(());
}

pub fn init_rootfs(
    cfd_log: RawFd,
    spec: &Spec,
    cpath: &HashMap<String, String>,
    mounts: &HashMap<String, String>,
    bind_device: bool,
) -> Result<()> {
    lazy_static::initialize(&OPTIONS);
    lazy_static::initialize(&PROPAGATION);
    lazy_static::initialize(&LINUXDEVICETYPE);

    let linux = &spec
        .linux
        .as_ref()
        .ok_or_else(|| anyhow!("Could not get linux configuration from spec"))?;

    let mut flags = MsFlags::MS_REC;
    match PROPAGATION.get(&linux.rootfs_propagation.as_str()) {
        Some(fl) => flags |= *fl,
        None => flags |= MsFlags::MS_SLAVE,
    }

    let root = spec
        .root
        .as_ref()
        .ok_or_else(|| anyhow!("Could not get rootfs path from spec"))
        .and_then(|r| {
            fs::canonicalize(r.path.as_str()).context("Could not canonicalize rootfs path")
        })?;

    let rootfs = (*root)
        .to_str()
        .ok_or_else(|| anyhow!("Could not convert rootfs path to string"))?;

    mount(None::<&str>, "/", None::<&str>, flags, None::<&str>)?;

    rootfs_parent_mount_private(rootfs)?;

    mount(
        Some(rootfs),
        rootfs,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )?;

    let mut bind_mount_dev = false;
    for m in &spec.mounts {
        let (mut flags, pgflags, data) = parse_mount(&m);
        if !m.destination.starts_with('/') || m.destination.contains("..") {
            return Err(anyhow!(
                "the mount destination {} is invalid",
                m.destination
            ));
        }

        if m.r#type == "cgroup" {
            mount_cgroups(cfd_log, &m, rootfs, flags, &data, cpath, mounts)?;
        } else {
            if m.destination == "/dev" {
                if m.r#type == "bind" {
                    bind_mount_dev = true;
                }
                flags &= !MsFlags::MS_RDONLY;
            }

            if m.r#type == "bind" {
                check_proc_mount(m)?;
            }

            // If the destination already exists and is not a directory, we bail
            // out This is to avoid mounting through a symlink or similar -- which
            // has been a "fun" attack scenario in the past.
            if m.r#type == "proc" || m.r#type == "sysfs" {
                if let Ok(meta) = fs::symlink_metadata(&m.destination) {
                    if !meta.is_dir() {
                        return Err(anyhow!(
                            "Mount point {} must be ordinary directory: got {:?}",
                            m.destination,
                            meta.file_type()
                        ));
                    }
                }
            }

            mount_from(cfd_log, &m, &rootfs, flags, &data, "")?;
            // bind mount won't change mount options, we need remount to make mount options
            // effective.
            // first check that we have non-default options required before attempting a
            // remount
            if m.r#type == "bind" && !pgflags.is_empty() {
                let dest = secure_join(rootfs, &m.destination);
                mount(
                    None::<&str>,
                    dest.as_str(),
                    None::<&str>,
                    pgflags,
                    None::<&str>,
                )?;
            }
        }
    }

    let olddir = unistd::getcwd()?;
    unistd::chdir(rootfs)?;

    // in case the /dev directory was binded mount from guest,
    // then there's no need to create devices nodes and symlinks
    // in /dev.
    if !bind_mount_dev {
        default_symlinks()?;
        create_devices(&linux.devices, bind_device)?;
        ensure_ptmx()?;
    }

    unistd::chdir(&olddir)?;

    Ok(())
}

fn check_proc_mount(m: &Mount) -> Result<()> {
    // White list, it should be sub directories of invalid destinations
    // These entries can be bind mounted by files emulated by fuse,
    // so commands like top, free displays stats in container.
    let valid_destinations = [
        "/proc/cpuinfo",
        "/proc/diskstats",
        "/proc/meminfo",
        "/proc/stat",
        "/proc/swaps",
        "/proc/uptime",
        "/proc/loadavg",
        "/proc/net/dev",
    ];

    for i in valid_destinations.iter() {
        if m.destination.as_str() == *i {
            return Ok(());
        }
    }

    if m.destination == PROC_PATH {
        // only allow a mount on-top of proc if it's source is "proc"
        unsafe {
            let mut stats = MaybeUninit::<libc::statfs>::uninit();
            if m.source
                .with_nix_path(|path| libc::statfs(path.as_ptr(), stats.as_mut_ptr()))
                .is_ok()
            {
                if stats.assume_init().f_type == PROC_SUPER_MAGIC {
                    return Ok(());
                }
            } else {
                return Ok(());
            }

            return Err(anyhow!(format!(
                "{} cannot be mounted to {} because it is not of type proc",
                m.source, m.destination
            )));
        }
    }

    if m.destination.starts_with(PROC_PATH) {
        return Err(anyhow!(format!(
            "{} cannot be mounted because it is inside /proc",
            m.destination
        )));
    }

    Ok(())
}

fn mount_cgroups_v2(cfd_log: RawFd, m: &Mount, rootfs: &str, flags: MsFlags) -> Result<()> {
    let olddir = unistd::getcwd()?;
    unistd::chdir(rootfs)?;

    // https://github.com/opencontainers/runc/blob/09ddc63afdde16d5fb859a1d3ab010bd45f08497/libcontainer/rootfs_linux.go#L287
    let bm = Mount {
        source: "cgroup".to_string(),
        r#type: "cgroup2".to_string(),
        destination: m.destination.clone(),
        options: Vec::new(),
    };

    let mount_flags: MsFlags = flags;

    mount_from(cfd_log, &bm, rootfs, mount_flags, "", "")?;

    unistd::chdir(&olddir)?;

    if flags.contains(MsFlags::MS_RDONLY) {
        let dest = format!("{}{}", rootfs, m.destination.as_str());
        mount(
            Some(dest.as_str()),
            dest.as_str(),
            None::<&str>,
            flags | MsFlags::MS_BIND | MsFlags::MS_REMOUNT,
            None::<&str>,
        )?;
    }

    Ok(())
}

fn mount_cgroups(
    cfd_log: RawFd,
    m: &Mount,
    rootfs: &str,
    flags: MsFlags,
    _data: &str,
    cpath: &HashMap<String, String>,
    mounts: &HashMap<String, String>,
) -> Result<()> {
    if cgroups::hierarchies::is_cgroup2_unified_mode() {
        return mount_cgroups_v2(cfd_log, &m, rootfs, flags);
    }
    // mount tmpfs
    let ctm = Mount {
        source: "tmpfs".to_string(),
        r#type: "tmpfs".to_string(),
        destination: m.destination.clone(),
        options: Vec::new(),
    };

    let cflags = MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV;
    mount_from(cfd_log, &ctm, rootfs, cflags, "", "")?;
    let olddir = unistd::getcwd()?;

    unistd::chdir(rootfs)?;

    let mut srcs: HashSet<String> = HashSet::new();

    // bind mount cgroups
    for (key, mount) in mounts.iter() {
        log_child!(cfd_log, "mount cgroup subsystem {}", key);
        let source = if cpath.get(key).is_some() {
            cpath.get(key).unwrap()
        } else {
            continue;
        };

        let base = if let Some(o) = mount.rfind('/') {
            &mount[o + 1..]
        } else {
            &mount[..]
        };

        let destination = format!("{}/{}", m.destination.as_str(), base);

        if srcs.contains(source) {
            // already mounted, xxx,yyy style cgroup
            if key != base {
                let src = format!("{}/{}", m.destination.as_str(), key);
                unix::fs::symlink(destination.as_str(), &src[1..])?;
            }

            continue;
        }

        srcs.insert(source.to_string());

        log_child!(cfd_log, "mount destination: {}", destination.as_str());

        let bm = Mount {
            source: source.to_string(),
            r#type: "bind".to_string(),
            destination: destination.clone(),
            options: Vec::new(),
        };

        let mut mount_flags: MsFlags = flags | MsFlags::MS_REC | MsFlags::MS_BIND;
        if key.contains("systemd") {
            mount_flags &= !MsFlags::MS_RDONLY;
        }
        mount_from(cfd_log, &bm, rootfs, mount_flags, "", "")?;

        if key != base {
            let src = format!("{}/{}", m.destination.as_str(), key);
            unix::fs::symlink(destination.as_str(), &src[1..]).map_err(|e| {
                log_child!(
                    cfd_log,
                    "symlink: {} {} err: {}",
                    key,
                    destination.as_str(),
                    e.to_string()
                );

                e
            })?;
        }
    }

    unistd::chdir(&olddir)?;

    if flags.contains(MsFlags::MS_RDONLY) {
        let dest = format!("{}{}", rootfs, m.destination.as_str());
        mount(
            Some(dest.as_str()),
            dest.as_str(),
            None::<&str>,
            flags | MsFlags::MS_BIND | MsFlags::MS_REMOUNT,
            None::<&str>,
        )?;
    }

    Ok(())
}

#[allow(unused_variables)]
fn pivot_root<P1: ?Sized + NixPath, P2: ?Sized + NixPath>(
    new_root: &P1,
    put_old: &P2,
) -> anyhow::Result<(), nix::Error> {
    #[cfg(not(test))]
    return unistd::pivot_root(new_root, put_old);
    #[cfg(test)]
    return Ok(());
}

pub fn pivot_rootfs<P: ?Sized + NixPath + std::fmt::Debug>(path: &P) -> Result<()> {
    let oldroot = fcntl::open("/", OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;
    defer!(unistd::close(oldroot).unwrap());
    let newroot = fcntl::open(path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;
    defer!(unistd::close(newroot).unwrap());

    // Change to the new root so that the pivot_root actually acts on it.
    unistd::fchdir(newroot)?;
    pivot_root(".", ".").context(format!("failed to pivot_root on {:?}", path))?;

    // Currently our "." is oldroot (according to the current kernel code).
    // However, purely for safety, we will fchdir(oldroot) since there isn't
    // really any guarantee from the kernel what /proc/self/cwd will be after a
    // pivot_root(2).
    unistd::fchdir(oldroot)?;

    // Make oldroot rslave to make sure our unmounts don't propagate to the
    // host. We don't use rprivate because this is known to cause issues due
    // to races where we still have a reference to a mount while a process in
    // the host namespace are trying to operate on something they think has no
    // mounts (devicemapper in particular).
    mount(
        Some("none"),
        ".",
        Some(""),
        MsFlags::MS_SLAVE | MsFlags::MS_REC,
        Some(""),
    )?;

    // Preform the unmount. MNT_DETACH allows us to unmount /proc/self/cwd.
    umount2(".", MntFlags::MNT_DETACH).context("failed to do umount2")?;

    // Switch back to our shiny new root.
    unistd::chdir("/")?;
    stat::umask(Mode::from_bits_truncate(0o022));
    Ok(())
}

fn rootfs_parent_mount_private(path: &str) -> Result<()> {
    let mount_infos = parse_mount_table()?;

    let mut max_len = 0;
    let mut mount_point = String::from("");
    let mut options = String::from("");
    for i in mount_infos {
        if path.starts_with(&i.mount_point) && i.mount_point.len() > max_len {
            max_len = i.mount_point.len();
            mount_point = i.mount_point;
            options = i.optional;
        }
    }

    if options.contains("shared:") {
        mount(
            None::<&str>,
            mount_point.as_str(),
            None::<&str>,
            MsFlags::MS_PRIVATE,
            None::<&str>,
        )?;
    }

    Ok(())
}

// Parse /proc/self/mountinfo because comparing Dev and ino does not work from
// bind mounts
fn parse_mount_table() -> Result<Vec<Info>> {
    let file = File::open("/proc/self/mountinfo")?;
    let reader = BufReader::new(file);
    let mut infos = Vec::new();

    for (_index, line) in reader.lines().enumerate() {
        let line = line?;

        let (id, parent, major, minor, root, mount_point, opts, optional) = scan_fmt!(
            &line,
            MOUNTINFOFORMAT,
            i32,
            i32,
            i32,
            i32,
            String,
            String,
            String,
            String
        )?;

        let fields: Vec<&str> = line.split(" - ").collect();
        if fields.len() == 2 {
            let (fstype, source, vfs_opts) =
                scan_fmt!(fields[1], "{} {} {}", String, String, String)?;

            let mut optional_new = String::new();
            if optional != "-" {
                optional_new = optional;
            }

            let info = Info {
                id,
                parent,
                major,
                minor,
                root,
                mount_point,
                opts,
                optional: optional_new,
                fstype,
                source,
                vfs_opts,
            };

            infos.push(info);
        } else {
            return Err(anyhow!("failed to parse mount info file".to_string()));
        }
    }

    Ok(infos)
}

#[inline(always)]
#[allow(unused_variables)]
fn chroot<P: ?Sized + NixPath>(path: &P) -> Result<(), nix::Error> {
    #[cfg(not(test))]
    return unistd::chroot(path);
    #[cfg(test)]
    return Ok(());
}

pub fn ms_move_root(rootfs: &str) -> Result<bool> {
    unistd::chdir(rootfs)?;
    let mount_infos = parse_mount_table()?;

    let root_path = Path::new(rootfs);
    let abs_root_buf = root_path.absolutize()?;
    let abs_root = abs_root_buf
        .to_str()
        .ok_or_else(|| anyhow!("failed to parse {} to absolute path", rootfs))?;

    for info in mount_infos.iter() {
        let mount_point = Path::new(&info.mount_point);
        let abs_mount_buf = mount_point.absolutize()?;
        let abs_mount_point = abs_mount_buf
            .to_str()
            .ok_or_else(|| anyhow!("failed to parse {} to absolute path", info.mount_point))?;
        let abs_mount_point_string = String::from(abs_mount_point);

        // Umount every syfs and proc file systems, except those under the container rootfs
        if (info.fstype != "proc" && info.fstype != "sysfs")
            || abs_mount_point_string.starts_with(abs_root)
        {
            continue;
        }

        // Be sure umount events are not propagated to the host.
        mount(
            None::<&str>,
            abs_mount_point,
            None::<&str>,
            MsFlags::MS_SLAVE | MsFlags::MS_REC,
            None::<&str>,
        )?;
        umount2(abs_mount_point, MntFlags::MNT_DETACH).or_else(|e| {
            if e.ne(&nix::Error::from(Errno::EINVAL)) && e.ne(&nix::Error::from(Errno::EPERM)) {
                return Err(anyhow!(e));
            }

            // If we have not privileges for umounting (e.g. rootless), then
            // cover the path.
            mount(
                Some("tmpfs"),
                abs_mount_point,
                Some("tmpfs"),
                MsFlags::empty(),
                None::<&str>,
            )?;

            Ok(())
        })?;
    }

    mount(
        Some(abs_root),
        "/",
        None::<&str>,
        MsFlags::MS_MOVE,
        None::<&str>,
    )?;
    chroot(".")?;
    unistd::chdir("/")?;

    Ok(true)
}

fn parse_mount(m: &Mount) -> (MsFlags, MsFlags, String) {
    let mut flags = MsFlags::empty();
    let mut pgflags = MsFlags::empty();
    let mut data = Vec::new();

    for o in &m.options {
        if let Some(v) = OPTIONS.get(o.as_str()) {
            let (clear, fl) = *v;
            if clear {
                flags &= !fl;
            } else {
                flags |= fl;
            }
        } else if let Some(fl) = PROPAGATION.get(o.as_str()) {
            pgflags |= *fl;
        } else {
            data.push(o.clone());
        }
    }

    (flags, pgflags, data.join(","))
}

// This function constructs a canonicalized path by combining the `rootfs` and `unsafe_path` elements.
// The resulting path is guaranteed to be ("below" / "in a directory under") the `rootfs` directory.
//
// Parameters:
//
// - `rootfs` is the absolute path to the root of the containers root filesystem directory.
// - `unsafe_path` is path inside a container. It is unsafe since it may try to "escape" from the containers
//    rootfs by using one or more "../" path elements or is its a symlink to path.
fn secure_join(rootfs: &str, unsafe_path: &str) -> String {
    let mut path = PathBuf::from(format!("{}/", rootfs));
    let unsafe_p = Path::new(&unsafe_path);

    for it in unsafe_p.iter() {
        let it_p = Path::new(&it);

        // if it_p leads with "/", path.push(it) will be replace as it, so ignore "/"
        if it_p.has_root() {
            continue;
        };

        path.push(it);
        if let Ok(v) = path.read_link() {
            if v.is_absolute() {
                path = PathBuf::from(format!("{}{}", rootfs, v.to_str().unwrap().to_string()));
            } else {
                path.pop();
                for it in v.iter() {
                    path.push(it);
                    if path.exists() {
                        path = path.canonicalize().unwrap();
                        if !path.starts_with(rootfs) {
                            path = PathBuf::from(rootfs.to_string());
                        }
                    }
                }
            }
        }
        // skip any ".."
        if path.ends_with("..") {
            path.pop();
        }
    }

    path.to_str().unwrap().to_string()
}

fn mount_from(
    cfd_log: RawFd,
    m: &Mount,
    rootfs: &str,
    flags: MsFlags,
    data: &str,
    _label: &str,
) -> Result<()> {
    let d = String::from(data);
    let dest = secure_join(rootfs, &m.destination);

    let src = if m.r#type.as_str() == "bind" {
        let src = fs::canonicalize(m.source.as_str())?;
        let dir = if src.is_dir() {
            Path::new(&dest)
        } else {
            Path::new(&dest).parent().unwrap()
        };

        let _ = fs::create_dir_all(&dir).map_err(|e| {
            log_child!(
                cfd_log,
                "creat dir {}: {}",
                dir.to_str().unwrap(),
                e.to_string()
            )
        });

        // make sure file exists so we can bind over it
        if !src.is_dir() {
            let _ = OpenOptions::new().create(true).write(true).open(&dest);
        }
        src.to_str().unwrap().to_string()
    } else {
        let _ = fs::create_dir_all(&dest);
        if m.r#type.as_str() == "cgroup2" {
            "cgroup2".to_string()
        } else {
            let tmp = PathBuf::from(&m.source);
            tmp.to_str().unwrap().to_string()
        }
    };

    let _ = stat::stat(dest.as_str()).map_err(|e| {
        log_child!(
            cfd_log,
            "dest stat error. {}: {:?}",
            dest.as_str(),
            e.as_errno()
        )
    });

    mount(
        Some(src.as_str()),
        dest.as_str(),
        Some(m.r#type.as_str()),
        flags,
        Some(d.as_str()),
    )
    .map_err(|e| {
        log_child!(cfd_log, "mount error: {:?}", e.as_errno());
        e
    })?;

    if flags.contains(MsFlags::MS_BIND)
        && flags.intersects(
            !(MsFlags::MS_REC
                | MsFlags::MS_REMOUNT
                | MsFlags::MS_BIND
                | MsFlags::MS_PRIVATE
                | MsFlags::MS_SHARED
                | MsFlags::MS_SLAVE),
        )
    {
        mount(
            Some(dest.as_str()),
            dest.as_str(),
            None::<&str>,
            flags | MsFlags::MS_REMOUNT,
            None::<&str>,
        )
        .map_err(|e| {
            log_child!(cfd_log, "remout {}: {:?}", dest.as_str(), e.as_errno());
            e
        })?;
    }
    Ok(())
}

static SYMLINKS: &[(&str, &str)] = &[
    ("/proc/self/fd", "dev/fd"),
    ("/proc/self/fd/0", "dev/stdin"),
    ("/proc/self/fd/1", "dev/stdout"),
    ("/proc/self/fd/2", "dev/stderr"),
];

fn default_symlinks() -> Result<()> {
    if Path::new("/proc/kcore").exists() {
        unix::fs::symlink("/proc/kcore", "dev/kcore")?;
    }
    for &(src, dst) in SYMLINKS {
        unix::fs::symlink(src, dst)?;
    }
    Ok(())
}
fn create_devices(devices: &[LinuxDevice], bind: bool) -> Result<()> {
    let op: fn(&LinuxDevice) -> Result<()> = if bind { bind_dev } else { mknod_dev };
    let old = stat::umask(Mode::from_bits_truncate(0o000));
    for dev in DEFAULT_DEVICES.iter() {
        op(dev)?;
    }
    for dev in devices {
        if !dev.path.starts_with("/dev") || dev.path.contains("..") {
            let msg = format!("{} is not a valid device path", dev.path);
            bail!(anyhow!(msg));
        }
        op(dev)?;
    }
    stat::umask(old);
    Ok(())
}

fn ensure_ptmx() -> Result<()> {
    let _ = fs::remove_file("dev/ptmx");
    unix::fs::symlink("pts/ptmx", "dev/ptmx")?;
    Ok(())
}

lazy_static! {
    static ref LINUXDEVICETYPE: HashMap<&'static str, SFlag> = {
        let mut m = HashMap::new();
        m.insert("c", SFlag::S_IFCHR);
        m.insert("b", SFlag::S_IFBLK);
        m.insert("p", SFlag::S_IFIFO);
        m
    };
}

fn mknod_dev(dev: &LinuxDevice) -> Result<()> {
    let f = match LINUXDEVICETYPE.get(dev.r#type.as_str()) {
        Some(v) => v,
        None => return Err(anyhow!("invalid spec".to_string())),
    };

    stat::mknod(
        &dev.path[1..],
        *f,
        Mode::from_bits_truncate(dev.file_mode.unwrap_or(0)),
        nix::sys::stat::makedev(dev.major as u64, dev.minor as u64),
    )?;

    unistd::chown(
        &dev.path[1..],
        Some(Uid::from_raw(dev.uid.unwrap_or(0) as uid_t)),
        Some(Gid::from_raw(dev.gid.unwrap_or(0) as uid_t)),
    )?;

    Ok(())
}

fn bind_dev(dev: &LinuxDevice) -> Result<()> {
    let fd = fcntl::open(
        &dev.path[1..],
        OFlag::O_RDWR | OFlag::O_CREAT,
        Mode::from_bits_truncate(0o644),
    )?;

    unistd::close(fd)?;

    mount(
        Some(&*dev.path),
        &dev.path[1..],
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )?;
    Ok(())
}

pub fn finish_rootfs(cfd_log: RawFd, spec: &Spec) -> Result<()> {
    let olddir = unistd::getcwd()?;
    log_child!(cfd_log, "old cwd: {}", olddir.to_str().unwrap());
    unistd::chdir("/")?;
    if spec.linux.is_some() {
        let linux = spec.linux.as_ref().unwrap();

        for path in linux.masked_paths.iter() {
            mask_path(path)?;
        }

        for path in linux.readonly_paths.iter() {
            readonly_path(path)?;
        }
    }

    for m in spec.mounts.iter() {
        if m.destination == "/dev" {
            let (flags, _, _) = parse_mount(m);
            if flags.contains(MsFlags::MS_RDONLY) {
                mount(
                    Some("/dev"),
                    "/dev",
                    None::<&str>,
                    flags | MsFlags::MS_REMOUNT,
                    None::<&str>,
                )?;
            }
        }
    }

    if spec.root.as_ref().unwrap().readonly {
        let flags = MsFlags::MS_BIND | MsFlags::MS_RDONLY | MsFlags::MS_NODEV | MsFlags::MS_REMOUNT;

        mount(Some("/"), "/", None::<&str>, flags, None::<&str>)?;
    }
    stat::umask(Mode::from_bits_truncate(0o022));
    unistd::chdir(&olddir)?;

    Ok(())
}

fn mask_path(path: &str) -> Result<()> {
    if !path.starts_with('/') || path.contains("..") {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }

    match mount(
        Some("/dev/null"),
        path,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    ) {
        Err(nix::Error::Sys(e)) => {
            if e != Errno::ENOENT && e != Errno::ENOTDIR {
                //info!("{}: {}", path, e.desc());
                return Err(nix::Error::Sys(e).into());
            }
        }

        Err(e) => {
            return Err(e.into());
        }

        Ok(_) => {}
    }

    Ok(())
}

fn readonly_path(path: &str) -> Result<()> {
    if !path.starts_with('/') || path.contains("..") {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }

    match mount(
        Some(&path[1..]),
        path,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    ) {
        Err(nix::Error::Sys(e)) => {
            if e == Errno::ENOENT {
                return Ok(());
            } else {
                //info!("{}: {}", path, e.desc());
                return Err(nix::Error::Sys(e).into());
            }
        }

        Err(e) => {
            return Err(e.into());
        }

        Ok(_) => {}
    }

    mount(
        Some(&path[1..]),
        &path[1..],
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT,
        None::<&str>,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skip_if_not_root;
    use std::fs::create_dir;
    use std::fs::create_dir_all;
    use std::fs::remove_dir_all;
    use std::os::unix::fs;
    use std::os::unix::io::AsRawFd;
    use tempfile::tempdir;

    #[test]
    #[serial(chdir)]
    fn test_init_rootfs() {
        let stdout_fd = std::io::stdout().as_raw_fd();
        let mut spec = oci::Spec::default();
        let cpath = HashMap::new();
        let mounts = HashMap::new();

        // there is no spec.linux, should fail
        let ret = init_rootfs(stdout_fd, &spec, &cpath, &mounts, true);
        assert!(
            ret.is_err(),
            "Should fail: there is no spec.linux. Got: {:?}",
            ret
        );

        // there is no spec.Root, should fail
        spec.linux = Some(oci::Linux::default());
        let ret = init_rootfs(stdout_fd, &spec, &cpath, &mounts, true);
        assert!(
            ret.is_err(),
            "should fail: there is no spec.Root. Got: {:?}",
            ret
        );

        let rootfs = tempdir().unwrap();
        let ret = create_dir(rootfs.path().join("dev"));
        assert!(ret.is_ok(), "Got: {:?}", ret);

        spec.root = Some(oci::Root {
            path: rootfs.path().to_str().unwrap().to_string(),
            readonly: false,
        });

        // there is no spec.mounts, but should pass
        let ret = init_rootfs(stdout_fd, &spec, &cpath, &mounts, true);
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
        let _ = remove_dir_all(rootfs.path().join("dev"));
        let _ = create_dir(rootfs.path().join("dev"));

        // Adding bad mount point to spec.mounts
        spec.mounts.push(oci::Mount {
            destination: "error".into(),
            r#type: "bind".into(),
            source: "error".into(),
            options: vec!["shared".into(), "rw".into(), "dev".into()],
        });

        // destination doesn't start with /, should fail
        let ret = init_rootfs(stdout_fd, &spec, &cpath, &mounts, true);
        assert!(
            ret.is_err(),
            "Should fail: destination doesn't start with '/'. Got: {:?}",
            ret
        );
        spec.mounts.pop();
        let _ = remove_dir_all(rootfs.path().join("dev"));
        let _ = create_dir(rootfs.path().join("dev"));

        // mounting a cgroup
        spec.mounts.push(oci::Mount {
            destination: "/cgroup".into(),
            r#type: "cgroup".into(),
            source: "/cgroup".into(),
            options: vec!["shared".into()],
        });

        let ret = init_rootfs(stdout_fd, &spec, &cpath, &mounts, true);
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
        spec.mounts.pop();
        let _ = remove_dir_all(rootfs.path().join("dev"));
        let _ = create_dir(rootfs.path().join("dev"));

        // mounting /dev
        spec.mounts.push(oci::Mount {
            destination: "/dev".into(),
            r#type: "bind".into(),
            source: "/dev".into(),
            options: vec!["shared".into()],
        });

        let ret = init_rootfs(stdout_fd, &spec, &cpath, &mounts, true);
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }

    #[test]
    #[serial(chdir)]
    fn test_mount_cgroups() {
        let stdout_fd = std::io::stdout().as_raw_fd();
        let mount = oci::Mount {
            destination: "/cgroups".to_string(),
            r#type: "cgroup".to_string(),
            source: "/cgroups".to_string(),
            options: vec!["shared".to_string()],
        };
        let tempdir = tempdir().unwrap();
        let rootfs = tempdir.path().to_str().unwrap().to_string();
        let flags = MsFlags::MS_RDONLY;
        let mut cpath = HashMap::new();
        let mut cgroup_mounts = HashMap::new();

        cpath.insert("cpu".to_string(), "cpu".to_string());
        cpath.insert("memory".to_string(), "memory".to_string());

        cgroup_mounts.insert("default".to_string(), "default".to_string());
        cgroup_mounts.insert("cpu".to_string(), "cpu".to_string());
        cgroup_mounts.insert("memory".to_string(), "memory".to_string());

        let ret = create_dir_all(tempdir.path().join("cgroups"));
        assert!(ret.is_ok(), "Should pass. Got {:?}", ret);
        let ret = create_dir_all(tempdir.path().join("cpu"));
        assert!(ret.is_ok(), "Should pass. Got {:?}", ret);
        let ret = create_dir_all(tempdir.path().join("memory"));
        assert!(ret.is_ok(), "Should pass. Got {:?}", ret);

        let ret = mount_cgroups(
            stdout_fd,
            &mount,
            &rootfs,
            flags,
            "",
            &cpath,
            &cgroup_mounts,
        );
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }

    #[test]
    #[serial(chdir)]
    fn test_pivot_root() {
        let ret = pivot_rootfs("/tmp");
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }

    #[test]
    #[serial(chdir)]
    fn test_ms_move_rootfs() {
        let ret = ms_move_root("/abc");
        assert!(
            ret.is_err(),
            "Should fail. path doesn't exist. Got: {:?}",
            ret
        );

        let ret = ms_move_root("/tmp");
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }

    #[test]
    fn test_mask_path() {
        let ret = mask_path("abc");
        assert!(
            ret.is_err(),
            "Should fail: path doesn't start with '/'. Got: {:?}",
            ret
        );

        let ret = mask_path("abc/../");
        assert!(
            ret.is_err(),
            "Should fail: path contains '..'. Got: {:?}",
            ret
        );

        let ret = mask_path("/tmp");
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }

    #[test]
    #[serial(chdir)]
    fn test_finish_rootfs() {
        let stdout_fd = std::io::stdout().as_raw_fd();
        let mut spec = oci::Spec::default();

        spec.linux = Some(oci::Linux::default());
        spec.linux.as_mut().unwrap().masked_paths = vec!["/tmp".to_string()];
        spec.linux.as_mut().unwrap().readonly_paths = vec!["/tmp".to_string()];
        spec.root = Some(oci::Root {
            path: "/tmp".to_string(),
            readonly: true,
        });
        spec.mounts = vec![oci::Mount {
            destination: "/dev".to_string(),
            r#type: "bind".to_string(),
            source: "/dev".to_string(),
            options: vec!["ro".to_string(), "shared".to_string()],
        }];

        let ret = finish_rootfs(stdout_fd, &spec);
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }

    #[test]
    fn test_readonly_path() {
        let ret = readonly_path("abc");
        assert!(ret.is_err(), "Should fail. Got: {:?}", ret);

        let ret = readonly_path("../../");
        assert!(ret.is_err(), "Should fail. Got: {:?}", ret);

        let ret = readonly_path("/tmp");
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }

    #[test]
    #[serial(chdir)]
    fn test_mknod_dev() {
        skip_if_not_root!();

        let tempdir = tempdir().unwrap();

        let olddir = unistd::getcwd().unwrap();
        defer!(let _ = unistd::chdir(&olddir););
        let _ = unistd::chdir(tempdir.path());

        let dev = oci::LinuxDevice {
            path: "/fifo".to_string(),
            r#type: "c".to_string(),
            major: 0,
            minor: 0,
            file_mode: Some(0660),
            uid: Some(unistd::getuid().as_raw()),
            gid: Some(unistd::getgid().as_raw()),
        };

        let ret = mknod_dev(&dev);
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);

        let ret = stat::stat("fifo");
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);
    }
    #[test]
    fn test_check_proc_mount() {
        let mount = oci::Mount {
            destination: "/proc".to_string(),
            r#type: "bind".to_string(),
            source: "/test".to_string(),
            options: vec!["shared".to_string()],
        };

        assert!(check_proc_mount(&mount).is_err());

        let mount = oci::Mount {
            destination: "/proc/cpuinfo".to_string(),
            r#type: "bind".to_string(),
            source: "/test".to_string(),
            options: vec!["shared".to_string()],
        };

        assert!(check_proc_mount(&mount).is_ok());

        let mount = oci::Mount {
            destination: "/proc/test".to_string(),
            r#type: "bind".to_string(),
            source: "/test".to_string(),
            options: vec!["shared".to_string()],
        };

        assert!(check_proc_mount(&mount).is_err());
    }

    #[test]
    fn test_secure_join() {
        #[derive(Debug)]
        struct TestData<'a> {
            name: &'a str,
            rootfs: &'a str,
            unsafe_path: &'a str,
            symlink_path: &'a str,
            result: &'a str,
        }

        // create tempory directory to simulate container rootfs with symlink
        let rootfs_dir = tempdir().expect("failed to create tmpdir");
        let rootfs_path = rootfs_dir.path().to_str().unwrap();

        let tests = &[
            TestData {
                name: "rootfs_not_exist",
                rootfs: "/home/rootfs",
                unsafe_path: "a/b/c",
                symlink_path: "",
                result: "/home/rootfs/a/b/c",
            },
            TestData {
                name: "relative_path",
                rootfs: "/home/rootfs",
                unsafe_path: "../../../a/b/c",
                symlink_path: "",
                result: "/home/rootfs/a/b/c",
            },
            TestData {
                name: "skip any ..",
                rootfs: "/home/rootfs",
                unsafe_path: "../../../a/../../b/../../c",
                symlink_path: "",
                result: "/home/rootfs/a/b/c",
            },
            TestData {
                name: "rootfs is null",
                rootfs: "",
                unsafe_path: "",
                symlink_path: "",
                result: "/",
            },
            TestData {
                name: "relative softlink beyond container rootfs",
                rootfs: rootfs_path,
                unsafe_path: "1",
                symlink_path: "../../../",
                result: rootfs_path,
            },
            TestData {
                name: "abs softlink points to the non-exist directory",
                rootfs: rootfs_path,
                unsafe_path: "2",
                symlink_path: "/dddd",
                result: &format!("{}/dddd", rootfs_path).as_str().to_owned(),
            },
            TestData {
                name: "abs softlink points to the root",
                rootfs: rootfs_path,
                unsafe_path: "3",
                symlink_path: "/",
                result: &format!("{}/", rootfs_path).as_str().to_owned(),
            },
        ];

        for (i, t) in tests.iter().enumerate() {
            // Create a string containing details of the test
            let msg = format!("test[{}]: {:?}", i, t);

            // if is_symlink, then should be prepare the softlink environment
            if t.symlink_path != "" {
                fs::symlink(t.symlink_path, format!("{}/{}", t.rootfs, t.unsafe_path)).unwrap();
            }
            let result = secure_join(t.rootfs, t.unsafe_path);

            // Update the test details string with the results of the call
            let msg = format!("{}, result: {:?}", msg, result);

            // Perform the checks
            assert!(result == t.result, "{}", msg);
        }
    }
}
