use bitflags::bitflags;

use crate::{from_iter, FileWrapper, ProcResult};

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Lines, Read};
use std::path::PathBuf;
use std::time::Duration;

bitflags! {
    pub struct NFSServerCaps: u32 {

        const NFS_CAP_READDIRPLUS = 1;
        const NFS_CAP_HARDLINKS = (1 << 1);
        const NFS_CAP_SYMLINKS = (1 << 2);
        const NFS_CAP_ACLS = (1 << 3);
        const NFS_CAP_ATOMIC_OPEN = (1 << 4);
        const NFS_CAP_LGOPEN = (1 << 5);
        const NFS_CAP_FILEID = (1 << 6);
        const NFS_CAP_MODE = (1 << 7);
        const NFS_CAP_NLINK = (1 << 8);
        const NFS_CAP_OWNER = (1 << 9);
        const NFS_CAP_OWNER_GROUP = (1 << 10);
        const NFS_CAP_ATIME = (1 << 11);
        const NFS_CAP_CTIME = (1 << 12);
        const NFS_CAP_MTIME = (1 << 13);
        const NFS_CAP_POSIX_LOCK = (1 << 14);
        const NFS_CAP_UIDGID_NOMAP = (1 << 15);
        const NFS_CAP_STATEID_NFSV41 = (1 << 16);
        const NFS_CAP_ATOMIC_OPEN_V1 = (1 << 17);
        const NFS_CAP_SECURITY_LABEL = (1 << 18);
        const NFS_CAP_SEEK = (1 << 19);
        const NFS_CAP_ALLOCATE = (1 << 20);
        const NFS_CAP_DEALLOCATE = (1 << 21);
        const NFS_CAP_LAYOUTSTATS = (1 << 22);
        const NFS_CAP_CLONE = (1 << 23);
        const NFS_CAP_COPY = (1 << 24);
        const NFS_CAP_OFFLOAD_CANCEL = (1 << 25);
    }
}

impl super::Process {
    /// Returns the [MountStat] data for this processes mount namespace.
    pub fn mountstats(&self) -> ProcResult<Vec<MountStat>> {
        let path = self.root.join("mountstats");
        let file = FileWrapper::open(&path)?;
        MountStat::from_reader(file)
    }

    /// Returns info about the mountpoints in this this process's mount namespace
    ///
    /// This data is taken from the `/proc/[pid]/mountinfo` file
    ///
    /// (Since Linux 2.6.26)
    pub fn mountinfo(&self) -> ProcResult<Vec<MountInfo>> {
        let path = self.root.join("mountinfo");
        let file = FileWrapper::open(&path)?;
        let bufread = BufReader::new(file);
        let lines = bufread.lines();
        let mut vec = Vec::new();
        for line in lines {
            vec.push(MountInfo::from_line(&line?)?);
        }

        Ok(vec)
    }
}

/// Information about a specific mount in a process's mount namespace.
///
/// This data is taken from the `/proc/[pid]/mountinfo` file.
///
/// For an example, see the [mountinfo.rs](https://github.com/eminence/procfs/tree/master/examples)
/// example in the source repo.
#[derive(Debug, Clone)]
pub struct MountInfo {
    /// Mount ID.  A unique ID for the mount (but may be reused after `unmount`)
    pub mnt_id: i32,
    /// Parent mount ID.  The ID of the parent mount (or of self for the root of the mount
    /// namespace's mount tree).
    ///
    /// If the parent mount point lies outside the process's root directory, the ID shown here
    /// won't have a corresponding record in mountinfo whose mount ID matches this parent mount
    /// ID (because mount points that lie outside the process's root directory are not shown in
    /// mountinfo).  As a special case of this point, the process's root mount point may have a
    /// parent mount (for the initramfs filesystem) that lies outside the process's root
    /// directory, and an entry for  that mount point will not appear in mountinfo.
    pub pid: i32,
    /// The value of `st_dev` for files on this filesystem
    pub majmin: String,
    /// The pathname of the directory in the filesystem which forms the root of this mount.
    pub root: String,
    /// The pathname of the mount point relative to the process's root directory.
    pub mount_point: PathBuf,
    /// Per-mount options
    pub mount_options: HashMap<String, Option<String>>,
    /// Optional fields
    pub opt_fields: Vec<MountOptFields>,
    /// Filesystem type
    pub fs_type: String,
    /// Mount source
    pub mount_source: Option<String>,
    /// Per-superblock options.
    pub super_options: HashMap<String, Option<String>>,
}

