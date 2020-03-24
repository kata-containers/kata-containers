// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use libc::uid_t;
use nix::errno::Errno;
use nix::fcntl::{self, OFlag};
use nix::mount::{self, MntFlags, MsFlags};
use nix::sys::stat::{self, Mode, SFlag};
use nix::unistd::{self, Gid, Uid};
use nix::NixPath;
use oci::{LinuxDevice, Mount, Spec};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::os::unix;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

use path_absolutize::*;
use scan_fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::container::DEFAULT_DEVICES;
use crate::errors::*;
use crate::sync::write_count;
use lazy_static;
use std::string::ToString;

use crate::log_child;

// Info reveals information about a particular mounted filesystem. This
// struct is populated from the content in the /proc/<pid>/mountinfo file.
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

const MOUNTINFOFORMAT: &'static str = "{d} {d} {d}:{d} {} {} {} {}";

lazy_static! {
    static ref PROPAGATION: HashMap<&'static str, MsFlags> = {
        let mut m = HashMap::new();
        m.insert("shared", MsFlags::MS_SHARED | MsFlags::MS_REC);
        m.insert("private", MsFlags::MS_PRIVATE | MsFlags::MS_REC);
        m.insert("slave", MsFlags::MS_SLAVE | MsFlags::MS_REC);
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
        m.insert("unbindable", (false, MsFlags::MS_UNBINDABLE));
        m.insert(
            "runbindable",
            (false, MsFlags::MS_UNBINDABLE | MsFlags::MS_REC),
        );
        m.insert("private", (false, MsFlags::MS_PRIVATE));
        m.insert("rprivate", (false, MsFlags::MS_PRIVATE | MsFlags::MS_REC));
        m.insert("shared", (false, MsFlags::MS_SHARED));
        m.insert("rshared", (false, MsFlags::MS_SHARED | MsFlags::MS_REC));
        m.insert("slave", (false, MsFlags::MS_SLAVE));
        m.insert("rslave", (false, MsFlags::MS_SLAVE | MsFlags::MS_REC));
        m.insert("relatime", (false, MsFlags::MS_RELATIME));
        m.insert("norelatime", (true, MsFlags::MS_RELATIME));
        m.insert("strictatime", (false, MsFlags::MS_STRICTATIME));
        m.insert("nostrictatime", (true, MsFlags::MS_STRICTATIME));
        m
    };
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

    let linux = spec.linux.as_ref().unwrap();
    let mut flags = MsFlags::MS_REC;
    match PROPAGATION.get(&linux.rootfs_propagation.as_str()) {
        Some(fl) => flags |= *fl,
        None => flags |= MsFlags::MS_SLAVE,
    }

    let rootfs = spec.root.as_ref().unwrap().path.as_str();
    let root = fs::canonicalize(rootfs)?;
    let rootfs = root.to_str().unwrap();

    mount::mount(None::<&str>, "/", None::<&str>, flags, None::<&str>)?;
    mount::mount(
        Some(rootfs),
        rootfs,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )?;

    for m in &spec.mounts {
        let (mut flags, data) = parse_mount(&m);
        if !m.destination.starts_with("/") || m.destination.contains("..") {
            return Err(ErrorKind::Nix(nix::Error::Sys(Errno::EINVAL)).into());
        }
        if m.r#type == "cgroup" {
            mount_cgroups(cfd_log, &m, rootfs, flags, &data, cpath, mounts)?;
        } else {
            if m.destination == "/dev" {
                flags &= !MsFlags::MS_RDONLY;
            }

            mount_from(cfd_log, &m, &rootfs, flags, &data, "")?;
        }
    }

    let olddir = unistd::getcwd()?;
    unistd::chdir(rootfs)?;

    default_symlinks()?;
    create_devices(&linux.devices, bind_device)?;
    ensure_ptmx()?;

    unistd::chdir(&olddir)?;

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
    // mount tmpfs
    let ctm = Mount {
        source: "tmpfs".to_string(),
        r#type: "tmpfs".to_string(),
        destination: m.destination.clone(),
        options: Vec::new(),
    };

    let cflags = MsFlags::MS_NOEXEC | MsFlags::MS_NOSUID | MsFlags::MS_NODEV;
    //  info!(logger, "tmpfs");
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
            match unix::fs::symlink(destination.as_str(), &src[1..]) {
                Err(e) => {
                    log_child!(
                        cfd_log,
                        "symlink: {} {} err: {}",
                        key,
                        destination.as_str(),
                        e.to_string()
                    );

                    return Err(e.into());
                }
                Ok(_) => {}
            }
        }
    }

    unistd::chdir(&olddir)?;

    if flags.contains(MsFlags::MS_RDONLY) {
        let dest = format!("{}{}", rootfs, m.destination.as_str());
        mount::mount(
            Some(dest.as_str()),
            dest.as_str(),
            None::<&str>,
            flags | MsFlags::MS_BIND | MsFlags::MS_REMOUNT,
            None::<&str>,
        )?;
    }

    Ok(())
}

