// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use rustjail::errors::*;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs;
use std::io;
use std::iter::FromIterator;
use std::os::unix::fs::PermissionsExt;

use std::path::Path;
use std::ptr::null;
use std::sync::{Arc, Mutex};

use libc::{c_void, mount};
use nix::mount::{self, MsFlags};

use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::device::{get_pci_device_name, get_scsi_device_name, online_device};
use crate::protocols::agent::Storage;
use crate::Sandbox;
use slog::Logger;

const DRIVER9PTYPE: &'static str = "9p";
const DRIVERVIRTIOFSTYPE: &'static str = "virtio-fs";
pub const DRIVERBLKTYPE: &'static str = "blk";
pub const DRIVERMMIOBLKTYPE: &'static str = "mmioblk";
pub const DRIVERSCSITYPE: &'static str = "scsi";
pub const DRIVERNVDIMMTYPE: &'static str = "nvdimm";
const DRIVEREPHEMERALTYPE: &'static str = "ephemeral";
const DRIVERLOCALTYPE: &'static str = "local";

pub const TYPEROOTFS: &'static str = "rootfs";

pub const PROCMOUNTSTATS: &'static str = "/proc/self/mountstats";

const ROOTBUSPATH: &'static str = "/devices/pci0000:00";

const CGROUPPATH: &'static str = "/sys/fs/cgroup";
const PROCCGROUPS: &'static str = "/proc/cgroups";

pub const TIMEOUT_HOTPLUG: u64 = 3;

#[cfg_attr(rustfmt, rustfmt_skip)]
lazy_static! {
    pub static ref FLAGS: HashMap<&'static str, (bool, MsFlags)> = {
        let mut m = HashMap::new();
        m.insert("defaults",      (false, MsFlags::empty()));
        m.insert("ro",            (false, MsFlags::MS_RDONLY));
        m.insert("rw",            (true,  MsFlags::MS_RDONLY));
        m.insert("suid",          (true,  MsFlags::MS_NOSUID));
        m.insert("nosuid",        (false, MsFlags::MS_NOSUID));
        m.insert("dev",           (true,  MsFlags::MS_NODEV));
        m.insert("nodev",         (false, MsFlags::MS_NODEV));
        m.insert("exec",          (true,  MsFlags::MS_NOEXEC));
        m.insert("noexec",        (false, MsFlags::MS_NOEXEC));
        m.insert("sync",          (false, MsFlags::MS_SYNCHRONOUS));
        m.insert("async",         (true,  MsFlags::MS_SYNCHRONOUS));
        m.insert("dirsync",       (false, MsFlags::MS_DIRSYNC));
        m.insert("remount",       (false, MsFlags::MS_REMOUNT));
        m.insert("mand",          (false, MsFlags::MS_MANDLOCK));
        m.insert("nomand",        (true,  MsFlags::MS_MANDLOCK));
        m.insert("atime",         (true,  MsFlags::MS_NOATIME));
        m.insert("noatime",       (false, MsFlags::MS_NOATIME));
        m.insert("diratime",      (true,  MsFlags::MS_NODIRATIME));
        m.insert("nodiratime",    (false, MsFlags::MS_NODIRATIME));
        m.insert("bind",          (false, MsFlags::MS_BIND));
        m.insert("rbind",         (false, MsFlags::MS_BIND | MsFlags::MS_REC));
        m.insert("unbindable",    (false, MsFlags::MS_UNBINDABLE));
        m.insert("runbindable",   (false, MsFlags::MS_UNBINDABLE | MsFlags::MS_REC));
        m.insert("private",       (false, MsFlags::MS_PRIVATE));
        m.insert("rprivate",      (false, MsFlags::MS_PRIVATE | MsFlags::MS_REC));
        m.insert("shared",        (false, MsFlags::MS_SHARED));
        m.insert("rshared",       (false, MsFlags::MS_SHARED | MsFlags::MS_REC));
        m.insert("slave",         (false, MsFlags::MS_SLAVE));
        m.insert("rslave",        (false, MsFlags::MS_SLAVE | MsFlags::MS_REC));
        m.insert("relatime",      (false, MsFlags::MS_RELATIME));
        m.insert("norelatime",    (true,  MsFlags::MS_RELATIME));
        m.insert("strictatime",   (false, MsFlags::MS_STRICTATIME));
        m.insert("nostrictatime", (true,  MsFlags::MS_STRICTATIME));
        m
    };
}

#[derive(Debug, PartialEq)]
pub struct INIT_MOUNT {
    fstype: &'static str,
    src: &'static str,
    dest: &'static str,
    options: Vec<&'static str>,
}