impl MountInfo {
    pub(crate) fn from_line(line: &str) -> ProcResult<MountInfo> {
        let mut split = line.split_whitespace();

        let mnt_id = expect!(from_iter(&mut split));
        let pid = expect!(from_iter(&mut split));
        let majmin: String = expect!(from_iter(&mut split));
        let root = expect!(from_iter(&mut split));
        let mount_point = expect!(from_iter(&mut split));
        let mount_options = {
            let mut map = HashMap::new();
            let all_opts = expect!(split.next());
            for opt in all_opts.split(',') {
                let mut s = opt.splitn(2, '=');
                let opt_name = expect!(s.next());
                map.insert(opt_name.to_owned(), s.next().map(|s| s.to_owned()));
            }
            map
        };

        let mut opt_fields = Vec::new();
        loop {
            let f = expect!(split.next());
            if f == "-" {
                break;
            }
            let mut s = f.split(':');
            let opt = match expect!(s.next()) {
                "shared" => {
                    let val = expect!(from_iter(&mut s));
                    MountOptFields::Shared(val)
                }
                "master" => {
                    let val = expect!(from_iter(&mut s));
                    MountOptFields::Master(val)
                }
                "propagate_from" => {
                    let val = expect!(from_iter(&mut s));
                    MountOptFields::PropagateFrom(val)
                }
                "unbindable" => MountOptFields::Unbindable,
                _ => continue,
            };
            opt_fields.push(opt);
        }
        let fs_type: String = expect!(from_iter(&mut split));
        let mount_source = match expect!(split.next()) {
            "none" => None,
            x => Some(x.to_owned()),
        };
        let super_options = {
            let mut map = HashMap::new();
            let all_opts = expect!(split.next());
            for opt in all_opts.split(',') {
                let mut s = opt.splitn(2, '=');
                let opt_name = expect!(s.next());
                map.insert(opt_name.to_owned(), s.next().map(|s| s.to_owned()));
            }
            map
        };

        Ok(MountInfo {
            mnt_id,
            pid,
            majmin,
            root,
            mount_point,
            mount_options,
            opt_fields,
            fs_type,
            mount_source,
            super_options,
        })
    }
}

/// Optional fields used in [MountInfo]
#[derive(Debug, Clone)]
pub enum MountOptFields {
    /// This mount point is shared in peer group.  Each peer group has a unique ID that is
    /// automatically generated by the kernel, and all mount points in the same peer group will
    /// show the same ID
    Shared(u32),
    /// THis mount is a slave to the specified shared peer group.
    Master(u32),
    /// This mount is a slave and receives propagation from the shared peer group
    PropagateFrom(u32),
    /// This is an unbindable mount
    Unbindable,
}

/// Mount information from `/proc/<pid>/mountstats`.
///
/// # Example:
///
/// ```
/// # use procfs::process::Process;
/// let stats = Process::myself().unwrap().mountstats().unwrap();
///
/// for mount in stats {
///     println!("{} mounted on {} wth type {}",
///         mount.device.unwrap_or("??".to_owned()),
///         mount.mount_point.display(),
///         mount.fs
///     );
/// }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct MountStat {
    /// The name of the mounted device
    pub device: Option<String>,
    /// The mountpoint within the filesystem tree
    pub mount_point: PathBuf,
    /// The filesystem type
    pub fs: String,
    /// If the mount is NFS, this will contain various NFS statistics
    pub statistics: Option<MountNFSStatistics>,
}

impl MountStat {
    pub fn from_reader<R: Read>(r: R) -> ProcResult<Vec<MountStat>> {
        let mut v = Vec::new();
        let bufread = BufReader::new(r);
        let mut lines = bufread.lines();
        while let Some(Ok(line)) = lines.next() {
            if line.starts_with("device ") {
                // line will be of the format:
                // device proc mounted on /proc with fstype proc
                let mut s = line.split_whitespace();

                let device = Some(expect!(s.nth(1)).to_owned());
                let mount_point = PathBuf::from(expect!(s.nth(2)));
                let fs = expect!(s.nth(2)).to_owned();
                let statistics = match s.next() {
                    Some(stats) if stats.starts_with("statvers=") => {
                        Some(MountNFSStatistics::from_lines(&mut lines, &stats[9..])?)
                    }
                    _ => None,
                };

                v.push(MountStat {
                    device,
                    mount_point,
                    fs,
                    statistics,
                });
            }
        }

        Ok(v)
    }
}

