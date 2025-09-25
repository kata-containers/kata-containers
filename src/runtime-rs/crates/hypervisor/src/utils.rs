// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::HashSet,
    fs::{metadata, set_permissions, File, OpenOptions, Permissions},
    io,
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{anyhow, Context, Result};
use kata_types::{
    build_path,
    config::{Hypervisor, KATA_PATH},
};
use nix::{
    fcntl,
    sched::{setns, CloneFlags},
    sys::stat,
    unistd::{chown, setgroups, Gid, Uid},
};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use serde_json;

use crate::device::Tap;

use crate::{DEFAULT_HYBRID_VSOCK_NAME, JAILER_ROOT};

pub fn get_child_threads(pid: u32) -> HashSet<u32> {
    let mut result = HashSet::new();
    let path_name = format!("/proc/{}/task", pid);
    let path = std::path::Path::new(path_name.as_str());
    if path.is_dir() {
        if let Ok(dir) = path.read_dir() {
            for entity in dir {
                if let Ok(entity) = entity.as_ref() {
                    let file_name = entity.file_name();
                    let file_name = file_name.to_str().unwrap_or_default();
                    if let Ok(tid) = file_name.parse::<u32>() {
                        result.insert(tid);
                    }
                }
            }
        }
    }
    result
}

// Return the path for a _hypothetical_ sandbox: the path does *not* exist
// yet, and for this reason safe-path cannot be used.
pub fn get_sandbox_path(sid: &str) -> String {
    Path::new(build_path(KATA_PATH).as_str())
        .join(sid)
        .to_string_lossy()
        .to_string()
}

pub fn get_hvsock_path(sid: &str) -> String {
    let jailer_root_path = get_jailer_root(sid);

    [jailer_root_path, DEFAULT_HYBRID_VSOCK_NAME.to_owned()].join("/")
}

pub fn get_jailer_root(sid: &str) -> String {
    let sandbox_path = get_sandbox_path(sid);

    [&sandbox_path, JAILER_ROOT].join("/")
}

// Clear the O_CLOEXEC which is set by default by Rust standard library on
// file descriptors that it opens.  This function is mostly meant to be
// called on descriptors to be passed to a child (hypervisor) process as
// O_CLOEXEC would obviously prevent that.
pub fn clear_cloexec(rawfd: RawFd) -> Result<()> {
    let cur_flags = fcntl::fcntl(rawfd, fcntl::FcntlArg::F_GETFD)?;
    let mut new_flags = fcntl::FdFlag::from_bits(cur_flags).ok_or(anyhow!(
        "couldn't construct FdFlag from flags value {:?}",
        cur_flags
    ))?;
    new_flags.remove(fcntl::FdFlag::FD_CLOEXEC);
    if let Err(err) = fcntl::fcntl(rawfd, fcntl::FcntlArg::F_SETFD(new_flags)) {
        info!(sl!(), "couldn't clear O_CLOEXEC on fd: {:?}", err);
        return Err(err.into());
    }

    Ok(())
}

pub fn enter_netns(netns_path: &str) -> Result<()> {
    if !netns_path.is_empty() {
        let netns =
            File::open(netns_path).context(anyhow!("open netns path {:?} failed.", netns_path))?;
        setns(netns.as_raw_fd(), CloneFlags::CLONE_NEWNET).context("set netns failed")?;
    }

    Ok(())
}

pub fn set_groups(groups: &[u32]) -> Result<()> {
    if !groups.is_empty() {
        let group = groups
            .iter()
            .map(|gid| Gid::from_raw(*gid))
            .collect::<Vec<_>>();
        setgroups(&group).context("set groups failed")?;
    }

    Ok(())
}

pub fn open_named_tuntap(if_name: &str, queues: u32) -> Result<Vec<File>> {
    let (multi_vq, vq_pairs) = if queues > 1 {
        (true, queues as usize)
    } else {
        (false, 1_usize)
    };

    let tap: Tap = Tap::open_named(if_name, multi_vq).context("open named tuntap device failed")?;
    let taps: Vec<Tap> = tap.into_mq_taps(vq_pairs).context("into mq taps failed.")?;

    let mut tap_files: Vec<std::fs::File> = Vec::new();
    for tap in taps {
        tap_files.push(tap.tap_file);
    }

    Ok(tap_files)
}

// /dev/tap$(cat /sys/class/net/macvtap1/ifindex)
// for example: /dev/tap2381
#[allow(dead_code)]
pub fn create_macvtap_fds(ifindex: u32, queues: u32) -> Result<Vec<File>> {
    let macvtap = format!("/dev/tap{}", ifindex);
    create_fds(macvtap.as_str(), queues as usize)
}