#[cfg_attr(rustfmt, rustfmt_skip)]
lazy_static!{
    static ref CGROUPS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("cpu", "/sys/fs/cgroup/cpu");
        m.insert("cpuacct", "/sys/fs/cgroup/cpuacct");
        m.insert("blkio", "/sys/fs/cgroup/blkio");
        m.insert("cpuset", "/sys/fs/cgroup/cpuset");
        m.insert("memory", "/sys/fs/cgroup/memory");
        m.insert("devices", "/sys/fs/cgroup/devices");
        m.insert("freezer", "/sys/fs/cgroup/freezer");
        m.insert("net_cls", "/sys/fs/cgroup/net_cls");
        m.insert("perf_event", "/sys/fs/cgroup/perf_event");
        m.insert("net_prio", "/sys/fs/cgroup/net_prio");
        m.insert("hugetlb", "/sys/fs/cgroup/hugetlb");
        m.insert("pids", "/sys/fs/cgroup/pids");
        m.insert("rdma", "/sys/fs/cgroup/rdma");
        m
    };
}

#[cfg_attr(rustfmt, rustfmt_skip)]
lazy_static! {
    pub static ref INIT_ROOTFS_MOUNTS: Vec<INIT_MOUNT> = vec![
        INIT_MOUNT{fstype: "proc", src: "proc", dest: "/proc", options: vec!["nosuid", "nodev", "noexec"]},
        INIT_MOUNT{fstype: "sysfs", src: "sysfs", dest: "/sys", options: vec!["nosuid", "nodev", "noexec"]},
        INIT_MOUNT{fstype: "devtmpfs", src: "dev", dest: "/dev", options: vec!["nosuid"]},
        INIT_MOUNT{fstype: "tmpfs", src: "tmpfs", dest: "/dev/shm", options: vec!["nosuid", "nodev"]},
        INIT_MOUNT{fstype: "devpts", src: "devpts", dest: "/dev/pts", options: vec!["nosuid", "noexec"]},
        INIT_MOUNT{fstype: "tmpfs", src: "tmpfs", dest: "/run", options: vec!["nosuid", "nodev"]},
    ];
}

// StorageHandler is the type of callback to be defined to handle every
// type of storage driver.
type StorageHandler = fn(&Logger, &Storage, Arc<Mutex<Sandbox>>) -> Result<String>;

// StorageHandlerList lists the supported drivers.
#[cfg_attr(rustfmt, rustfmt_skip)]
lazy_static! {
    pub static ref STORAGEHANDLERLIST: HashMap<&'static str, StorageHandler> = {
    	let mut m = HashMap::new();
    let blk: StorageHandler = virtio_blk_storage_handler;
        m.insert(DRIVERBLKTYPE, blk);
	let p9: StorageHandler= virtio9p_storage_handler;
        m.insert(DRIVER9PTYPE, p9);
	let virtiofs: StorageHandler = virtiofs_storage_handler;
        m.insert(DRIVERVIRTIOFSTYPE, virtiofs);
    let ephemeral: StorageHandler = ephemeral_storage_handler;
        m.insert(DRIVEREPHEMERALTYPE, ephemeral);
    let virtiommio: StorageHandler = virtiommio_blk_storage_handler;
        m.insert(DRIVERMMIOBLKTYPE, virtiommio);
    let local: StorageHandler = local_storage_handler;
        m.insert(DRIVERLOCALTYPE, local);
    let scsi: StorageHandler = virtio_scsi_storage_handler;
        m.insert(DRIVERSCSITYPE, scsi);
        m
    };
}

#[derive(Debug, Clone)]
pub struct BareMount<'a> {
    source: &'a str,
    destination: &'a str,
    fs_type: &'a str,
    flags: MsFlags,
    options: &'a str,
    logger: Logger,
}

// mount mounts a source in to a destination. This will do some bookkeeping:
// * evaluate all symlinks
// * ensure the source exists
impl<'a> BareMount<'a> {
    pub fn new(
        s: &'a str,
        d: &'a str,
        fs_type: &'a str,
        flags: MsFlags,
        options: &'a str,
        logger: &Logger,
    ) -> Self {
        BareMount {
            source: s,
            destination: d,
            fs_type: fs_type,
            flags: flags,
            options: options,
            logger: logger.new(o!("subsystem" => "baremount")),
        }
    }

