// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::{get_linux_mount_info, parse_mount_options};
use nix::mount::MsFlags;
use regex::Regex;
use slog::Logger;
use tracing::instrument;

use crate::device::online_device;
use crate::linux_abi::*;

pub const TYPE_ROOTFS: &str = "rootfs";

#[derive(Debug, PartialEq)]
pub struct InitMount<'a> {
    fstype: &'a str,
    src: &'a str,
    dest: &'a str,
    options: Vec<&'a str>,
}

#[rustfmt::skip]
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

#[rustfmt::skip]
lazy_static! {
    pub static ref INIT_ROOTFS_MOUNTS: Vec<InitMount<'static>> = vec![
        InitMount{fstype: "proc", src: "proc", dest: "/proc", options: vec!["nosuid", "nodev", "noexec"]},
        InitMount{fstype: "sysfs", src: "sysfs", dest: "/sys", options: vec!["nosuid", "nodev", "noexec"]},
        InitMount{fstype: "devtmpfs", src: "dev", dest: "/dev", options: vec!["nosuid"]},
        InitMount{fstype: "tmpfs", src: "tmpfs", dest: "/dev/shm", options: vec!["nosuid", "nodev"]},
        InitMount{fstype: "devpts", src: "devpts", dest: "/dev/pts", options: vec!["nosuid", "noexec"]},
        InitMount{fstype: "tmpfs", src: "tmpfs", dest: "/run", options: vec!["nosuid", "nodev"]},
    ];
}

#[instrument]
pub fn baremount(
    source: &Path,
    destination: &Path,
    fs_type: &str,
    flags: MsFlags,
    options: &str,
    logger: &Logger,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "baremount"));

    if source.as_os_str().is_empty() {
        return Err(anyhow!("need mount source"));
    }

    if destination.as_os_str().is_empty() {
        return Err(anyhow!("need mount destination"));
    }

    if fs_type.is_empty() {
        return Err(anyhow!("need mount FS type"));
    }

    let destination_str = destination.to_string_lossy();
    if let Ok(m) = get_linux_mount_info(destination_str.deref()) {
        if m.fs_type == fs_type {
            slog_info!(logger, "{source:?} is already mounted at {destination:?}");
            return Ok(());
        }
    }

    info!(
        logger,
        "baremount source={:?}, dest={:?}, fs_type={:?}, options={:?}, flags={:?}",
        source,
        destination,
        fs_type,
        options,
        flags
    );

    nix::mount::mount(
        Some(source),
        destination,
        Some(fs_type),
        flags,
        Some(options),
    )
    .map_err(|e| {
        anyhow!(
            "failed to mount {} to {}, with error: {}",
            source.display(),
            destination.display(),
            e
        )
    })
}

/// Looks for `mount_point` entry in the /proc/mounts.
#[instrument]
pub fn is_mounted(mount_point: &str) -> Result<bool> {
    let mount_point = mount_point.trim_end_matches('/');
    let found = fs::metadata(mount_point).is_ok() && get_linux_mount_info(mount_point).is_ok();
    Ok(found)
}

#[instrument]
fn mount_to_rootfs(logger: &Logger, m: &InitMount) -> Result<()> {
    fs::create_dir_all(m.dest).context("could not create directory")?;

    let (flags, options) = parse_mount_options(&m.options)?;
    let source = Path::new(m.src);
    let dest = Path::new(m.dest);

    baremount(source, dest, m.fstype, flags, &options, logger).or_else(|e| {
        if m.src == "dev" {
            error!(
                logger,
                "Could not mount filesystem from {} to {}", m.src, m.dest
            );
            Ok(())
        } else {
            Err(e)
        }
    })
}

#[instrument]
pub fn general_mount(logger: &Logger) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "mount"));

    for m in INIT_ROOTFS_MOUNTS.iter() {
        mount_to_rootfs(&logger, m)?;
    }

    Ok(())
}

#[inline]
pub fn get_mount_fs_type(mount_point: &str) -> Result<String> {
    get_mount_fs_type_from_file(PROC_MOUNTSTATS, mount_point)
}

// get_mount_fs_type_from_file returns the FS type corresponding to the passed mount point and
// any error encountered.
#[instrument]
pub fn get_mount_fs_type_from_file(mount_file: &str, mount_point: &str) -> Result<String> {
    if mount_point.is_empty() {
        return Err(anyhow!("Invalid mount point {}", mount_point));
    }

    let content = fs::read_to_string(mount_file)
        .map_err(|e| anyhow!("read mount file {}: {}", mount_file, e))?;

    let re = Regex::new(format!("device .+ mounted on {} with fstype (.+)", mount_point).as_str())?;

    // Read the file line by line using the lines() iterator from std::io::BufRead.
    for line in content.lines() {
        if let Some(capes) = re.captures(line) {
            if capes.len() > 1 {
                return Ok(capes[1].to_string());
            }
        }
    }

    Err(anyhow!(
        "failed to find FS type for mount point {}, mount file content: {:?}",
        mount_point,
        content
    ))
}