pub fn create_vhost_net_fds(queues: u32) -> Result<Vec<File>> {
    let vhost_dev = "/dev/vhost-net";
    let num_fds = if queues > 1 { queues as usize } else { 1_usize };

    create_fds(vhost_dev, num_fds)
}

// For example: if num_fds = 3; fds = {0xc000012028, 0xc000012030, 0xc000012038}
fn create_fds(device: &str, num_fds: usize) -> Result<Vec<File>> {
    let mut fds: Vec<File> = Vec::with_capacity(num_fds);

    for i in 0..num_fds {
        match OpenOptions::new().read(true).write(true).open(device) {
            Ok(f) => {
                fds.push(f);
            }
            Err(e) => {
                fds.clear();
                return Err(anyhow!(
                    "It failed with error {:?} when opened the {:?} device.",
                    e,
                    i
                ));
            }
        };
    }

    Ok(fds)
}

pub fn create_dir_all_with_inherit_owner<P: AsRef<Path>>(path: P, perm: u32) -> io::Result<()> {
    let path = path.as_ref();

    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the path must be absolute",
        ));
    }

    let mut uid = Uid::current();
    let mut gid = Gid::current();

    let mut current_path = PathBuf::new();

    for p in path.components() {
        current_path.push(p);
        let current = current_path.as_path();

        match stat::stat(current) {
            Ok(s) => {
                if !current.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        format!("{} exists but is not a directory", current.display()),
                    ));
                }
                uid = Uid::from_raw(s.st_uid);
                gid = Gid::from_raw(s.st_gid);
            }
            Err(nix::Error::ENOENT) => {
                std::fs::create_dir(current)?;
                set_permissions(current, Permissions::from_mode(perm))?;
                chown(current, Some(uid), Some(gid))?;
            }
            Err(e) => {
                return Err(io::Error::from_raw_os_error(e as i32));
            }
        }
    }

    Ok(())
}

/// chown_to_parent changes the owners of the path to the same of parent directory.
pub fn chown_to_parent<P: AsRef<Path>>(path: P) -> io::Result<()> {
    // This relies on the process's current working directory to resolve relative paths.
    let path = std::path::absolute(path.as_ref())?;

    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no parent directory"))?;

    let st = stat::stat(parent).map_err(|e| io::Error::from_raw_os_error(e as i32))?;
    let uid = Uid::from_raw(st.st_uid);
    let gid = Gid::from_raw(st.st_gid);

    chown(&path, Some(uid), Some(gid)).map_err(|e| io::Error::from_raw_os_error(e as i32))
}

fn first_valid_executable_path(paths: &[&str]) -> Result<String> {
    for p in paths {
        if let Ok(m) = metadata(p) {
            if m.is_file() && m.mode() & 0o111 != 0 {
                return Ok(p.to_string());
            }
        }
    }
    Err(anyhow!("No valid executable found in paths: {:?}", paths))
}

pub fn create_vmm_user() -> Result<String> {
    let useradd_path =
        first_valid_executable_path(&["/usr/sbin/useradd", "/sbin/useradd", "/bin/useradd"])?;
    let nologin_path =
        first_valid_executable_path(&["/usr/sbin/nologin", "/sbin/nologin", "/bin/nologin"])?;

    let max_attempt = 5;
    for _ in 0..max_attempt {
        let user_name = format!("kata-{}", thread_rng().gen_range(0..10000));
        let status = Command::new(&useradd_path)
            .arg("-M")
            .arg("-s")
            .arg(&nologin_path)
            .arg(&user_name)
            .arg("-c")
            .arg("\"Kata Containers temporary hypervisor user\"")
            .status()?;
        if status.success() {
            return Ok(user_name);
        }
    }
    Err(anyhow!("could not create VMM user"))
}

pub fn remove_vmm_user(user: &str) -> Result<()> {
    let userdel_path =
        first_valid_executable_path(&["/usr/sbin/userdel", "/sbin/userdel", "/bin/userdel"])?;

    for _ in 0..5 {
        let status = Command::new(&userdel_path).arg("-r").arg(user).status()?;
        if status.success() {
            return Ok(());
        }
    }
    Err(anyhow!("failed to remove VMM user"))
}

pub fn vm_cleanup(config: &Hypervisor, vm_path: &str) -> Result<()> {
    std::fs::remove_dir_all(vm_path)?;
    if kata_types::rootless::is_rootless() {
        let user = &config
            .security_info
            .rootless_user
            .as_ref()
            .ok_or_else(|| anyhow!("rootless user not specified in security_info"))?
            .user_name;
        match nix::unistd::User::from_name(user)? {
            Some(_) => {
                remove_vmm_user(user)?;
            }
            None => {
                error!(
                    sl!(),
                    "failed to find user: {}, it might have been removed", user
                );
            }
        }
    }
    Ok(())
}