/// Only NFS mounts provide additional statistics in `MountStat` entries.
//
// Thank you to Chris Siebenmann for their helpful work in documenting these structures:
// https://utcc.utoronto.ca/~cks/space/blog/linux/NFSMountstatsIndex
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct MountNFSStatistics {
    /// The version of the NFS statistics block.  Either "1.0" or "1.1".
    pub version: String,
    /// The mount options.
    ///
    /// The meaning of these can be found in the manual pages for mount(5) and nfs(5)
    pub opts: Vec<String>,
    /// Duration the NFS mount has been in existence.
    pub age: Duration,
    // * fsc (?)
    // * impl_id (NFSv4): Option<HashMap<String, Some(String)>>
    /// NFS Capabilities.
    ///
    /// See `include/linux/nfs_fs_sb.h`
    ///
    /// Some known values:
    /// * caps: server capabilities.  See [NFSServerCaps].
    /// * wtmult: server disk block size
    /// * dtsize: readdir size
    /// * bsize: server block size
    pub caps: Vec<String>,
    // * nfsv4 (NFSv4): Option<HashMap<String, Some(String)>>
    pub sec: Vec<String>,
    pub events: NFSEventCounter,
    pub bytes: NFSByteCounter,
    // * RPC iostats version:
    // * xprt
    // * per-op statistics
    pub per_op_stats: NFSPerOpStats,
}

impl MountNFSStatistics {
    // Keep reading lines until we get to a blank line
    fn from_lines<B: BufRead>(r: &mut Lines<B>, statsver: &str) -> ProcResult<MountNFSStatistics> {
        let mut parsing_per_op = false;

        let mut opts: Option<Vec<String>> = None;
        let mut age = None;
        let mut caps = None;
        let mut sec = None;
        let mut bytes = None;
        let mut events = None;
        let mut per_op = HashMap::new();

        while let Some(Ok(line)) = r.next() {
            let line = line.trim();
            if line.trim() == "" {
                break;
            }
            if !parsing_per_op {
                if line.starts_with("opts:") {
                    opts = Some(line[5..].trim().split(',').map(|s| s.to_string()).collect());
                } else if line.starts_with("age:") {
                    age = Some(Duration::from_secs(from_str!(u64, &line[4..].trim())));
                } else if line.starts_with("caps:") {
                    caps = Some(line[5..].trim().split(',').map(|s| s.to_string()).collect());
                } else if line.starts_with("sec:") {
                    sec = Some(line[4..].trim().split(',').map(|s| s.to_string()).collect());
                } else if line.starts_with("bytes:") {
                    bytes = Some(NFSByteCounter::from_str(line[6..].trim())?);
                } else if line.starts_with("events:") {
                    events = Some(NFSEventCounter::from_str(line[7..].trim())?);
                }
                if line == "per-op statistics" {
                    parsing_per_op = true;
                }
            } else {
                let mut split = line.split(':');
                let name = expect!(split.next()).to_string();
                let stats = NFSOperationStat::from_str(expect!(split.next()))?;
                per_op.insert(name, stats);
            }
        }

        Ok(MountNFSStatistics {
            version: statsver.to_string(),
            opts: expect!(opts, "Failed to find opts field in nfs stats"),
            age: expect!(age, "Failed to find age field in nfs stats"),
            caps: expect!(caps, "Failed to find caps field in nfs stats"),
            sec: expect!(sec, "Failed to find sec field in nfs stats"),
            events: expect!(events, "Failed to find events section in nfs stats"),
            bytes: expect!(bytes, "Failed to find bytes section in nfs stats"),
            per_op_stats: per_op,
        })
    }

    /// Attempts to parse the caps= value from the [caps](struct.MountNFSStatistics.html#structfield.caps) field.
    pub fn server_caps(&self) -> ProcResult<Option<NFSServerCaps>> {
        for data in &self.caps {
            if data.starts_with("caps=0x") {
                let val = from_str!(u32, &data[7..], 16);
                return Ok(NFSServerCaps::from_bits(val));
            }
        }
        Ok(None)
    }
}