    pub fn mount(&self) -> Result<()> {
        let source;
        let dest;
        let fs_type;
        let mut options = null();
        let cstr_options: CString;
        let cstr_source: CString;
        let cstr_dest: CString;
        let cstr_fs_type: CString;

        if self.source.len() == 0 {
            return Err(ErrorKind::ErrorCode("need mount source".to_string()).into());
        }

        if self.destination.len() == 0 {
            return Err(ErrorKind::ErrorCode("need mount destination".to_string()).into());
        }

        cstr_source = CString::new(self.source)?;
        source = cstr_source.as_ptr();

        cstr_dest = CString::new(self.destination)?;
        dest = cstr_dest.as_ptr();

        if self.fs_type.len() == 0 {
            return Err(ErrorKind::ErrorCode("need mount FS type".to_string()).into());
        }

        cstr_fs_type = CString::new(self.fs_type)?;
        fs_type = cstr_fs_type.as_ptr();

        if self.options.len() > 0 {
            cstr_options = CString::new(self.options)?;
            options = cstr_options.as_ptr() as *const c_void;
        }

        info!(
            self.logger,
            "mount source={:?}, dest={:?}, fs_type={:?}, options={:?}",
            self.source,
            self.destination,
            self.fs_type,
            self.options
        );
        let rc = unsafe { mount(source, dest, fs_type, self.flags.bits(), options) };

        if rc < 0 {
            return Err(ErrorKind::ErrorCode(format!(
                "failed to mount {:?} to {:?}, with error: {}",
                self.source,
                self.destination,
                io::Error::last_os_error()
            ))
            .into());
        }
        Ok(())
    }
}