pub fn pivot_rootfs<P: ?Sized + NixPath>(path: &P) -> Result<()> {
    let oldroot = fcntl::open("/", OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;
    defer!(unistd::close(oldroot).unwrap());
    let newroot = fcntl::open(path, OFlag::O_DIRECTORY | OFlag::O_RDONLY, Mode::empty())?;
    defer!(unistd::close(newroot).unwrap());
    unistd::pivot_root(path, path)?;
    mount::umount2("/", MntFlags::MNT_DETACH)?;
    unistd::fchdir(newroot)?;
    stat::umask(Mode::from_bits_truncate(0o022));
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
            return Err(ErrorKind::ErrorCode("failed to parse mount info file".to_string()).into());
        }
    }

    Ok(infos)
}

pub fn ms_move_root(rootfs: &str) -> Result<bool> {
    unistd::chdir(rootfs)?;
    let mount_infos = parse_mount_table()?;

    let root_path = Path::new(rootfs);
    let abs_root_buf = root_path.absolutize()?;
    let abs_root = abs_root_buf.to_str().ok_or::<Error>(
        ErrorKind::ErrorCode(format!("failed to parse {} to absolute path", rootfs)).into(),
    )?;

    for info in mount_infos.iter() {
        let mount_point = Path::new(&info.mount_point);
        let abs_mount_buf = mount_point.absolutize()?;
        let abs_mount_point = abs_mount_buf.to_str().ok_or::<Error>(
            ErrorKind::ErrorCode(format!(
                "failed to parse {} to absolute path",
                info.mount_point
            ))
            .into(),
        )?;
        let abs_mount_point_string = String::from(abs_mount_point);

        // Umount every syfs and proc file systems, except those under the container rootfs
        if (info.fstype != "proc" && info.fstype != "sysfs")
            || abs_mount_point_string.starts_with(abs_root)
        {
            continue;
        }

        // Be sure umount events are not propagated to the host.
        mount::mount(
            None::<&str>,
            abs_mount_point,
            None::<&str>,
            MsFlags::MS_SLAVE | MsFlags::MS_REC,
            None::<&str>,
        )?;
        match mount::umount2(abs_mount_point, MntFlags::MNT_DETACH) {
            Ok(_) => (),
            Err(e) => {
                if e.ne(&nix::Error::from(Errno::EINVAL)) && e.ne(&nix::Error::from(Errno::EPERM)) {
                    return Err(ErrorKind::ErrorCode(e.to_string()).into());
                }

                // If we have not privileges for umounting (e.g. rootless), then
                // cover the path.
                mount::mount(
                    Some("tmpfs"),
                    abs_mount_point,
                    Some("tmpfs"),
                    MsFlags::empty(),
                    None::<&str>,
                )?;
            }
        }
    }

    mount::mount(
        Some(abs_root),
        "/",
        None::<&str>,
        MsFlags::MS_MOVE,
        None::<&str>,
    )?;
    unistd::chroot(".")?;
    unistd::chdir("/")?;

    Ok(true)
}