/// Represents NFS data from `/proc/<pid>/mountstats` under the section `events`.
///
/// The underlying data structure in the kernel can be found under *fs/nfs/iostat.h* `nfs_iostat`.
/// The fields are documented in the kernel source only under *include/linux/nfs_iostat.h* `enum
/// nfs_stat_eventcounters`.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct NFSEventCounter {
    pub inode_revalidate: libc::c_ulong,
    pub deny_try_revalidate: libc::c_ulong,
    pub data_invalidate: libc::c_ulong,
    pub attr_invalidate: libc::c_ulong,
    pub vfs_open: libc::c_ulong,
    pub vfs_lookup: libc::c_ulong,
    pub vfs_access: libc::c_ulong,
    pub vfs_update_page: libc::c_ulong,
    pub vfs_read_page: libc::c_ulong,
    pub vfs_read_pages: libc::c_ulong,
    pub vfs_write_page: libc::c_ulong,
    pub vfs_write_pages: libc::c_ulong,
    pub vfs_get_dents: libc::c_ulong,
    pub vfs_set_attr: libc::c_ulong,
    pub vfs_flush: libc::c_ulong,
    pub vfs_fs_sync: libc::c_ulong,
    pub vfs_lock: libc::c_ulong,
    pub vfs_release: libc::c_ulong,
    pub congestion_wait: libc::c_ulong,
    pub set_attr_trunc: libc::c_ulong,
    pub extend_write: libc::c_ulong,
    pub silly_rename: libc::c_ulong,
    pub short_read: libc::c_ulong,
    pub short_write: libc::c_ulong,
    pub delay: libc::c_ulong,
    pub pnfs_read: libc::c_ulong,
    pub pnfs_write: libc::c_ulong,
}

impl NFSEventCounter {
    fn from_str(s: &str) -> ProcResult<NFSEventCounter> {
        use libc::c_ulong;
        let mut s = s.split_whitespace();
        Ok(NFSEventCounter {
            inode_revalidate: from_str!(c_ulong, expect!(s.next())),
            deny_try_revalidate: from_str!(c_ulong, expect!(s.next())),
            data_invalidate: from_str!(c_ulong, expect!(s.next())),
            attr_invalidate: from_str!(c_ulong, expect!(s.next())),
            vfs_open: from_str!(c_ulong, expect!(s.next())),
            vfs_lookup: from_str!(c_ulong, expect!(s.next())),
            vfs_access: from_str!(c_ulong, expect!(s.next())),
            vfs_update_page: from_str!(c_ulong, expect!(s.next())),
            vfs_read_page: from_str!(c_ulong, expect!(s.next())),
            vfs_read_pages: from_str!(c_ulong, expect!(s.next())),
            vfs_write_page: from_str!(c_ulong, expect!(s.next())),
            vfs_write_pages: from_str!(c_ulong, expect!(s.next())),
            vfs_get_dents: from_str!(c_ulong, expect!(s.next())),
            vfs_set_attr: from_str!(c_ulong, expect!(s.next())),
            vfs_flush: from_str!(c_ulong, expect!(s.next())),
            vfs_fs_sync: from_str!(c_ulong, expect!(s.next())),
            vfs_lock: from_str!(c_ulong, expect!(s.next())),
            vfs_release: from_str!(c_ulong, expect!(s.next())),
            congestion_wait: from_str!(c_ulong, expect!(s.next())),
            set_attr_trunc: from_str!(c_ulong, expect!(s.next())),
            extend_write: from_str!(c_ulong, expect!(s.next())),
            silly_rename: from_str!(c_ulong, expect!(s.next())),
            short_read: from_str!(c_ulong, expect!(s.next())),
            short_write: from_str!(c_ulong, expect!(s.next())),
            delay: from_str!(c_ulong, expect!(s.next())),
            pnfs_read: from_str!(c_ulong, expect!(s.next())),
            pnfs_write: from_str!(c_ulong, expect!(s.next())),
        })
    }
}