#[instrument]
pub fn get_cgroup_mounts(
    logger: &Logger,
    cg_path: &str,
    unified_cgroup_hierarchy: bool,
) -> Result<Vec<InitMount<'static>>> {
    // cgroup v2
    // https://github.com/kata-containers/agent/blob/8c9bbadcd448c9a67690fbe11a860aaacc69813c/agent.go#L1249
    if unified_cgroup_hierarchy {
        return Ok(vec![InitMount {
            fstype: "cgroup2",
            src: "cgroup2",
            dest: "/sys/fs/cgroup",
            options: vec!["nosuid", "nodev", "noexec", "relatime", "nsdelegate"],
        }]);
    }

    let file = File::open(cg_path)?;
    let reader = BufReader::new(file);

    let mut has_device_cgroup = false;
    let mut cg_mounts: Vec<InitMount> = vec![InitMount {
        fstype: "tmpfs",
        src: "tmpfs",
        dest: SYSFS_CGROUPPATH,
        options: vec!["nosuid", "nodev", "noexec", "mode=755"],
    }];

    // #subsys_name    hierarchy       num_cgroups     enabled
    // fields[0]       fields[1]       fields[2]       fields[3]
    'outer: for line in reader.lines() {
        let line = line?;

        let fields: Vec<&str> = line.split('\t').collect();

        // Ignore comment header
        if fields[0].starts_with('#') {
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

        let subsystem_name = fields[0];

        if subsystem_name.is_empty() {
            continue;
        }

        if subsystem_name == "devices" {
            has_device_cgroup = true;
        }

        if let Some((key, value)) = CGROUPS.get_key_value(subsystem_name) {
            cg_mounts.push(InitMount {
                fstype: "cgroup",
                src: "cgroup",
                dest: value,
                options: vec!["nosuid", "nodev", "noexec", "relatime", key],
            });
        }
    }

    if !has_device_cgroup {
        warn!(logger, "The system didn't support device cgroup, which is dangerous, thus agent initialized without cgroup support!\n");
        return Ok(Vec::new());
    }

    cg_mounts.push(InitMount {
        fstype: "tmpfs",
        src: "tmpfs",
        dest: SYSFS_CGROUPPATH,
        options: vec!["remount", "ro", "nosuid", "nodev", "noexec", "mode=755"],
    });

    Ok(cg_mounts)
}

#[instrument]
pub fn cgroups_mount(logger: &Logger, unified_cgroup_hierarchy: bool) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "mount"));

    let cgroups = get_cgroup_mounts(&logger, PROC_CGROUPS, unified_cgroup_hierarchy)?;

    for cg in cgroups.iter() {
        mount_to_rootfs(&logger, cg)?;
    }

    // Enable memory hierarchical account.
    // For more information see https://www.kernel.org/doc/Documentation/cgroup-v1/memory.txt
    // cgroupsV2 will automatically enable memory.use_hierarchy.
    // additinoally this directory layout is not present in cgroupsV2.
    if !unified_cgroup_hierarchy {
        return online_device("/sys/fs/cgroup/memory/memory.use_hierarchy");
    }

    Ok(())
}