// QGS_SOCKET_PATH: the Unix Domain Socket Path served by Intel TDX Quote Generation Service
const QGS_SOCKET_PATH: &str = "/var/run/tdx-qgs/qgs.socket";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocketAddress {
    #[serde(rename = "type")]
    pub typ: String,

    #[serde(rename = "cid", skip_serializing_if = "String::is_empty")]
    pub cid: String,

    #[serde(rename = "port", skip_serializing_if = "String::is_empty")]
    pub port: String,

    #[serde(rename = "path", skip_serializing_if = "String::is_empty")]
    pub path: String,
}

impl SocketAddress {
    pub fn new(port: u32) -> Self {
        if port == 0 {
            Self {
                typ: "unix".to_string(),
                cid: "".to_string(),
                port: "".to_string(),
                path: QGS_SOCKET_PATH.to_string(),
            }
        } else {
            Self {
                typ: "vsock".to_string(),
                cid: format!("{}", 2),
                port: port.to_string(),
                path: "".to_string(),
            }
        }
    }
}

impl std::fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)
            .and_then(|s| write!(f, "{}", s))
    }
}

pub fn bytes_to_megs(bytes: u64) -> u32 {
    (bytes / (1 << 20)) as u32
}

pub fn megs_to_bytes(bytes: u32) -> u64 {
    bytes as u64 * (1 << 20)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use nix::unistd::chown;
    use nix::unistd::geteuid;
    use nix::unistd::Gid;
    use nix::unistd::Uid;
    use tempfile::Builder;
    use tempfile::TempDir;

    use crate::utils::create_dir_all_with_inherit_owner;
    use crate::utils::first_valid_executable_path;

    use super::create_fds;
    use super::SocketAddress;

    #[test]
    fn test_ctreate_fds() {
        let device = "/dev/null";
        let num_fds = 3_usize;
        let fds = create_fds(device, num_fds);
        assert!(fds.is_ok());
        assert_eq!(fds.unwrap().len(), num_fds);
    }

    #[test]
    fn test_vsocket_address_new() {
        let socket = SocketAddress::new(8866);
        assert_eq!(socket.typ, "vsock");
        assert_eq!(socket.cid, "2");
        assert_eq!(socket.port, "8866");
    }

    #[test]
    fn test_unix_address_new() {
        let socket = SocketAddress::new(0);
        assert_eq!(socket.typ, "unix");
        assert_eq!(socket.path, "/var/run/tdx-qgs/qgs.socket");
    }

    #[test]
    fn test_socket_address_display() {
        let socket = SocketAddress::new(6688);
        let expected_json = r#"{"type":"vsock","cid":"2","port":"6688"}"#;
        assert_eq!(format!("{}", socket), expected_json);
    }

    #[test]
    fn test_socket_address_serialize_deserialize() {
        let socket = SocketAddress::new(0);
        let serialized = serde_json::to_string(&socket).unwrap();
        let expected_json = r#"{"type":"unix","path":"/var/run/tdx-qgs/qgs.socket"}"#;
        assert_eq!(expected_json, serialized);
    }

    #[test]
    fn test_socket_address_kebab_case() {
        let socket = SocketAddress::new(6868);
        let serialized = serde_json::to_string(&socket).unwrap();
        assert!(serialized.contains(r#""type":"#));
        assert!(serialized.contains(r#""cid":"#));
        assert!(serialized.contains(r#""port":"#));
    }

    #[test]
    fn test_mkdir_all_with_inherited_owner_successful() {
        if geteuid().as_raw() != 0 {
            eprintln!("skipped: requires root");
            return;
        }

        let tmp1 = TempDir::new().expect("create tmp1");
        let tmp1_path = tmp1.path().to_path_buf();

        chown(
            &tmp1_path,
            Some(Uid::from_raw(1234)),
            Some(Gid::from_raw(5678)),
        )
        .expect("chown tmp1");

        let target1 = tmp1_path.join("foo").join("bar");
        create_dir_all_with_inherit_owner(&target1, 0o700).expect("mkdir -p target1");

        let temp_root = std::env::temp_dir();
        assert!(
            target1.starts_with(&temp_root),
            "target1 not under temp dir"
        );

        let mut chain: Vec<PathBuf> = target1.ancestors().map(|p| p.to_path_buf()).collect();
        chain.reverse();

        let start = chain
            .iter()
            .position(|p| p == &temp_root)
            .map(|i| i + 1)
            .unwrap_or(1);

        for p in chain.into_iter().skip(start) {
            let md = fs::metadata(&p).expect("stat p");
            assert!(md.is_dir(), "not a dir: {}", p.display());
            assert_eq!(md.uid(), 1234, "uid mismatch for {}", p.display());
            assert_eq!(md.gid(), 5678, "gid mismatch for {}", p.display());
        }

        let tmp2 = TempDir::new().expect("create tmp2");
        let tmp2_path = tmp2.into_path();
        let _ = fs::remove_dir_all(&tmp2_path);

        let target2 = tmp2_path.join("foo").join("bar");
        create_dir_all_with_inherit_owner(&target2, 0o700).expect("mkdir -p target2");

        let temp_root = std::env::temp_dir();
        assert!(
            target2.starts_with(&temp_root),
            "target2 not under temp dir"
        );

        let mut chain: Vec<PathBuf> = target2.ancestors().map(|p| p.to_path_buf()).collect();
        chain.reverse();

        let start = chain
            .iter()
            .position(|p| p == &temp_root)
            .map(|i| i + 1)
            .unwrap_or(1);

        for p in chain.into_iter().skip(start) {
            let md = fs::metadata(&p).expect("stat p");
            assert!(md.is_dir(), "not a dir: {}", p.display());
            assert_eq!(md.uid(), 0, "uid mismatch for {}", p.display());
            assert_eq!(md.gid(), 0, "gid mismatch for {}", p.display());
        }

        let _ = fs::remove_dir_all(&tmp2_path);
    }

    #[test]
    fn test_chown_to_parent() {
        if geteuid().as_raw() != 0 {
            eprintln!("skipped: requires root");
            return;
        }

        let tmp = Builder::new()
            .prefix("root")
            .tempdir()
            .expect("create temp dir");
        let root_dir = tmp.path();

        chown(
            root_dir,
            Some(Uid::from_raw(1234)),
            Some(Gid::from_raw(5678)),
        )
        .expect("chown root_dir");

        let target_dir = root_dir.join("foo");
        fs::create_dir_all(&target_dir).expect("mkdir -p target_dir");

        super::chown_to_parent(&target_dir).expect("chown_to_parent");

        let md = fs::metadata(&target_dir).expect("stat target_dir");
        assert_eq!(md.uid(), 1234, "uid mismatch");
        assert_eq!(md.gid(), 5678, "gid mismatch");
    }

    #[test]
    fn test_first_valid_executable_path() {
        let tmp = TempDir::new().expect("create tmpdir");
        let tmpdir = tmp.path();

        {
            let paths: Vec<String> = vec!["a/b/c".to_string(), "c/d".to_string()];
            let slice: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

            let err = first_valid_executable_path(&slice).unwrap_err();
            let expected = format!("No valid executable found in paths: {:?}", &slice);
            assert_eq!(
                err.to_string(),
                expected,
                "all invalid: error message mismatch"
            );
        }

        {
            let ab = tmpdir.join("a").join("b");
            fs::create_dir_all(&ab).expect("mkdir -p a/b");

            let c = ab.join("c");
            fs::write(&c, b"test\n").expect("write c");
            let mut perm = fs::metadata(&c).unwrap().permissions();
            perm.set_mode(0o644);
            fs::set_permissions(&c, perm).unwrap();

            let paths: Vec<String> = vec![c.to_string_lossy().into_owned(), "c/d".to_string()];
            let slice: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

            let err = first_valid_executable_path(&slice).unwrap_err();
            let expected = format!("No valid executable found in paths: {:?}", &slice);
            assert_eq!(err.to_string(), expected, "non-exec file should be invalid");
        }

        {
            let de = tmpdir.join("d").join("e");
            fs::create_dir_all(&de).expect("mkdir -p d/e");

            let f = de.join("f");
            fs::write(&f, b"test\n").expect("write f");
            let mut perm = fs::metadata(&f).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&f, perm).unwrap();

            let expect_path = f.to_string_lossy().into_owned();
            let paths: Vec<String> = vec![expect_path.clone(), "c/d".to_string()];
            let slice: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

            let got = first_valid_executable_path(&slice).expect("should find executable");
            assert_eq!(got, expect_path, "should return the first executable path");
        }

        {
            let gh = tmpdir.join("g").join("h");
            fs::create_dir_all(&gh).expect("mkdir -p g/h");

            let i = gh.join("i");
            fs::write(&i, b"test\n").expect("write i");
            let mut perm = fs::metadata(&i).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&i, perm).unwrap();

            let expect_path = i.to_string_lossy().into_owned();
            let paths: Vec<String> = vec!["c/d".to_string(), expect_path.clone()];
            let slice: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();

            let got = first_valid_executable_path(&slice).expect("should find executable");
            assert_eq!(got, expect_path, "should return the second executable path");
        }
    }
}