/// Represents NFS data from `/proc/<pid>/mountstats` under the section `bytes`.
///
/// The underlying data structure in the kernel can be found under *fs/nfs/iostat.h* `nfs_iostat`.
/// The fields are documented in the kernel source only under *include/linux/nfs_iostat.h* `enum
/// nfs_stat_bytecounters`
#[derive(Debug, Copy, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct NFSByteCounter {
    pub normal_read: libc::c_ulonglong,
    pub normal_write: libc::c_ulonglong,
    pub direct_read: libc::c_ulonglong,
    pub direct_write: libc::c_ulonglong,
    pub server_read: libc::c_ulonglong,
    pub server_write: libc::c_ulonglong,
    pub pages_read: libc::c_ulonglong,
    pub pages_write: libc::c_ulonglong,
}

impl NFSByteCounter {
    fn from_str(s: &str) -> ProcResult<NFSByteCounter> {
        use libc::c_ulonglong;
        let mut s = s.split_whitespace();
        Ok(NFSByteCounter {
            normal_read: from_str!(c_ulonglong, expect!(s.next())),
            normal_write: from_str!(c_ulonglong, expect!(s.next())),
            direct_read: from_str!(c_ulonglong, expect!(s.next())),
            direct_write: from_str!(c_ulonglong, expect!(s.next())),
            server_read: from_str!(c_ulonglong, expect!(s.next())),
            server_write: from_str!(c_ulonglong, expect!(s.next())),
            pages_read: from_str!(c_ulonglong, expect!(s.next())),
            pages_write: from_str!(c_ulonglong, expect!(s.next())),
        })
    }
}

/// Represents NFS data from `/proc/<pid>/mountstats` under the section of `per-op statistics`.
///
/// Here is what the Kernel says about the attributes:
///
/// Regarding `operations`, `transmissions` and `major_timeouts`:
///
/// >  These counters give an idea about how many request
/// >  transmissions are required, on average, to complete that
/// >  particular procedure.  Some procedures may require more
/// >  than one transmission because the server is unresponsive,
/// >  the client is retransmitting too aggressively, or the
/// >  requests are large and the network is congested.
///
/// Regarding `bytes_sent` and `bytes_recv`:
///
/// >  These count how many bytes are sent and received for a
/// >  given RPC procedure type.  This indicates how much load a
/// >  particular procedure is putting on the network.  These
/// >  counts include the RPC and ULP headers, and the request
/// >  payload.
///
/// Regarding `cum_queue_time`, `cum_resp_time` and `cum_total_req_time`:
///
/// >  The length of time an RPC request waits in queue before
/// >  transmission, the network + server latency of the request,
/// >  and the total time the request spent from init to release
/// >  are measured.
///
/// (source: *include/linux/sunrpc/metrics.h* `struct rpc_iostats`)
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct NFSOperationStat {
    /// Count of rpc operations.
    pub operations: libc::c_ulong,
    /// Count of rpc transmissions
    pub transmissions: libc::c_ulong,
    /// Count of rpc major timeouts
    pub major_timeouts: libc::c_ulong,
    /// Count of bytes send. Does not only include the RPC payload but the RPC headers as well.
    pub bytes_sent: libc::c_ulonglong,
    /// Count of bytes received as `bytes_sent`.
    pub bytes_recv: libc::c_ulonglong,
    /// How long all requests have spend in the queue before being send.
    pub cum_queue_time: Duration,
    /// How long it took to get a response back.
    pub cum_resp_time: Duration,
    /// How long all requests have taken from beeing queued to the point they where completely
    /// handled.
    pub cum_total_req_time: Duration,
}

impl NFSOperationStat {
    fn from_str(s: &str) -> ProcResult<NFSOperationStat> {
        use libc::{c_ulong, c_ulonglong};
        let mut s = s.split_whitespace();

        let operations = from_str!(c_ulong, expect!(s.next()));
        let transmissions = from_str!(c_ulong, expect!(s.next()));
        let major_timeouts = from_str!(c_ulong, expect!(s.next()));
        let bytes_sent = from_str!(c_ulonglong, expect!(s.next()));
        let bytes_recv = from_str!(c_ulonglong, expect!(s.next()));
        let cum_queue_time_ms = from_str!(u64, expect!(s.next()));
        let cum_resp_time_ms = from_str!(u64, expect!(s.next()));
        let cum_total_req_time_ms = from_str!(u64, expect!(s.next()));

        Ok(NFSOperationStat {
            operations,
            transmissions,
            major_timeouts,
            bytes_sent,
            bytes_recv,
            cum_queue_time: Duration::from_millis(cum_queue_time_ms),
            cum_resp_time: Duration::from_millis(cum_resp_time_ms),
            cum_total_req_time: Duration::from_millis(cum_total_req_time_ms),
        })
    }
}