fn ephemeral_storage_handler(
    logger: &Logger,
    storage: &Storage,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<String> {
    let s = sandbox.clone();
    let mut sb = s.lock().unwrap();
    let new_storage = sb.set_sandbox_storage(&storage.mount_point);

    if !new_storage {
        return Ok("".to_string());
    }

    if let Err(err) = fs::create_dir_all(Path::new(&storage.mount_point)) {
        return Err(err.into());
    }

    common_storage_handler(logger, storage)
}

fn local_storage_handler(
    _logger: &Logger,
    storage: &Storage,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<String> {
    let s = sandbox.clone();
    let mut sb = s.lock().unwrap();
    let new_storage = sb.set_sandbox_storage(&storage.mount_point);

    if !new_storage {
        return Ok("".to_string());
    }

    fs::create_dir_all(&storage.mount_point)?;

    let opts_vec: Vec<String> = storage.options.to_vec();

    let opts = parse_options(opts_vec);
    let mode = opts.get("mode");
    if mode.is_some() {
        let mode = mode.unwrap();
        let mut permission = fs::metadata(&storage.mount_point)?.permissions();

        let o_mode = u32::from_str_radix(mode, 8)?;
        permission.set_mode(o_mode);

        fs::set_permissions(&storage.mount_point, permission)?;
    }

    Ok("".to_string())
}

fn virtio9p_storage_handler(
    logger: &Logger,
    storage: &Storage,
    _sandbox: Arc<Mutex<Sandbox>>,
) -> Result<String> {
    common_storage_handler(logger, storage)
}

// virtiommio_blk_storage_handler handles the storage for mmio blk driver.
fn virtiommio_blk_storage_handler(
    logger: &Logger,
    storage: &Storage,
    _sandbox: Arc<Mutex<Sandbox>>,
) -> Result<String> {
    //The source path is VmPath
    common_storage_handler(logger, storage)
}

// virtiofs_storage_handler handles the storage for virtio-fs.
fn virtiofs_storage_handler(
    logger: &Logger,
    storage: &Storage,
    _sandbox: Arc<Mutex<Sandbox>>,
) -> Result<String> {
    common_storage_handler(logger, storage)
}

// virtio_blk_storage_handler handles the storage for blk driver.
fn virtio_blk_storage_handler(
    logger: &Logger,
    storage: &Storage,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<String> {
    let mut storage = storage.clone();
    // If hot-plugged, get the device node path based on the PCI address else
    // use the virt path provided in Storage Source
    if storage.source.starts_with("/dev") {
        let metadata = fs::metadata(&storage.source)?;

        let mode = metadata.permissions().mode();
        if mode & libc::S_IFBLK == 0 {
            return Err(ErrorKind::ErrorCode(format!("Invalid device {}", &storage.source)).into());
        }
    } else {
        let dev_path = get_pci_device_name(sandbox, &storage.source)?;
        storage.source = dev_path;
    }

    common_storage_handler(logger, &storage)
}

// virtio_scsi_storage_handler handles the storage for scsi driver.
fn virtio_scsi_storage_handler(
    logger: &Logger,
    storage: &Storage,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<String> {
    let mut storage = storage.clone();

    // Retrieve the device path from SCSI address.
    let dev_path = get_scsi_device_name(sandbox, &storage.source)?;
    storage.source = dev_path;

    common_storage_handler(logger, &storage)
}

fn common_storage_handler(logger: &Logger, storage: &Storage) -> Result<String> {
    // Mount the storage device.
    let mount_point = storage.mount_point.to_string();

    mount_storage(logger, storage).and(Ok(mount_point))
}

// mount_storage performs the mount described by the storage structure.
fn mount_storage(logger: &Logger, storage: &Storage) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "mount"));

    match storage.fstype.as_str() {
        DRIVER9PTYPE | DRIVERVIRTIOFSTYPE => {
            let dest_path = Path::new(storage.mount_point.as_str());
            if !dest_path.exists() {
                fs::create_dir_all(dest_path).chain_err(|| "Create mount destination failed")?;
            }
        }
        _ => {
            ensure_destination_exists(storage.mount_point.as_str(), storage.fstype.as_str())?;
        }
    }

    let options_vec = storage.options.to_vec();
    let options_vec = Vec::from_iter(options_vec.iter().map(String::as_str));
    let (flags, options) = parse_mount_flags_and_options(options_vec);

    info!(logger, "mounting storage";
    "mount-source:" => storage.source.as_str(),
    "mount-destination" => storage.mount_point.as_str(),
    "mount-fstype"  => storage.fstype.as_str(),
    "mount-options" => options.as_str(),
    );

    let bare_mount = BareMount::new(
        storage.source.as_str(),
        storage.mount_point.as_str(),
        storage.fstype.as_str(),
        flags,
        options.as_str(),
        &logger,
    );

    bare_mount.mount()
}

fn parse_mount_flags_and_options(options_vec: Vec<&str>) -> (MsFlags, String) {
    let mut flags = MsFlags::empty();
    let mut options: String = "".to_string();

    for opt in options_vec {
        if opt.len() != 0 {
            match FLAGS.get(opt) {
                Some(x) => {
                    let (_, f) = *x;
                    flags = flags | f;
                }
                None => {
                    if options.len() > 0 {
                        options.push_str(format!(",{}", opt).as_str());
                    } else {
                        options.push_str(format!("{}", opt).as_str());
                    }
                }
            };
        }
    }
    (flags, options)
}

// add_storages takes a list of storages passed by the caller, and perform the
// associated operations such as waiting for the device to show up, and mount
// it to a specific location, according to the type of handler chosen, and for
// each storage.
pub fn add_storages(
    logger: Logger,
    storages: Vec<Storage>,
    sandbox: Arc<Mutex<Sandbox>>,
) -> Result<Vec<String>> {
    let mut mount_list = Vec::new();

    for storage in storages {
        let handler_name = storage.driver.clone();
        let logger = logger.new(o!(
            "subsystem" => "storage",
            "storage-type" => handler_name.to_owned()));

        let handler = match STORAGEHANDLERLIST.get(&handler_name.as_str()) {
            None => {
                return Err(ErrorKind::ErrorCode(format!(
                    "Failed to find the storage handler {}",
                    storage.driver.to_owned()
                ))
                .into());
            }
            Some(f) => f,
        };

        let mount_point = match handler(&logger, &storage, sandbox.clone()) {
            // Todo need to rollback the mounted storage if err met.
            Err(e) => return Err(e),
            Ok(m) => m,
        };

        if mount_point.len() > 0 {
            mount_list.push(mount_point);
        }
    }

    Ok(mount_list)
}

fn mount_to_rootfs(logger: &Logger, m: &INIT_MOUNT) -> Result<()> {
    let options_vec: Vec<&str> = m.options.clone();

    let (flags, options) = parse_mount_flags_and_options(options_vec);

    let bare_mount = BareMount::new(m.src, m.dest, m.fstype, flags, options.as_str(), logger);

    fs::create_dir_all(Path::new(m.dest)).chain_err(|| "could not create directory")?;

    if let Err(err) = bare_mount.mount() {
        if m.src != "dev" {
            return Err(err.into());
        }
        error!(
            logger,
            "Could not mount filesystem from {} to {}", m.src, m.dest
        );
    }

    Ok(())
}

pub fn general_mount(logger: &Logger) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "mount"));

    for m in INIT_ROOTFS_MOUNTS.iter() {
        mount_to_rootfs(&logger, m)?;
    }

    Ok(())
}

#[inline]
pub fn get_mount_fs_type(mount_point: &str) -> Result<String> {
    get_mount_fs_type_from_file(PROCMOUNTSTATS, mount_point)
}