fn parse_mount(m: &Mount) -> (MsFlags, String) {
    let mut flags = MsFlags::empty();
    let mut data = Vec::new();

    for o in &m.options {
        match OPTIONS.get(o.as_str()) {
            Some(v) => {
                let (clear, fl) = *v;
                if clear {
                    flags &= !fl;
                } else {
                    flags |= fl;
                }
            }

            None => data.push(o.clone()),
        }
    }

    (flags, data.join(","))
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
    let dest = format!("{}{}", rootfs, &m.destination);

    let src = if m.r#type.as_str() == "bind" {
        let src = fs::canonicalize(m.source.as_str())?;
        let dir = if src.is_file() {
            Path::new(&dest).parent().unwrap()
        } else {
            Path::new(&dest)
        };

        // let _ = fs::create_dir_all(&dir);
        match fs::create_dir_all(&dir) {
            Ok(_) => {}
            Err(e) => {
                log_child!(
                    cfd_log,
                    "creat dir {}: {}",
                    dir.to_str().unwrap(),
                    e.to_string()
                );
            }
        }

        // make sure file exists so we can bind over it
        if src.is_file() {
            let _ = OpenOptions::new().create(true).write(true).open(&dest);
        }
        src
    } else {
        let _ = fs::create_dir_all(&dest);
        PathBuf::from(&m.source)
    };

    // ignore this check since some mount's src didn't been a directory
    // such as tmpfs.
    /*
        match stat::stat(src.to_str().unwrap()) {
            Ok(_) => {}
            Err(e) => {
                info!("{}: {}", src.to_str().unwrap(), e.as_errno().unwrap().desc());
            }
        }
    */

    match stat::stat(dest.as_str()) {
        Ok(_) => {}
        Err(e) => {
            log_child!(
                cfd_log,
                "{}: {}",
                dest.as_str(),
                e.as_errno().unwrap().desc()
            );
        }
    }

    match mount::mount(
        Some(src.to_str().unwrap()),
        dest.as_str(),
        Some(m.r#type.as_str()),
        flags,
        Some(d.as_str()),
    ) {
        Ok(_) => {}
        Err(e) => {
            log_child!(cfd_log, "mount error: {}", e.as_errno().unwrap().desc());
            return Err(e.into());
        }
    }

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
        match mount::mount(
            Some(dest.as_str()),
            dest.as_str(),
            None::<&str>,
            flags | MsFlags::MS_REMOUNT,
            None::<&str>,
        ) {
            Err(e) => {
                log_child!(
                    cfd_log,
                    "remout {}: {}",
                    dest.as_str(),
                    e.as_errno().unwrap().desc()
                );
                return Err(e.into());
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

static SYMLINKS: &'static [(&'static str, &'static str)] = &[
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
            bail!(ErrorKind::ErrorCode(msg));
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

fn makedev(major: u64, minor: u64) -> u64 {
    (minor & 0xff) | ((major & 0xfff) << 8) | ((minor & !0xff) << 12) | ((major & !0xfff) << 32)
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
        None => return Err(ErrorKind::ErrorCode("invalid spec".to_string()).into()),
    };

    stat::mknod(
        &dev.path[1..],
        *f,
        Mode::from_bits_truncate(dev.file_mode.unwrap_or(0)),
        makedev(dev.major as u64, dev.minor as u64),
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

    mount::mount(
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
            let (flags, _) = parse_mount(m);
            if flags.contains(MsFlags::MS_RDONLY) {
                mount::mount(
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

        mount::mount(Some("/"), "/", None::<&str>, flags, None::<&str>)?;
    }
    stat::umask(Mode::from_bits_truncate(0o022));
    unistd::chdir(&olddir)?;

    Ok(())
}

fn mask_path(path: &str) -> Result<()> {
    if !path.starts_with("/") || path.contains("..") {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }

    //info!("{}", path);

    match mount::mount(
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
            //info!("{}: {}", path, e.as_errno().unwrap().desc());
            return Err(e.into());
        }

        Ok(_) => {}
    }

    Ok(())
}

fn readonly_path(path: &str) -> Result<()> {
    if !path.starts_with("/") || path.contains("..") {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    }

    //info!("{}", path);

    match mount::mount(
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
            //info!("{}: {}", path, e.as_errno().unwrap().desc());
            return Err(e.into());
        }

        Ok(_) => {}
    }

    mount::mount(
        Some(&path[1..]),
        &path[1..],
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT,
        None::<&str>,
    )?;

    Ok(())
}