pub type NFSPerOpStats = HashMap<String, NFSOperationStat>;

#[cfg(test)]
mod tests {
    use crate::process::*;
    use std::time::Duration;

    #[test]
    fn test_mountinfo() {
        let s = "25 0 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw,errors=remount-ro";

        let stat = MountInfo::from_line(s).unwrap();
        println!("{:?}", stat);
    }

    #[test]
    fn test_mountinfo_live() {
        let me = Process::myself().unwrap();
        let mounts = me.mountinfo().unwrap();
        println!("{:#?}", mounts);
    }

    #[test]
    fn test_proc_mountstats() {
        let simple = MountStat::from_reader(
            "device /dev/md127 mounted on /boot with fstype ext2 
device /dev/md124 mounted on /home with fstype ext4 
device tmpfs mounted on /run/user/0 with fstype tmpfs 
"
            .as_bytes(),
        )
        .unwrap();
        let simple_parsed = vec![
            MountStat {
                device: Some("/dev/md127".to_string()),
                mount_point: PathBuf::from("/boot"),
                fs: "ext2".to_string(),
                statistics: None,
            },
            MountStat {
                device: Some("/dev/md124".to_string()),
                mount_point: PathBuf::from("/home"),
                fs: "ext4".to_string(),
                statistics: None,
            },
            MountStat {
                device: Some("tmpfs".to_string()),
                mount_point: PathBuf::from("/run/user/0"),
                fs: "tmpfs".to_string(),
                statistics: None,
            },
        ];
        assert_eq!(simple, simple_parsed);
        let mountstats = MountStat::from_reader("device elwe:/space mounted on /srv/elwe/space with fstype nfs4 statvers=1.1 
       opts:   rw,vers=4.1,rsize=131072,wsize=131072,namlen=255,acregmin=3,acregmax=60,acdirmin=30,acdirmax=60,hard,proto=tcp,port=0,timeo=600,retrans=2,sec=krb5,clientaddr=10.0.1.77,local_lock=none 
       age:    3542 
       impl_id:        name='',domain='',date='0,0' 
       caps:   caps=0x3ffdf,wtmult=512,dtsize=32768,bsize=0,namlen=255 
       nfsv4:  bm0=0xfdffbfff,bm1=0x40f9be3e,bm2=0x803,acl=0x3,sessions,pnfs=not configured 
       sec:    flavor=6,pseudoflavor=390003 
       events: 114 1579 5 3 132 20 3019 1 2 3 4 5 115 1 4 1 2 4 3 4 5 6 7 8 9 0 1  
       bytes:  1 2 3 4 5 6 7 8  
       RPC iostats version: 1.0  p/v: 100003/4 (nfs) 
       xprt:   tcp 909 0 1 0 2 294 294 0 294 0 2 0 0 
       per-op statistics 
               NULL: 0 0 0 0 0 0 0 0 
               READ: 1 2 3 4 5 6 7 8 
              WRITE: 0 0 0 0 0 0 0 0 
             COMMIT: 0 0 0 0 0 0 0 0 
               OPEN: 1 1 0 320 420 0 124 124 
        ".as_bytes()).unwrap();
        let nfs_v4 = &mountstats[0];
        match &nfs_v4.statistics {
            Some(stats) => {
                assert_eq!("1.1".to_string(), stats.version, "mountstats version wrongly parsed.");
                assert_eq!(Duration::from_secs(3542), stats.age);
                assert_eq!(1, stats.bytes.normal_read);
                assert_eq!(114, stats.events.inode_revalidate);
                assert!(stats.server_caps().unwrap().is_some());
            }
            None => {
                panic!("Failed to retrieve nfs statistics");
            }
        }
    }
    #[test]
    fn test_proc_mountstats_live() {
        // this tries to parse a live mountstats file
        // there are no assertions, but we still want to check for parsing errors (which can
        // cause panics)

        let stats = MountStat::from_reader(FileWrapper::open("/proc/self/mountstats").unwrap()).unwrap();
        for stat in stats {
            println!("{:#?}", stat);
            if let Some(nfs) = stat.statistics {
                println!("  {:?}", nfs.server_caps().unwrap());
            }
        }
    }
}