// get_mount_fs_type returns the FS type corresponding to the passed mount point and
// any error ecountered.
pub fn get_mount_fs_type_from_file(mount_file: &str, mount_point: &str) -> Result<String> {
    if mount_point == "" {
        return Err(ErrorKind::ErrorCode(format!("Invalid mount point {}", mount_point)).into());
    }

    let file = File::open(mount_file)?;
    let reader = BufReader::new(file);

    let re = Regex::new(format!("device .+ mounted on {} with fstype (.+)", mount_point).as_str())
        .unwrap();

    // Read the file line by line using the lines() iterator from std::io::BufRead.
    for (_index, line) in reader.lines().enumerate() {
        let line = line?;
        let capes = match re.captures(line.as_str()) {
            Some(c) => c,
            None => continue,
        };

        if capes.len() > 1 {
            return Ok(capes[1].to_string());
        }
    }

    Err(ErrorKind::ErrorCode(format!(
        "failed to find FS type for mount point {}",
        mount_point
    ))
    .into())
}

pub fn get_cgroup_mounts(logger: &Logger, cg_path: &str) -> Result<Vec<INIT_MOUNT>> {
    let file = File::open(&cg_path)?;
    let reader = BufReader::new(file);

    let mut has_device_cgroup = false;
    let mut cg_mounts: Vec<INIT_MOUNT> = vec![INIT_MOUNT {
        fstype: "tmpfs",
        src: "tmpfs",
        dest: CGROUPPATH,
        options: vec!["nosuid", "nodev", "noexec", "mode=755"],
    }];

    // #subsys_name    hierarchy       num_cgroups     enabled
    // fields[0]       fields[1]       fields[2]       fields[3]
    'outer: for (_, line) in reader.lines().enumerate() {
        let line = line?;

        let fields: Vec<&str> = line.split("\t").collect();

        // Ignore comment header
        if fields[0].starts_with("#") {
            continue;
        }

        // Ignore truncated lines
        if fields.len() < 4 {
            continue;
        }

        // Ignore disabled cgroups
        if fields[3] == "0" {
            continue;
        }

        // Ignore fields containing invalid numerics
        for f in [fields[1], fields[2], fields[3]].iter() {
            if f.parse::<u64>().is_err() {
                continue 'outer;
            }
        }

        if fields[0] == "" {
            continue;
        }

        if fields[0] == "devices" {
            has_device_cgroup = true;
        }

        if let Some(value) = CGROUPS.get(&fields[0]) {
            let key = CGROUPS.keys().find(|&&f| f == fields[0]).unwrap();
            cg_mounts.push(INIT_MOUNT {
                fstype: "cgroup",
                src: "cgroup",
                dest: *value,
                options: vec!["nosuid", "nodev", "noexec", "relatime", key]
            });
        }
    }

    if !has_device_cgroup {
        warn!(logger, "The system didn't support device cgroup, which is dangerous, thus agent initialized without cgroup support!\n");
        return Ok(Vec::new());
    }

    cg_mounts.push(INIT_MOUNT {
        fstype: "tmpfs",
        src: "tmpfs",
        dest: CGROUPPATH,
        options: vec!["remount", "ro", "nosuid", "nodev", "noexec", "mode=755"],
    });

    Ok(cg_mounts)
}

pub fn cgroups_mount(logger: &Logger) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "mount"));

    let cgroups = get_cgroup_mounts(&logger, PROCCGROUPS)?;

    for cg in cgroups.iter() {
        mount_to_rootfs(&logger, cg)?;
    }

    // Enable memory hierarchical account.
    // For more information see https://www.kernel.org/doc/Documentation/cgroup-v1/memory.txt
    online_device("/sys/fs/cgroup/memory//memory.use_hierarchy")?;
    Ok(())
}

pub fn remove_mounts(mounts: &Vec<String>) -> Result<()> {
    for m in mounts.iter() {
        mount::umount(m.as_str())?;
    }
    Ok(())
}

// ensureDestinationExists will recursively create a given mountpoint. If directories
// are created, their permissions are initialized to mountPerm
fn ensure_destination_exists(destination: &str, fs_type: &str) -> Result<()> {
    let d = Path::new(destination);
    if !d.exists() {
        let dir = match d.parent() {
            Some(d) => d,
            None => {
                return Err(ErrorKind::ErrorCode(format!(
                    "mount destination {} doesn't exist",
                    destination
                ))
                .into())
            }
        };
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
    }

    if fs_type != "bind" || d.is_dir() {
        fs::create_dir_all(d)?;
    } else {
        fs::OpenOptions::new().create(true).open(d)?;
    }

    Ok(())
}