#[instrument]
pub fn remove_mounts<P: AsRef<str> + std::fmt::Debug>(mounts: &[P]) -> Result<()> {
    for m in mounts.iter() {
        nix::mount::umount(m.as_ref()).context(format!("failed to umount {:?}", m.as_ref()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slog::Drain;
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use test_utils::TestUserType;
    use test_utils::{
        skip_if_not_root, skip_loop_by_user, skip_loop_if_not_root, skip_loop_if_root,
    };

    #[test]
    fn test_already_baremounted() {
        let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
        let logger = Logger::root(slog_term::FullFormat::new(plain).build().fuse(), o!());

        let test_cases = [
            ("dev", "/dev", "devtmpfs"),
            ("udev", "/dev", "devtmpfs"),
            ("proc", "/proc", "proc"),
            ("sysfs", "/sys", "sysfs"),
        ];

        for &(source, destination, fs_type) in &test_cases {
            let source = Path::new(source);
            let destination = Path::new(destination);
            let flags = MsFlags::MS_RDONLY;
            let options = "mode=755";
            println!(
                "testing if already mounted baremount({:?} {:?} {:?})",
                source, destination, fs_type
            );
            assert!(baremount(source, destination, fs_type, flags, options, &logger).is_ok());
        }
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

            skip_loop_by_user!(msg, d.test_user);

            let src: PathBuf;
            let dest: PathBuf;

            let src_filename: String;
            let dest_filename: String;

            if !d.src.is_empty() {
                src = dir.path().join(d.src);
                src_filename = src
                    .to_str()
                    .expect("failed to convert src to filename")
                    .to_string();
            } else {
                src_filename = "".to_owned();
            }

            if !d.dest.is_empty() {
                dest = dir.path().join(d.dest);
                dest_filename = dest
                    .to_str()
                    .expect("failed to convert dest to filename")
                    .to_string();
            } else {
                dest_filename = "".to_owned();
            }

            // Create the mount directories
            for d in [src_filename.clone(), dest_filename.clone()].iter() {
                if d.is_empty() {
                    continue;
                }

                std::fs::create_dir_all(d).expect("failed to created directory");
            }

            let src = Path::new(&src_filename);
            let dest = Path::new(&dest_filename);

            let result = baremount(src, dest, d.fs_type, d.flags, d.options, &logger);

            let msg = format!("{}: result: {:?}", msg, result);

            if d.error_contains.is_empty() {
                assert!(result.is_ok(), "{}", msg);

                // Cleanup
                nix::mount::umount(dest_filename.as_str()).unwrap();

                continue;
            }

            let err = result.unwrap_err();
            let error_msg = format!("{}", err);
            assert!(error_msg.contains(d.error_contains), "{}", msg);
        }
    }

    #[test]
    fn test_is_mounted() {
        assert!(is_mounted("/proc").unwrap());
        assert!(!is_mounted("").unwrap());
        assert!(!is_mounted("!").unwrap());
        assert!(!is_mounted("/not_existing_path").unwrap());
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
            .truncate(true)
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
            std::fs::create_dir_all(d)
                .unwrap_or_else(|_| panic!("failed to create directory {}", d));
        }

        let src = Path::new(mnt_src_filename);
        let dest = Path::new(mnt_dest_filename);

        // Create an actual mount
        let result = baremount(src, dest, "bind", MsFlags::MS_BIND, "", &logger);
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

            if d.error_contains.is_empty() {
                assert!(result.is_ok(), "{}", msg);
                continue;
            }

            let error_msg = format!("{:#}", result.unwrap_err());

            assert!(error_msg.contains(d.error_contains), "{}", msg);
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
                "{}",
                msg
            );
        }

        // Now, test various combinations of file contents
        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let file_path = dir.path().join("mount_stats");

            let filename = file_path
                .to_str()
                .unwrap_or_else(|| panic!("{}: failed to create filename", msg));

            let mut file =
                File::create(filename).unwrap_or_else(|_| panic!("{}: failed to create file", msg));

            file.write_all(d.contents.as_bytes())
                .unwrap_or_else(|_| panic!("{}: failed to write file contents", msg));

            let result = get_mount_fs_type_from_file(filename, d.mount_point);

            // add more details if an assertion fails
            let msg = format!("{}: result: {:?}", msg, result);

            if d.error_contains.is_empty() {
                let fs_type = result.unwrap();

                assert!(d.fs_type == fs_type, "{}", msg);

                continue;
            }

            let error_msg = format!("{}", result.unwrap_err());
            assert!(error_msg.contains(d.error_contains), "{}", msg);
        }
    }

    #[test]
    fn test_get_cgroup_v2_mounts() {
        let _ = tempdir().expect("failed to create tmpdir");
        let drain = slog::Discard;
        let logger = slog::Logger::root(drain, o!());
        let result = get_cgroup_mounts(&logger, "", true);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(1, result.len());
        assert_eq!(result[0].fstype, "cgroup2");
        assert_eq!(result[0].src, "cgroup2");
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

        let first_mount = InitMount {
            fstype: "tmpfs",
            src: "tmpfs",
            dest: SYSFS_CGROUPPATH,
            options: vec!["nosuid", "nodev", "noexec", "mode=755"],
        };

        let last_mount = InitMount {
            fstype: "tmpfs",
            src: "tmpfs",
            dest: SYSFS_CGROUPPATH,
            options: vec!["remount", "ro", "nosuid", "nodev", "noexec", "mode=755"],
        };

        let cg_devices_mount = InitMount {
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
        let result = get_cgroup_mounts(&logger, enoent_filename, false);

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
                File::create(filename).unwrap_or_else(|_| panic!("{}: failed to create file", msg));

            file.write_all(d.contents.as_bytes())
                .unwrap_or_else(|_| panic!("{}: failed to write file contents", msg));

            let result = get_cgroup_mounts(&logger, filename, false);
            let msg = format!("{}: result: {:?}", msg, result);

            if !d.error_contains.is_empty() {
                assert!(result.is_err(), "{}", msg);

                let error_msg = format!("{}", result.unwrap_err());
                assert!(error_msg.contains(d.error_contains), "{}", msg);
                continue;
            }

            assert!(result.is_ok(), "{}", msg);

            let mounts = result.unwrap();
            let count = mounts.len();

            if !d.devices_cgroup {
                assert!(count == 0, "{}", msg);
                continue;
            }

            // get_cgroup_mounts() adds the device cgroup plus two other mounts.
            assert!(count == (1 + 2), "{}", msg);

            // First mount
            assert!(mounts[0].eq(&first_mount), "{}", msg);

            // Last mount
            assert!(mounts[2].eq(&last_mount), "{}", msg);

            // Devices cgroup
            assert!(mounts[1].eq(&cg_devices_mount), "{}", msg);
        }
    }

    #[test]
    fn test_mount_to_rootfs() {
        #[derive(Debug)]
        struct TestData<'a> {
            test_user: TestUserType,
            src: &'a str,
            options: Vec<&'a str>,
            error_contains: &'a str,
            deny_mount_dir_permission: bool,
            // if true src will be prepended with a temporary directory
            mask_src: bool,
        }

        impl Default for TestData<'_> {
            fn default() -> Self {
                TestData {
                    test_user: TestUserType::Any,
                    src: "src",
                    options: vec![],
                    error_contains: "",
                    deny_mount_dir_permission: false,
                    mask_src: true,
                }
            }
        }

        let tests = &[
            TestData {
                test_user: TestUserType::NonRootOnly,
                error_contains: "EPERM: Operation not permitted",
                ..Default::default()
            },
            TestData {
                test_user: TestUserType::NonRootOnly,
                src: "dev",
                mask_src: false,
                ..Default::default()
            },
            TestData {
                test_user: TestUserType::RootOnly,
                ..Default::default()
            },
            TestData {
                test_user: TestUserType::NonRootOnly,
                deny_mount_dir_permission: true,
                error_contains: "could not create directory",
                ..Default::default()
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            skip_loop_by_user!(msg, d.test_user);

            let drain = slog::Discard;
            let logger = slog::Logger::root(drain, o!());
            let tempdir = tempdir().unwrap();

            let src = if d.mask_src {
                tempdir.path().join(d.src)
            } else {
                Path::new(d.src).to_path_buf()
            };
            let dest = tempdir.path().join("mnt");
            let init_mount = InitMount {
                fstype: "tmpfs",
                src: src.to_str().unwrap(),
                dest: dest.to_str().unwrap(),
                options: d.options.clone(),
            };

            if d.deny_mount_dir_permission {
                fs::set_permissions(dest.parent().unwrap(), fs::Permissions::from_mode(0o000))
                    .unwrap();
            }

            let result = mount_to_rootfs(&logger, &init_mount);

            // restore permissions so tempdir can be cleaned up
            if d.deny_mount_dir_permission {
                fs::set_permissions(dest.parent().unwrap(), fs::Permissions::from_mode(0o755))
                    .unwrap();
            }

            if result.is_ok() && d.mask_src {
                nix::mount::umount(&dest).unwrap();
            }

            let msg = format!("{}: result: {:?}", msg, result);
            if d.error_contains.is_empty() {
                assert!(result.is_ok(), "{}", msg);
            } else {
                assert!(result.is_err(), "{}", msg);
                let error_msg = format!("{}", result.unwrap_err());
                assert!(error_msg.contains(d.error_contains), "{}", msg);
            }
        }
    }

    #[test]
    fn test_parse_mount_flags_and_options() {
        #[derive(Debug)]
        struct TestData<'a> {
            options_vec: Vec<&'a str>,
            result: (MsFlags, &'a str),
        }

        let tests = &[
            TestData {
                options_vec: vec![],
                result: (MsFlags::empty(), ""),
            },
            TestData {
                options_vec: vec!["ro"],
                result: (MsFlags::MS_RDONLY, ""),
            },
            TestData {
                options_vec: vec!["rw"],
                result: (MsFlags::empty(), ""),
            },
            TestData {
                options_vec: vec!["ro", "rw"],
                result: (MsFlags::empty(), ""),
            },
            TestData {
                options_vec: vec!["ro", "nodev"],
                result: (MsFlags::MS_RDONLY | MsFlags::MS_NODEV, ""),
            },
            TestData {
                options_vec: vec!["option1", "nodev", "option2"],
                result: (MsFlags::MS_NODEV, "option1,option2"),
            },
            TestData {
                options_vec: vec!["rbind", "", "ro"],
                result: (MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_RDONLY, ""),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = parse_mount_options(&d.options_vec).unwrap();

            let msg = format!("{}: result: {:?}", msg, result);

            let expected_result = (d.result.0, d.result.1.to_owned());
            assert_eq!(expected_result, result, "{}", msg);
        }
    }
}