fn parse_options(option_list: Vec<String>) -> HashMap<String, String> {
    let mut options = HashMap::new();
    for opt in option_list.iter() {
        let fields: Vec<&str> = opt.split("=").collect();
        if fields.len() != 2 {
            continue;
        }

        options.insert(fields[0].to_string(), fields[1].to_string());
    }

    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use libc::umount;
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use crate::{
        skip_if_not_root,
        skip_loop_if_not_root,
        skip_loop_if_root,
    };

    #[derive(Debug, PartialEq)]
    enum TestUserType {
        RootOnly,
        NonRootOnly,
        Any,
    }

    #[test]
    fn test_mount() {
        #[derive(Debug)]
        struct TestData<'a> {
            // User(s) who can run this test
            test_user: TestUserType,

            src: &'a str,
            dest: &'a str,
            fs_type: &'a str,
            flags: MsFlags,
            options: &'a str,

            // If set, assume an error will be generated,
            // else assume no error.
            //
            // If not set, assume root required to perform a
            // successful mount.
            error_contains: &'a str,
        }

        let dir = tempdir().expect("failed to create tmpdir");
        let drain = slog::Discard;
        let logger = slog::Logger::root(drain, o!());

        let tests = &[
            TestData {
                test_user: TestUserType::Any,
                src: "",
                dest: "",
                fs_type: "",
                flags: MsFlags::empty(),
                options: "",
                error_contains: "need mount source",
            },
            TestData {
                test_user: TestUserType::Any,
                src: "from",
                dest: "",
                fs_type: "",
                flags: MsFlags::empty(),
                options: "",
                error_contains: "need mount destination",
            },
            TestData {
                test_user: TestUserType::Any,
                src: "from",
                dest: "to",
                fs_type: "",
                flags: MsFlags::empty(),
                options: "",
                error_contains: "need mount FS type",
            },
            TestData {
                test_user: TestUserType::NonRootOnly,
                src: "from",
                dest: "to",
                fs_type: "bind",
                flags: MsFlags::empty(),
                options: "bind",
                error_contains: "Operation not permitted",
            },
            TestData {
                test_user: TestUserType::NonRootOnly,
                src: "from",
                dest: "to",
                fs_type: "bind",
                flags: MsFlags::MS_BIND,
                options: "",
                error_contains: "Operation not permitted",
            },
            TestData {
                test_user: TestUserType::RootOnly,
                src: "from",
                dest: "to",
                fs_type: "bind",
                flags: MsFlags::MS_BIND,
                options: "",
                error_contains: "",
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            if d.test_user == TestUserType::RootOnly {
                skip_loop_if_not_root!(msg);
            } else if d.test_user == TestUserType::NonRootOnly {
                skip_loop_if_root!(msg);
            }

            let src: PathBuf;
            let dest: PathBuf;

            let src_filename: String;
            let dest_filename: String;

            if d.src != "" {
                src = dir.path().join(d.src.to_string());
                src_filename = src
                    .to_str()
                    .expect("failed to convert src to filename")
                    .to_string();
            } else {
                src_filename = "".to_owned();
            }

            if d.dest != "" {
                dest = dir.path().join(d.dest.to_string());
                dest_filename = dest
                    .to_str()
                    .expect("failed to convert dest to filename")
                    .to_string();
            } else {
                dest_filename = "".to_owned();
            }

            // Create the mount directories
            for d in [src_filename.clone(), dest_filename.clone()].iter() {
                if d == "" {
                    continue;
                }

                std::fs::create_dir_all(d).expect("failed to created directory");
            }

            let bare_mount = BareMount::new(
                &src_filename,
                &dest_filename,
                d.fs_type,
                d.flags,
                d.options,
                &logger,
            );

            let result = bare_mount.mount();

            let msg = format!("{}: result: {:?}", msg, result);

            if d.error_contains == "" {
                assert!(result.is_ok(), msg);

                // Cleanup
                unsafe {
                    let cstr_dest =
                        CString::new(dest_filename).expect("failed to convert dest to cstring");
                    let umount_dest = cstr_dest.as_ptr();

                    let ret = umount(umount_dest);

                    let msg = format!("{}: umount result: {:?}", msg, result);

                    assert!(ret == 0, format!("{}", msg));
                };

                continue;
            }

            let err = result.unwrap_err();
            let error_msg = format!("{}", err);
            assert!(error_msg.contains(d.error_contains), msg);
        }
    }

    #[test]
    fn test_remove_mounts() {
        skip_if_not_root!();

        #[derive(Debug)]
        struct TestData<'a> {
            mounts: Vec<String>,

            // If set, assume an error will be generated,
            // else assume no error.
            error_contains: &'a str,
        }

        let dir = tempdir().expect("failed to create tmpdir");
        let drain = slog::Discard;
        let logger = slog::Logger::root(drain, o!());

        let test_dir_path = dir.path().join("dir");
        let test_dir_filename = test_dir_path
            .to_str()
            .expect("failed to create mount dir filename");

        let test_file_path = dir.path().join("file");
        let test_file_filename = test_file_path
            .to_str()
            .expect("failed to create mount file filename");

        OpenOptions::new()
            .create(true)
            .write(true)
            .open(test_file_filename)
            .expect("failed to create test file");

        std::fs::create_dir_all(test_dir_filename).expect("failed to create dir");

        let mnt_src = dir.path().join("mnt-src");
        let mnt_src_filename = mnt_src
            .to_str()
            .expect("failed to create mount source filename");

        let mnt_dest = dir.path().join("mnt-dest");
        let mnt_dest_filename = mnt_dest
            .to_str()
            .expect("failed to create mount destination filename");

        for d in [test_dir_filename, mnt_src_filename, mnt_dest_filename].iter() {
            std::fs::create_dir_all(d).expect(&format!("failed to create directory {}", d));
        }

        // Create an actual mount
        let bare_mount = BareMount::new(
            &mnt_src_filename,
            &mnt_dest_filename,
            "bind",
            MsFlags::MS_BIND,
            "",
            &logger,
        );

        let result = bare_mount.mount();
        assert!(result.is_ok(), "mount for test setup failed");

        let tests = &[
            TestData {
                mounts: vec![],
                error_contains: "",
            },
            TestData {
                mounts: vec!["".to_string()],
                error_contains: "ENOENT: No such file or directory",
            },
            TestData {
                mounts: vec![test_file_filename.to_string()],
                error_contains: "EINVAL: Invalid argument",
            },
            TestData {
                mounts: vec![test_dir_filename.to_string()],
                error_contains: "EINVAL: Invalid argument",
            },
            TestData {
                mounts: vec![mnt_dest_filename.to_string()],
                error_contains: "",
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = remove_mounts(&d.mounts);

            let msg = format!("{}: result: {:?}", msg, result);

            if d.error_contains == "" {
                assert!(result.is_ok(), msg);
                continue;
            }

            let error_msg = format!("{}", result.unwrap_err());

            assert!(error_msg.contains(d.error_contains), msg);
        }
    }

    #[test]
    fn test_get_mount_fs_type_from_file() {
        #[derive(Debug)]
        struct TestData<'a> {
            // Create file with the specified contents
            // (even if a nul string is specified).
            contents: &'a str,
            mount_point: &'a str,

            // If set, assume an error will be generated,
            // else assume no error.
            error_contains: &'a str,

            // successful return value
            fs_type: &'a str,
        }

        let dir = tempdir().expect("failed to create tmpdir");

        let tests = &[
            TestData {
                contents: "",
                mount_point: "",
                error_contains: "Invalid mount point",
                fs_type: "",
            },
            TestData {
                contents: "foo",
                mount_point: "",
                error_contains: "Invalid mount point",
                fs_type: "",
            },
            TestData {
                contents: "foo",
                mount_point: "/",
                error_contains: "failed to find FS type for mount point /",
                fs_type: "",
            },
            TestData {
                // contents missing fields
                contents: "device /dev/mapper/root mounted on /",
                mount_point: "/",
                error_contains: "failed to find FS type for mount point /",
                fs_type: "",
            },
            TestData {
                contents: "device /dev/mapper/root mounted on / with fstype ext4",
                mount_point: "/",
                error_contains: "",
                fs_type: "ext4",
            },
        ];

        let enoent_file_path = dir.path().join("enoent");
        let enoent_filename = enoent_file_path
            .to_str()
            .expect("failed to create enoent filename");

        // First, test that an empty mount file is handled
        for (i, mp) in ["/", "/somewhere", "/tmp", enoent_filename]
            .iter()
            .enumerate()
        {
            let msg = format!("missing mount file test[{}] with mountpoint: {}", i, mp);

            let result = get_mount_fs_type_from_file("", mp);
            let err = result.unwrap_err();

            let msg = format!("{}: error: {}", msg, err);

            assert!(
                format!("{}", err).contains("No such file or directory"),
                msg
            );
        }

        // Now, test various combinations of file contents
        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let file_path = dir.path().join("mount_stats");

            let filename = file_path
                .to_str()
                .expect(&format!("{}: failed to create filename", msg));

            let mut file =
                File::create(filename).expect(&format!("{}: failed to create file", msg));

            file.write_all(d.contents.as_bytes())
                .expect(&format!("{}: failed to write file contents", msg));

            let result = get_mount_fs_type_from_file(filename, d.mount_point);

            // add more details if an assertion fails
            let msg = format!("{}: result: {:?}", msg, result);

            if d.error_contains == "" {
                let fs_type = result.unwrap();

                assert!(d.fs_type == fs_type, msg);

                continue;
            }

            let error_msg = format!("{}", result.unwrap_err());
            assert!(error_msg.contains(d.error_contains), msg);
        }
    }

    #[test]
    fn test_get_cgroup_mounts() {
        #[derive(Debug)]
        struct TestData<'a> {
            // Create file with the specified contents
            // (even if a nul string is specified).
            contents: &'a str,

            // If set, assume an error will be generated,
            // else assume no error.
            error_contains: &'a str,

            // Set if the devices cgroup is expected to be found
            devices_cgroup: bool,
        }

        let dir = tempdir().expect("failed to create tmpdir");
        let drain = slog::Discard;
        let logger = slog::Logger::root(drain, o!());

        let first_mount = INIT_MOUNT {
            fstype: "tmpfs",
            src: "tmpfs",
            dest: CGROUPPATH,
            options: vec!["nosuid", "nodev", "noexec", "mode=755"],
        };

        let last_mount = INIT_MOUNT {
            fstype: "tmpfs",
            src: "tmpfs",
            dest: CGROUPPATH,
            options: vec!["remount", "ro", "nosuid", "nodev", "noexec", "mode=755"],
        };

        let cg_devices_mount = INIT_MOUNT {
            fstype: "cgroup",
            src: "cgroup",
            dest: "/sys/fs/cgroup/devices",
            options: vec!["nosuid", "nodev", "noexec", "relatime", "devices"],
        };

        let enoent_file_path = dir.path().join("enoent");
        let enoent_filename = enoent_file_path
            .to_str()
            .expect("failed to create enoent filename");

        let tests = &[
            TestData {
                // Empty file
                contents: "",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Only a comment line
                contents: "#subsys_name	hierarchy	num_cgroups	enabled",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Single (invalid) field
                contents: "foo",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Multiple (invalid) fields
                contents: "this\tis\tinvalid\tdata\n",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Valid first field, but other fields missing
                contents: "devices\n",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Valid first field, but invalid others fields
                contents: "devices\tinvalid\tinvalid\tinvalid\n",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Valid first field, but lots of invalid others fields
                contents: "devices\tinvalid\tinvalid\tinvalid\tinvalid\tinvalid\n",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Valid, but disabled
                contents: "devices\t1\t1\t0\n",
                error_contains: "",
                devices_cgroup: false,
            },
            TestData {
                // Valid
                contents: "devices\t1\t1\t1\n",
                error_contains: "",
                devices_cgroup: true,
            },
        ];

        // First, test a missing file
        let result = get_cgroup_mounts(&logger, enoent_filename);

        assert!(result.is_err());
        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("No such file or directory"),
            "enoent test"
        );

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let file_path = dir.path().join("cgroups");
            let filename = file_path
                .to_str()
                .expect("failed to create cgroup file filename");

            let mut file =
                File::create(filename).expect(&format!("{}: failed to create file", msg));

            file.write_all(d.contents.as_bytes())
                .expect(&format!("{}: failed to write file contents", msg));

            let result = get_cgroup_mounts(&logger, filename);
            let msg = format!("{}: result: {:?}", msg, result);

            if d.error_contains != "" {
                assert!(result.is_err(), msg);

                let error_msg = format!("{}", result.unwrap_err());
                assert!(error_msg.contains(d.error_contains), msg);
                continue;
            }

            assert!(result.is_ok(), msg);

            let mounts = result.unwrap();
            let count = mounts.len();

            if !d.devices_cgroup {
                assert!(count == 0, msg);
                continue;
            }

            // get_cgroup_mounts() adds the device cgroup plus two other mounts.
            assert!(count == (1 + 2), msg);

            // First mount
            assert!(mounts[0].eq(&first_mount), msg);

            // Last mount
            assert!(mounts[2].eq(&last_mount), msg);

            // Devices cgroup
            assert!(mounts[1].eq(&cg_devices_mount), msg);
        }
    }
}
