//! Functions and structs related to process information
//!
//! The primary source of data for functions in this module is the files in a `/proc/<pid>/`
//! directory.  If you have a process ID, you can use
//! [`Process::new(pid)`](struct.Process.html#method.new), otherwise you can get a
//! list of all running processes using [`all_processes()`](fn.all_processes.html).
//!
//! In case you have procfs filesystem mounted to a location other than `/proc`,
//! use [`Process::new_with_root()`](struct.Process.html#method.new_with_root).
//!
//! # Examples
//!
//! Here's a small example that prints out all processes that are running on the same tty as the calling
//! process.  This is very similar to what "ps" does in its default mode.  You can run this example
//! yourself with:
//!
//! > cargo run --example=ps
//!
//! ```rust
//! let me = procfs::process::Process::myself().unwrap();
//! let tps = procfs::ticks_per_second().unwrap();
//!
//! println!("{: >5} {: <8} {: >8} {}", "PID", "TTY", "TIME", "CMD");
//!
//! let tty = format!("pty/{}", me.stat.tty_nr().1);
//! for prc in procfs::process::all_processes().unwrap() {
//!     if prc.stat.tty_nr == me.stat.tty_nr {
//!         // total_time is in seconds
//!         let total_time =
//!             (prc.stat.utime + prc.stat.stime) as f32 / (tps as f32);
//!         println!(
//!             "{: >5} {: <8} {: >8} {}",
//!             prc.stat.pid, tty, total_time, prc.stat.comm
//!         );
//!     }
//! }
//! ```
//!
//! Here's a simple example of how you could get the total memory used by the current process.
//! There are several ways to do this.  For a longer example, see the `examples/self_memory.rs`
//! file in the git repository.  You can run this example with:
//!
//! > cargo run --example=self_memory
//!
//! ```rust
//! # use procfs::process::Process;
//! let me = Process::myself().unwrap();
//! let page_size = procfs::page_size().unwrap() as u64;
//!
//! println!("== Data from /proc/self/stat:");
//! println!("Total virtual memory used: {} bytes", me.stat.vsize);
//! println!("Total resident set: {} pages ({} bytes)", me.stat.rss, me.stat.rss as u64 * page_size);
//! ```

use super::*;
use crate::from_iter;

use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::fs::read_link;
use std::io::{self, Read};
#[cfg(target_os = "android")]
use std::os::android::fs::MetadataExt;
#[cfg(all(unix, not(target_os = "android")))]
use std::os::linux::fs::MetadataExt;
use std::path::PathBuf;
use std::str::FromStr;

mod limit;
pub use limit::*;

mod stat;
pub use stat::*;

mod mount;
pub use mount::*;

mod namespaces;
pub use namespaces::*;

mod status;
pub use status::*;

mod schedstat;
pub use schedstat::*;

mod task;
pub use task::*;

// provide a type-compatible st_uid for windows
#[cfg(windows)]
trait FakeMedatadataExt {
    fn st_uid(&self) -> u32;
}
#[cfg(windows)]
impl FakeMedatadataExt for std::fs::Metadata {
    fn st_uid(&self) -> u32 {
        panic!()
    }
}

bitflags! {
    /// Kernel flags for a process
    ///
    /// See also the [Stat::flags()] method.
    pub struct StatFlags: u32 {
        /// I am an IDLE thread
        const PF_IDLE = 0x0000_0002;
        /// Getting shut down
        const PF_EXITING = 0x0000_0004;
        /// PI exit done on shut down
        const PF_EXITPIDONE = 0x0000_0008;
        /// I'm a virtual CPU
        const PF_VCPU = 0x0000_0010;
        /// I'm a workqueue worker
        const PF_WQ_WORKER = 0x0000_0020;
        /// Forked but didn't exec
        const PF_FORKNOEXEC = 0x0000_0040;
        /// Process policy on mce errors;
        const PF_MCE_PROCESS = 0x0000_0080;
        /// Used super-user privileges
        const PF_SUPERPRIV = 0x0000_0100;
        /// Dumped core
        const PF_DUMPCORE = 0x0000_0200;
        /// Killed by a signal
        const PF_SIGNALED = 0x0000_0400;
        ///Allocating memory
        const PF_MEMALLOC = 0x0000_0800;
        /// set_user() noticed that RLIMIT_NPROC was exceeded
        const PF_NPROC_EXCEEDED = 0x0000_1000;
        /// If unset the fpu must be initialized before use
        const PF_USED_MATH = 0x0000_2000;
         /// Used async_schedule*(), used by module init
        const PF_USED_ASYNC = 0x0000_4000;
        ///  This thread should not be frozen
        const PF_NOFREEZE = 0x0000_8000;
        /// Frozen for system suspend
        const PF_FROZEN = 0x0001_0000;
        /// I am kswapd
        const PF_KSWAPD = 0x0002_0000;
        /// All allocation requests will inherit GFP_NOFS
        const PF_MEMALLOC_NOFS = 0x0004_0000;
        /// All allocation requests will inherit GFP_NOIO
        const PF_MEMALLOC_NOIO = 0x0008_0000;
        /// Throttle me less: I clean memory
        const PF_LESS_THROTTLE = 0x0010_0000;
        /// I am a kernel thread
        const PF_KTHREAD = 0x0020_0000;
        /// Randomize virtual address space
        const PF_RANDOMIZE = 0x0040_0000;
        /// Allowed to write to swap
        const PF_SWAPWRITE = 0x0080_0000;
        /// Stalled due to lack of memory
        const PF_MEMSTALL = 0x0100_0000;
        /// I'm an Usermodehelper process
        const PF_UMH = 0x0200_0000;
        /// Userland is not allowed to meddle with cpus_allowed
        const PF_NO_SETAFFINITY = 0x0400_0000;
        /// Early kill for mce process policy
        const PF_MCE_EARLY = 0x0800_0000;
        /// All allocation request will have _GFP_MOVABLE cleared
        const PF_MEMALLOC_NOCMA = 0x1000_0000;
        /// Thread belongs to the rt mutex tester
        const PF_MUTEX_TESTER = 0x2000_0000;
        /// Freezer should not count it as freezable
        const PF_FREEZER_SKIP = 0x4000_0000;
        /// This thread called freeze_processes() and should not be frozen
        const PF_SUSPEND_TASK = 0x8000_0000;

    }
}
bitflags! {

    /// See the [coredump_filter()](struct.Process.html#method.coredump_filter) method.
    pub struct CoredumpFlags: u32 {
        const ANONYMOUS_PRIVATE_MAPPINGS = 0x01;
        const ANONYMOUS_SHARED_MAPPINGS = 0x02;
        const FILEBACKED_PRIVATE_MAPPINGS = 0x04;
        const FILEBACKED_SHARED_MAPPINGS = 0x08;
        const ELF_HEADERS = 0x10;
        const PROVATE_HUGEPAGES = 0x20;
        const SHARED_HUGEPAGES = 0x40;
        const PRIVATE_DAX_PAGES = 0x80;
        const SHARED_DAX_PAGES = 0x100;
    }
}

bitflags! {
    /// The mode (read/write permissions) for an open file descriptor
    pub struct FDPermissions: libc::mode_t {
        const READ = libc::S_IRUSR;
        const WRITE = libc::S_IWUSR;
        const EXECUTE = libc::S_IXUSR;
    }
}

bitflags! {
    /// Represents the kernel flags associated with the virtual memory area.
    /// The names of these flags are just those you'll find in the man page, but in upper case.
    pub struct VmFlags: u32 {
        /// Invalid flags
        const INVALID = 0;
        /// Readable
        const RD = 1 << 0;
        /// Writable
        const WR = 1 << 1;
        /// Executable
        const EX = 1 << 2;
        /// Shared
        const SH = 1 << 3;
        /// May read
        const MR = 1 << 4;
        /// May write
        const MW = 1 << 5;
        /// May execute
        const ME = 1 << 6;
        /// May share
        const MS = 1 << 7;
        /// Stack segment grows down
        const GD = 1 << 8;
        /// Pure PFN range
        const PF = 1 << 9;
        /// Disable write to the mapped file
        const DW = 1 << 10;
        /// Pages are locked in memory
        const LO = 1 << 11;
        /// Memory mapped I/O area
        const IO = 1 << 12;
        /// Sequential read advise provided
        const SR = 1 << 13;
        /// Random read provided
        const RR = 1 << 14;
        /// Do not copy area on fork
        const DC = 1 << 15;
        /// Do not expand area on remapping
        const DE = 1 << 16;
        /// Area is accountable
        const AC = 1 << 17;
        /// Swap space is not reserved for the area
        const NR = 1 << 18;
        /// Area uses huge TLB pages
        const HT = 1 << 19;
        /// Perform synchronous page faults (since Linux 4.15)
        const SF = 1 << 20;
        /// Non-linear mapping (removed in Linux 4.0)
        const NL = 1 << 21;
        /// Architecture specific flag
        const AR = 1 << 22;
        /// Wipe on fork (since Linux 4.14)
        const WF = 1 << 23;
        /// Do not include area into core dump
        const DD = 1 << 24;
        /// Soft-dirty flag (since Linux 3.13)
        const SD = 1 << 25;
        /// Mixed map area
        const MM = 1 << 26;
        /// Huge page advise flag
        const HG = 1 << 27;
        /// No-huge page advise flag
        const NH = 1 << 28;
        /// Mergeable advise flag
        const MG = 1 << 29;
        /// Userfaultfd missing pages tracking (since Linux 4.3)
        const UM = 1 << 30;
        /// Userfaultfd wprotect pages tracking (since Linux 4.3)
        const UW = 1 << 31;
    }
}

impl VmFlags {
    fn from_str(flag: &str) -> Option<Self> {
        if flag.len() != 2 {
            return None;
        }

        match flag {
            "rd" => Some(VmFlags::RD),
            "wr" => Some(VmFlags::WR),
            "ex" => Some(VmFlags::EX),
            "sh" => Some(VmFlags::SH),
            "mr" => Some(VmFlags::MR),
            "mw" => Some(VmFlags::MW),
            "me" => Some(VmFlags::ME),
            "ms" => Some(VmFlags::MS),
            "gd" => Some(VmFlags::GD),
            "pf" => Some(VmFlags::PF),
            "dw" => Some(VmFlags::DW),
            "lo" => Some(VmFlags::LO),
            "io" => Some(VmFlags::IO),
            "sr" => Some(VmFlags::SR),
            "rr" => Some(VmFlags::RR),
            "dc" => Some(VmFlags::DC),
            "de" => Some(VmFlags::DE),
            "ac" => Some(VmFlags::AC),
            "nr" => Some(VmFlags::NR),
            "ht" => Some(VmFlags::HT),
            "sf" => Some(VmFlags::SF),
            "nl" => Some(VmFlags::NL),
            "ar" => Some(VmFlags::AR),
            "wf" => Some(VmFlags::WF),
            "dd" => Some(VmFlags::DD),
            "sd" => Some(VmFlags::SD),
            "mm" => Some(VmFlags::MM),
            "hg" => Some(VmFlags::HG),
            "nh" => Some(VmFlags::NH),
            "mg" => Some(VmFlags::MG),
            "um" => Some(VmFlags::UM),
            "uw" => Some(VmFlags::UW),
            _ => None,
        }
    }
}

//impl<'a, 'b, T> ProcFrom<&'b mut T> for u32 where T: Iterator<Item=&'a str> + Sized, 'a: 'b {
//    fn from(i: &'b mut T) -> u32 {
//        let s = i.next().unwrap();
//        u32::from_str_radix(s, 10).unwrap()
//    }
//}

//impl<'a> ProcFrom<&'a str> for u32 {
//    fn from(s: &str) -> Self {
//        u32::from_str_radix(s, 10).unwrap()
//    }
//}

//fn from_iter<'a, I: Iterator<Item=&'a str>>(i: &mut I) -> u32 {
//    u32::from_str_radix(i.next().unwrap(), 10).unwrap()
//}

/// Represents the state of a process.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProcState {
    /// Running (R)
    Running,
    /// Sleeping in an interruptible wait (S)
    Sleeping,
    /// Waiting in uninterruptible disk sleep (D)
    Waiting,
    /// Zombie (Z)
    Zombie,
    /// Stopped (on a signal) (T)
    ///
    /// Or before Linux 2.6.33, trace stopped
    Stopped,
    /// Tracing stop (t) (Linux 2.6.33 onward)
    Tracing,
    /// Dead (X)
    Dead,
    /// Wakekill (K) (Linux 2.6.33 to 3.13 only)
    Wakekill,
    /// Waking (W) (Linux 2.6.33 to 3.13 only)
    Waking,
    /// Parked (P) (Linux 3.9 to 3.13 only)
    Parked,
    /// Idle (I)
    Idle,
}

impl ProcState {
    pub fn from_char(c: char) -> Option<ProcState> {
        match c {
            'R' => Some(ProcState::Running),
            'S' => Some(ProcState::Sleeping),
            'D' => Some(ProcState::Waiting),
            'Z' => Some(ProcState::Zombie),
            'T' => Some(ProcState::Stopped),
            't' => Some(ProcState::Tracing),
            'X' | 'x' => Some(ProcState::Dead),
            'K' => Some(ProcState::Wakekill),
            'W' => Some(ProcState::Waking),
            'P' => Some(ProcState::Parked),
            'I' => Some(ProcState::Idle),
            _ => None,
        }
    }
}

impl FromStr for ProcState {
    type Err = ProcError;
    fn from_str(s: &str) -> Result<ProcState, ProcError> {
        ProcState::from_char(expect!(s.chars().next(), "empty string"))
            .ok_or_else(|| build_internal_error!("failed to convert"))
    }
}

//impl<'a, 'b, T> ProcFrom<&'b mut T> for ProcState where T: Iterator<Item=&'a str>, 'a: 'b {
//    fn from(s: &'b mut T) -> ProcState {
//        ProcState::from_str(s.next().unwrap()).unwrap()
//    }
//}

/// This struct contains I/O statistics for the process, built from `/proc/<pid>/io`
///
/// To construct this structure, see [Process::io()].
///
/// #  Note
///
/// In the current implementation, things are a bit racy on 32-bit systems: if process A
/// reads process B's `/proc/<pid>/io` while process  B is updating one of these 64-bit
/// counters, process A could see an intermediate result.
#[derive(Debug, Copy, Clone)]
pub struct Io {
    /// Characters read
    ///
    /// The number of bytes which this task has caused to be read from storage.  This is simply the
    /// sum of bytes which this process passed to read(2)  and  similar system calls.  It includes
    /// things such as terminal I/O and is unaffected by whether or not actual physical disk I/O
    /// was required (the read might have been satisfied from pagecache).
    pub rchar: u64,

    /// characters written
    ///
    /// The number of bytes which this task has caused, or shall cause to be written to disk.
    /// Similar caveats apply here as with rchar.
    pub wchar: u64,
    /// read syscalls
    ///
    /// Attempt to count the number of write I/O operations—that is, system calls such as write(2)
    /// and pwrite(2).
    pub syscr: u64,
    /// write syscalls
    ///
    /// Attempt to count the number of write I/O operations—that is, system calls such as write(2)
    /// and pwrite(2).
    pub syscw: u64,
    /// bytes read
    ///
    /// Attempt to count the number of bytes which this process really did cause to be fetched from
    /// the storage layer.  This is accurate  for block-backed filesystems.
    pub read_bytes: u64,
    /// bytes written
    ///
    /// Attempt to count the number of bytes which this process caused to be sent to the storage layer.
    pub write_bytes: u64,
    /// Cancelled write bytes.
    ///
    /// The  big inaccuracy here is truncate.  If a process writes 1MB to a file and then deletes
    /// the file, it will in fact perform no write‐ out.  But it will have been accounted as having
    /// caused 1MB of write.  In other words: this field represents the number of bytes which this
    /// process caused to not happen, by truncating pagecache.  A task can cause "negative" I/O too.
    /// If this task truncates some dirty pagecache, some I/O which another task has been accounted
    /// for (in its write_bytes) will not be happening.
    pub cancelled_write_bytes: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum MMapPath {
    /// The file that is backing the mapping.
    Path(PathBuf),
    /// The process's heap.
    Heap,
    /// The initial process's (also known as the main thread's) stack.
    Stack,
    /// A thread's stack (where the `<tid>` is a thread ID).  It corresponds to the
    /// `/proc/<pid>/task/<tid>/` path.
    ///
    /// (since Linux 3.4)
    TStack(u32),
    /// The virtual dynamically linked shared object.
    Vdso,
    /// Shared kernel variables
    Vvar,
    /// obsolete virtual syscalls, succeeded by vdso
    Vsyscall,
    /// An anonymous mapping as obtained via mmap(2).
    Anonymous,
    /// Shared memory segment
    Vsys(i32),
    /// Some other pseudo-path
    Other(String),
}

impl MMapPath {
    /// Needed for MemoryMap::new().
    fn new() -> MMapPath {
        MMapPath::Anonymous
    }

    fn from(path: &str) -> ProcResult<MMapPath> {
        Ok(match path.trim() {
            "" => MMapPath::Anonymous,
            "[heap]" => MMapPath::Heap,
            "[stack]" => MMapPath::Stack,
            "[vdso]" => MMapPath::Vdso,
            "[vvar]" => MMapPath::Vvar,
            "[vsyscall]" => MMapPath::Vsyscall,
            x if x.starts_with("[stack:") => {
                let mut s = x[1..x.len() - 1].split(':');
                let tid = from_str!(u32, expect!(s.nth(1)));
                MMapPath::TStack(tid)
            }
            x if x.starts_with('[') && x.ends_with(']') => MMapPath::Other(x[1..x.len() - 1].to_string()),
            x if x.starts_with("/SYSV") => MMapPath::Vsys(u32::from_str_radix(&x[5..13], 16)? as i32), // 32bits signed hex. /SYSVaabbccdd (deleted)
            x => MMapPath::Path(PathBuf::from(x)),
        })
    }
}

/// Represents an entry in a `/proc/<pid>/maps` file.
///
/// To construct this structure, see [Process::maps()] and [Process::smaps()].
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct MemoryMap {
    /// The address space in the process that the mapping occupies.
    pub address: (u64, u64),
    pub perms: String,
    /// The offset into the file/whatever
    pub offset: u64,
    /// The device (major, minor)
    pub dev: (i32, i32),
    /// The inode on that device
    ///
    /// 0 indicates that no inode is associated with the memory region, as would be the case with
    /// BSS (uninitialized data).
    pub inode: u64,
    pub pathname: MMapPath,
}

impl MemoryMap {
    /// Used internally in Process::smaps() as a "default value" thing
    fn new() -> Self {
        Self {
            address: (0, 0),
            perms: "".into(),
            offset: 0,
            dev: (0, 0),
            inode: 0,
            pathname: MMapPath::new(),
        }
    }
    fn from_line(line: &str) -> ProcResult<MemoryMap> {
        let mut s = line.splitn(6, ' ');
        let address = expect!(s.next());
        let perms = expect!(s.next());
        let offset = expect!(s.next());
        let dev = expect!(s.next());
        let inode = expect!(s.next());
        let path = expect!(s.next());

        Ok(MemoryMap {
            address: split_into_num(address, '-', 16)?,
            perms: perms.to_string(),
            offset: from_str!(u64, offset, 16),
            dev: split_into_num(dev, ':', 16)?,
            inode: from_str!(u64, inode),
            pathname: MMapPath::from(path)?,
        })
    }
}

/// Represents the information about a specific mapping as presented in /proc/<pid>/smaps
///
/// To construct this structure, see [Process::smaps()]
#[derive(Default, Debug)]
pub struct MemoryMapData {
    /// Key-Value pairs that may represent statistics about memory usage, or other interesting things,
    /// such a "ProtectionKey"(if you're on X86 and that kernel config option was specified).
    ///
    /// Note that should a Key-Value pair represent a memory usage statistic, it will be in bytes.
    ///
    /// Check your manpage for more information
    pub map: HashMap<String, u64>,
    /// Kernel flags associated with the virtual memory area
    ///
    /// (since Linux 3.8)
    pub vm_flags: Option<VmFlags>,
}

impl Io {
    pub fn from_reader<R: io::Read>(r: R) -> ProcResult<Io> {
        let mut map = HashMap::new();
        let reader = BufReader::new(r);

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() || !line.contains(' ') {
                continue;
            }
            let mut s = line.split_whitespace();
            let field = expect!(s.next());
            let value = expect!(s.next());

            let value = from_str!(u64, value);

            map.insert(field[..field.len() - 1].to_string(), value);
        }
        let io = Io {
            rchar: expect!(map.remove("rchar")),
            wchar: expect!(map.remove("wchar")),
            syscr: expect!(map.remove("syscr")),
            syscw: expect!(map.remove("syscw")),
            read_bytes: expect!(map.remove("read_bytes")),
            write_bytes: expect!(map.remove("write_bytes")),
            cancelled_write_bytes: expect!(map.remove("cancelled_write_bytes")),
        };

        assert!(!(cfg!(test) && !map.is_empty()), "io map is not empty: {:#?}", map);

        Ok(io)
    }
}

/// Describes a file descriptor opened by a process.
///
/// See also the [Process::fd()] method.
#[derive(Clone, Debug)]
pub enum FDTarget {
    /// A file or device
    Path(PathBuf),
    /// A socket type, with an inode
    Socket(u64),
    Net(u64),
    Pipe(u64),
    /// A file descriptor that have no corresponding inode.
    AnonInode(String),
    /// A memfd file descriptor with a name.
    MemFD(String),
    /// Some other file descriptor type, with an inode.
    Other(String, u64),
}

impl FromStr for FDTarget {
    type Err = ProcError;
    fn from_str(s: &str) -> Result<FDTarget, ProcError> {
        // helper function that removes the first and last character
        fn strip_first_last(s: &str) -> ProcResult<&str> {
            if s.len() > 2 {
                let mut c = s.chars();
                // remove the first and last characters
                let _ = c.next();
                let _ = c.next_back();
                Ok(c.as_str())
            } else {
                Err(ProcError::Incomplete(None))
            }
        }

        if !s.starts_with('/') && s.contains(':') {
            let mut s = s.split(':');
            let fd_type = expect!(s.next());
            match fd_type {
                "socket" => {
                    let inode = expect!(s.next(), "socket inode");
                    let inode = expect!(u64::from_str_radix(strip_first_last(inode)?, 10));
                    Ok(FDTarget::Socket(inode))
                }
                "net" => {
                    let inode = expect!(s.next(), "net inode");
                    let inode = expect!(u64::from_str_radix(strip_first_last(inode)?, 10));
                    Ok(FDTarget::Net(inode))
                }
                "pipe" => {
                    let inode = expect!(s.next(), "pipe inode");
                    let inode = expect!(u64::from_str_radix(strip_first_last(inode)?, 10));
                    Ok(FDTarget::Pipe(inode))
                }
                "anon_inode" => Ok(FDTarget::AnonInode(expect!(s.next(), "anon inode").to_string())),
                "/memfd" => Ok(FDTarget::MemFD(expect!(s.next(), "memfd name").to_string())),
                "" => Err(ProcError::Incomplete(None)),
                x => {
                    let inode = expect!(s.next(), "other inode");
                    let inode = expect!(u64::from_str_radix(strip_first_last(inode)?, 10));
                    Ok(FDTarget::Other(x.to_string(), inode))
                }
            }
        } else {
            Ok(FDTarget::Path(PathBuf::from(s)))
        }
    }
}

/// See the [Process::fd()] method
#[derive(Clone)]
pub struct FDInfo {
    /// The file descriptor
    pub fd: u32,
    /// The permission bits for this FD
    ///
    /// **Note**: this field is only the owner read/write/execute bits.  All the other bits
    /// (include filetype bits) are masked out.  See also the `mode()` method.
    pub mode: libc::mode_t,
    pub target: FDTarget,
}

impl FDInfo {
    /// Gets a file descriptor from a raw fd
    pub fn from_raw_fd(pid: pid_t, raw_fd: i32) -> ProcResult<Self> {
        Self::from_raw_fd_with_root("/proc", pid, raw_fd)
    }

    /// Gets a file descriptor from a raw fd based on a specified `/proc` path
    pub fn from_raw_fd_with_root(root: impl AsRef<Path>, pid: pid_t, raw_fd: i32) -> ProcResult<Self> {
        let path = root.as_ref().join(pid.to_string()).join("fd").join(raw_fd.to_string());
        let link = wrap_io_error!(path, read_link(&path))?;
        let md = wrap_io_error!(path, path.symlink_metadata())?;
        let link_os: &OsStr = link.as_ref();
        Ok(Self {
            fd: raw_fd as u32,
            mode: (md.st_mode() as libc::mode_t) & libc::S_IRWXU,
            target: expect!(FDTarget::from_str(expect!(link_os.to_str()))),
        })
    }

    /// Gets the read/write mode of this file descriptor as a bitfield
    pub fn mode(&self) -> FDPermissions {
        FDPermissions::from_bits_truncate(self.mode)
    }
}

impl std::fmt::Debug for FDInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FDInfo {{ fd: {:?}, mode: 0{:o}, target: {:?} }}",
            self.fd, self.mode, self.target
        )
    }
}

/// Represents a process in `/proc/<pid>`.
///
/// The `stat` structure is pre-populated because it's useful info, but other data is loaded on
/// demand (and so might fail, if the process no longer exist).
#[derive(Debug, Clone)]
pub struct Process {
    /// The process ID
    ///
    /// (same as the `Stat.pid` field).
    pub pid: i32,
    /// Process status, based on the `/proc/<pid>/stat` file.
    pub stat: Stat,
    /// The user id of the owner of this process
    pub owner: u32,
    pub(crate) root: PathBuf,
}

impl Process {
    /// Returns a `Process` based on a specified PID.
    ///
    /// This can fail if the process doesn't exist, or if you don't have permission to access it.
    pub fn new(pid: pid_t) -> ProcResult<Process> {
        let root = PathBuf::from("/proc").join(format!("{}", pid));
        Self::new_with_root(root)
    }

    /// Returns a `Process` based on a specified `/proc/<pid>` path.
    pub fn new_with_root(root: PathBuf) -> ProcResult<Process> {
        let path = root.join("stat");
        let stat = Stat::from_reader(FileWrapper::open(&path)?)?;

        let md = std::fs::metadata(&root)?;

        Ok(Process {
            pid: stat.pid,
            root,
            stat,
            owner: md.st_uid(),
        })
    }

    /// Returns a `Process` for the currently running process.
    ///
    /// This is done by using the `/proc/self` symlink
    pub fn myself() -> ProcResult<Process> {
        let root = PathBuf::from("/proc/self");
        Self::new_with_root(root)
    }

    /// Returns the complete command line for the process, unless the process is a zombie.
    ///
    ///
    pub fn cmdline(&self) -> ProcResult<Vec<String>> {
        let mut buf = String::new();
        let mut f = FileWrapper::open(self.root.join("cmdline"))?;
        f.read_to_string(&mut buf)?;
        Ok(buf
            .split('\0')
            .filter_map(|s| if !s.is_empty() { Some(s.to_string()) } else { None })
            .collect())
    }

    /// Returns the process ID for this process.
    pub fn pid(&self) -> pid_t {
        self.stat.pid
    }

    /// Is this process still alive?
    pub fn is_alive(&self) -> bool {
        match Process::new(self.pid()) {
            Ok(prc) => {
                // assume that the command line, uid and starttime don't change during a processes lifetime
                // additionally, do not consider defunct processes as "alive"
                // i.e. if they are different, a new process has the same PID as `self` and so `self` is not considered alive
                prc.stat.comm == self.stat.comm
                    && prc.owner == self.owner
                    && prc.stat.starttime == self.stat.starttime
                    && prc.stat.state().map(|s| s != ProcState::Zombie).unwrap_or(false)
                    && self.stat.state().map(|s| s != ProcState::Zombie).unwrap_or(false)
            }
            _ => false,
        }
    }

    /// Retrieves current working directory of the process by dereferencing `/proc/<pid>/cwd` symbolic link.
    ///
    /// This method has the following caveats:
    ///
    /// * if the pathname has been unlinked, the symbolic link will contain the string " (deleted)"
    ///   appended to the original pathname
    ///
    /// * in a multithreaded process, the contents of this symbolic link are not available if the
    ///   main thread has already terminated (typically by calling `pthread_exit(3)`)
    ///
    /// * permission to dereference or read this symbolic link is governed by a
    ///   `ptrace(2)` access mode `PTRACE_MODE_READ_FSCREDS` check
    pub fn cwd(&self) -> ProcResult<PathBuf> {
        Ok(std::fs::read_link(self.root.join("cwd"))?)
    }

    /// Retrieves current root directory of the process by dereferencing `/proc/<pid>/root` symbolic link.
    ///
    /// This method has the following caveats:
    ///
    /// * if the pathname has been unlinked, the symbolic link will contain the string " (deleted)"
    ///   appended to the original pathname
    ///
    /// * in a multithreaded process, the contents of this symbolic link are not available if the
    ///   main thread has already terminated (typically by calling `pthread_exit(3)`)
    ///
    /// * permission to dereference or read this symbolic link is governed by a
    ///   `ptrace(2)` access mode `PTRACE_MODE_READ_FSCREDS` check
    pub fn root(&self) -> ProcResult<PathBuf> {
        Ok(std::fs::read_link(self.root.join("root"))?)
    }

    /// Gets the current environment for the process.  This is done by reading the
    /// `/proc/pid/environ` file.
    pub fn environ(&self) -> ProcResult<HashMap<OsString, OsString>> {
        use std::os::unix::ffi::OsStrExt;

        let mut map = HashMap::new();

        let mut file = FileWrapper::open(self.root.join("environ"))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        for slice in buf.split(|b| *b == 0) {
            // slice will be in the form key=var, so split on the first equals sign
            let mut split = slice.splitn(2, |b| *b == b'=');
            if let (Some(k), Some(v)) = (split.next(), split.next()) {
                map.insert(OsStr::from_bytes(k).to_os_string(), OsStr::from_bytes(v).to_os_string());
            };
            //let env = OsStr::from_bytes(slice);
        }

        Ok(map)
    }

    /// Retrieves the actual path of the executed command by dereferencing `/proc/<pid>/exe` symbolic link.
    ///
    /// This method has the following caveats:
    ///
    /// * if the pathname has been unlinked, the symbolic link will contain the string " (deleted)"
    ///   appended to the original pathname
    ///
    /// * in a multithreaded process, the contents of this symbolic link are not available if the
    ///   main thread has already terminated (typically by calling `pthread_exit(3)`)
    ///
    /// * permission to dereference or read this symbolic link is governed by a
    ///   `ptrace(2)` access mode `PTRACE_MODE_READ_FSCREDS` check
    pub fn exe(&self) -> ProcResult<PathBuf> {
        Ok(std::fs::read_link(self.root.join("exe"))?)
    }

    /// Return the Io stats for this process, based on the `/proc/pid/io` file.
    ///
    /// (since kernel 2.6.20)
    pub fn io(&self) -> ProcResult<Io> {
        let path = self.root.join("io");
        let file = FileWrapper::open(&path)?;
        Io::from_reader(file)
    }

    /// Return a list of the currently mapped memory regions and their access permissions, based on
    /// the `/proc/pid/maps` file.
    pub fn maps(&self) -> ProcResult<Vec<MemoryMap>> {
        let path = self.root.join("maps");
        let file = FileWrapper::open(&path)?;

        let reader = BufReader::new(file);

        let mut vec = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|_| ProcError::Incomplete(Some(path.clone())))?;
            vec.push(MemoryMap::from_line(&line)?);
        }

        Ok(vec)
    }

    /// Returns a list of currently mapped memory regions and verbose information about them,
    /// such as memory consumption per mapping, based on the `/proc/pid/smaps` file.
    ///
    /// (since Linux 2.6.14 and requires CONFIG_PROG_PAGE_MONITOR)
    pub fn smaps(&self) -> ProcResult<Vec<(MemoryMap, MemoryMapData)>> {
        let path = self.root.join("smaps");
        let file = FileWrapper::open(&path)?;

        let reader = BufReader::new(file);

        let mut vec: Vec<(MemoryMap, MemoryMapData)> = Vec::new();

        let mut current_mapping = MemoryMap::new();
        let mut current_data = Default::default();
        for line in reader.lines() {
            let line = line.map_err(|_| ProcError::Incomplete(Some(path.clone())))?;

            if let Ok(mapping) = MemoryMap::from_line(&line) {
                vec.push((current_mapping, current_data));
                current_mapping = mapping;
                current_data = Default::default();
            } else {
                // This is probably an attribute
                if line.starts_with("VmFlags") {
                    let flags = line.split_ascii_whitespace();
                    let flags = flags.skip(1); // Skips the `VmFlags:` part since we don't need it.

                    let flags = flags
                        .map(|v| match VmFlags::from_str(v) {
                            None => VmFlags::INVALID,
                            Some(v) => v,
                        })
                        .fold(VmFlags::INVALID, |a, b| a | b);

                    current_data.vm_flags = Some(flags);
                } else {
                    let mut parts = line.split_ascii_whitespace();

                    let key = parts.next();
                    let value = parts.next();

                    if let (Some(k), Some(v)) = (key, value) {
                        // While most entries do have one, not all of them do.
                        let size_suffix = parts.next();

                        // Limited poking at /proc/<pid>/smaps and then checking if "MB", "GB", and "TB" appear in the C file that is
                        // supposedly responsible for creating smaps, has lead me to believe that the only size suffixes we'll ever encounter
                        // "kB", which is most likely kibibytes. Actually checking if the size suffix is any of the above is a way to
                        // future-proof the code, but I am not sure it is worth doing so.
                        let size_multiplier = if size_suffix.is_some() { 1024 } else { 1 };

                        let v = v.parse::<u64>().map_err(|_| {
                            ProcError::Other("Value in `Key: Value` pair was not actually a number".into())
                        })?;

                        // This ignores the case when our Key: Value pairs are really Key Value pairs. Is this a good idea?
                        let k = k.trim_end_matches(':');

                        current_data.map.insert(k.into(), v * size_multiplier);
                    }
                }
            }
        }

        Ok(vec)
    }

    /// Gets the number of open file descriptors for a process
    pub fn fd_count(&self) -> ProcResult<usize> {
        let path = self.root.join("fd");

        Ok(wrap_io_error!(path, path.read_dir())?.count())
    }

    /// Gets a list of open file descriptors for a process
    pub fn fd(&self) -> ProcResult<Vec<FDInfo>> {
        let mut vec = Vec::new();

        let path = self.root.join("fd");

        for dir in wrap_io_error!(path, path.read_dir())? {
            let entry = dir?;
            let file_name = entry.file_name();
            let fd = from_str!(u32, expect!(file_name.to_str()), 10);
            //  note: the link might have disappeared between the time we got the directory listing
            //  and now.  So if the read_link or metadata fails, that's OK
            if let (Ok(link), Ok(md)) = (read_link(entry.path()), entry.metadata()) {
                let link_os: &OsStr = link.as_ref();
                vec.push(FDInfo {
                    fd,
                    mode: (md.st_mode() as libc::mode_t) & libc::S_IRWXU,
                    target: expect!(FDTarget::from_str(expect!(link_os.to_str()))),
                });
            }
        }
        Ok(vec)
    }

    /// Lists which memory segments are written to the core dump in the event that a core dump is performed.
    ///
    /// By default, the following bits are set:
    /// 0, 1, 4 (if the CONFIG_CORE_DUMP_DEFAULT_ELF_HEADERS kernel configuration option is enabled), and 5.
    /// This default can be modified at boot time using the core dump_filter boot option.
    ///
    /// This function will return `Err(ProcError::NotFound)` if the `coredump_filter` file can't be
    /// found.  If it returns `Ok(None)` then the process has no coredump_filter
    pub fn coredump_filter(&self) -> ProcResult<Option<CoredumpFlags>> {
        let mut file = FileWrapper::open(self.root.join("coredump_filter"))?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        if s.trim().is_empty() {
            return Ok(None);
        }
        let flags = from_str!(u32, &s.trim(), 16, pid:self.stat.pid);

        Ok(Some(expect!(CoredumpFlags::from_bits(flags))))
    }

    /// Gets the process's autogroup membership
    ///
    /// (since Linux 2.6.38 and requires CONFIG_SCHED_AUTOGROUP)
    pub fn autogroup(&self) -> ProcResult<String> {
        let mut s = String::new();
        let mut file = FileWrapper::open(self.root.join("autogroup"))?;
        file.read_to_string(&mut s)?;
        Ok(s)
    }

    /// Get the process's auxiliary vector
    ///
    /// (since 2.6.0-test7)
    pub fn auxv(&self) -> ProcResult<HashMap<u32, u32>> {
        use byteorder::{NativeEndian, ReadBytesExt};

        let mut file = FileWrapper::open(self.root.join("auxv"))?;
        let mut map = HashMap::new();

        let mut buf = Vec::new();
        let bytes_read = file.read_to_end(&mut buf)?;
        if bytes_read == 0 {
            // some kernel processes won't have any data for their auxv file
            return Ok(map);
        }
        buf.truncate(bytes_read);
        let mut file = std::io::Cursor::new(buf);

        loop {
            let key = file.read_u32::<NativeEndian>()?;
            let value = file.read_u32::<NativeEndian>()?;
            if key == 0 && value == 0 {
                break;
            }
            map.insert(key, value);
        }

        Ok(map)
    }

    /// Gets the symbolic name corresponding to the location in the kernel where the process is sleeping.
    ///
    /// (since Linux 2.6.0)
    pub fn wchan(&self) -> ProcResult<String> {
        let mut s = String::new();
        let mut file = FileWrapper::open(self.root.join("wchan"))?;
        file.read_to_string(&mut s)?;
        Ok(s)
    }

    /// Return the `Status` for this process, based on the `/proc/[pid]/status` file.
    pub fn status(&self) -> ProcResult<Status> {
        let path = self.root.join("status");
        let file = FileWrapper::open(&path)?;
        Status::from_reader(file)
    }

    /// Returns the status info from `/proc/[pid]/stat`.
    ///
    /// Note that this data comes pre-loaded in the `stat` field.  This method is useful when you
    /// get the latest status data (since some of it changes while the program is running)
    pub fn stat(&self) -> ProcResult<Stat> {
        let path = self.root.join("stat");
        let stat = Stat::from_reader(FileWrapper::open(&path)?)?;
        Ok(stat)
    }

    /// Gets the process' login uid. May not be available.
    pub fn loginuid(&self) -> ProcResult<u32> {
        let mut uid = String::new();
        let path = self.root.join("loginuid");
        let mut file = FileWrapper::open(&path)?;
        file.read_to_string(&mut uid)?;
        Status::parse_uid_gid(&uid, 0)
    }

    /// The current score that the kernel gives to this process for the purpose of selecting a
    /// process for the OOM-killer
    ///
    /// A higher score means that the process is more likely to be selected by the OOM-killer.
    /// The basis for this score is the amount of memory used by the process, plus other factors.
    ///
    /// (Since linux 2.6.11)
    pub fn oom_score(&self) -> ProcResult<u32> {
        let path = self.root.join("oom_score");
        let mut file = FileWrapper::open(&path)?;
        let mut oom = String::new();
        file.read_to_string(&mut oom)?;
        Ok(from_str!(u32, oom.trim()))
    }

    /// Set process memory information
    ///
    /// Much of this data is the same as the data from `stat()` and `status()`
    pub fn statm(&self) -> ProcResult<StatM> {
        let path = self.root.join("statm");
        let file = FileWrapper::open(&path)?;
        StatM::from_reader(file)
    }

    /// Return a task for the main thread of this process
    pub fn task_main_thread(&self) -> ProcResult<Task> {
        Task::new(self.pid, self.pid)
    }

    /// Return the `Schedstat` for this process, based on the `/proc/<pid>/schedstat` file.
    ///
    /// (Requires CONFIG_SCHED_INFO)
    pub fn schedstat(&self) -> ProcResult<Schedstat> {
        let path = self.root.join("schedstat");
        let file = FileWrapper::open(&path)?;
        Schedstat::from_reader(file)
    }

    /// Iterate over all the [`Task`]s (aka Threads) in this process
    ///
    /// Note that the iterator does not receive a snapshot of tasks, it is a
    /// lazy iterator over whatever happens to be running when the iterator
    /// gets there, see the examples below.
    ///
    /// # Examples
    ///
    /// ## Simple iteration over subtasks
    ///
    /// If you want to get the info that most closely matches what was running
    /// when you call `tasks` you should collect them as quikcly as possible,
    /// and then run processing over that collection:
    ///
    /// ```
    /// # use std::thread;
    /// # use std::sync::mpsc::channel;
    /// # use procfs::process::Process;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let (finish_tx, finish_rx) = channel();
    /// # let (start_tx, start_rx) = channel();
    /// let name = "testing:example";
    /// let t = thread::Builder::new().name(name.to_string())
    ///   .spawn(move || { // do work
    /// #     start_tx.send(()).unwrap();
    /// #     finish_rx.recv().expect("valid channel");
    ///   })?;
    /// # start_rx.recv()?;
    ///
    /// let proc = Process::myself()?;
    ///
    /// // Collect a snapshot
    /// let threads: Vec<_> = proc.tasks()?.flatten().map(|t| t.stat().unwrap().comm).collect();
    /// threads.iter().find(|s| &**s == name).expect("thread should exist");
    ///
    /// # finish_tx.send(());
    /// # t.join().unwrap();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## The TaskIterator is lazy
    ///
    /// This means both that tasks that stop before you get to them in
    /// iteration will not be there, and that new tasks that are created after
    /// you start the iterator *will* appear.
    ///
    /// ```
    /// # use std::thread;
    /// # use std::sync::mpsc::channel;
    /// # use procfs::process::Process;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let proc = Process::myself()?;
    ///
    /// // Task iteration is lazy
    /// let mut task_iter = proc.tasks()?.flatten().map(|t| t.stat().unwrap().comm);
    ///
    /// # let (finish_tx, finish_rx) = channel();
    /// # let (start_tx, start_rx) = channel();
    /// let name = "testing:lazy";
    /// let t = thread::Builder::new().name(name.to_string())
    ///   .spawn(move || { // do work
    /// #     start_tx.send(()).unwrap();
    /// #     finish_rx.recv().expect("valid channel");
    ///   })?;
    /// # start_rx.recv()?;
    ///
    /// task_iter.find(|s| &**s == name).expect("thread should exist");
    ///
    /// # finish_tx.send(());
    /// # t.join().unwrap();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Tasks that stop while you're iterating may or may not appear:
    ///
    /// ```
    /// # use std::thread;
    /// # use std::sync::mpsc::channel;
    /// # use procfs::process::Process;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let (finish_tx, finish_rx) = channel();
    /// # let (start_tx, start_rx) = channel();
    /// let name = "testing:stopped";
    /// let t = thread::Builder::new().name(name.to_string())
    ///   .spawn(move || { // do work
    /// #     start_tx.send(()).unwrap();
    /// #     finish_rx.recv().expect("valid channel");
    ///   })?;
    /// # start_rx.recv()?;
    ///
    /// let proc = Process::myself()?;
    ///
    /// // Task iteration is lazy
    /// let mut task_iter = proc.tasks()?.flatten().map(|t| t.stat().unwrap().comm);
    ///
    /// # finish_tx.send(());
    /// t.join().unwrap();
    ///
    /// // It's impossible to know if this is going to be gone
    /// let _ = task_iter.find(|s| &**s == name).is_some();
    /// # Ok(())
    /// # }
    /// ```
    pub fn tasks(&self) -> ProcResult<TasksIter> {
        Ok(TasksIter {
            pid: self.pid,
            inner: fs::read_dir(self.root.join("task"))?,
        })
    }
}

/// The result of [`Process::tasks`], iterates over all tasks in a process
#[derive(Debug)]
pub struct TasksIter {
    pid: pid_t,
    inner: fs::ReadDir,
}

impl std::iter::Iterator for TasksIter {
    type Item = ProcResult<Task>;
    fn next(&mut self) -> Option<ProcResult<Task>> {
        match self.inner.next() {
            Some(Ok(tp)) => Some(Task::from_rel_path(self.pid, &tp.path())),
            Some(Err(e)) => Some(Err(ProcError::Io(e, None))),
            None => None,
        }
    }
}

/// Return a list of all processes
///
/// If a process can't be constructed for some reason, it won't be returned in the list.
pub fn all_processes() -> ProcResult<Vec<Process>> {
    all_processes_with_root("/proc")
}

/// Return a list of all processes based on a specified `/proc` path
///
/// If a process can't be constructed for some reason, it won't be returned in the list.
pub fn all_processes_with_root(root: impl AsRef<Path>) -> ProcResult<Vec<Process>> {
    let mut v = Vec::new();
    let root = root.as_ref();
    for entry in expect!(std::fs::read_dir(root), format!("No {} directory", root.display())).flatten() {
        if i32::from_str(&entry.file_name().to_string_lossy()).is_ok() {
            match Process::new_with_root(entry.path()) {
                Ok(prc) => v.push(prc),
                Err(ProcError::InternalError(e)) => return Err(ProcError::InternalError(e)),
                _ => {}
            }
        }
    }

    Ok(v)
}

/// Provides information about memory usage, measured in pages.
#[derive(Debug, Clone, Copy)]
pub struct StatM {
    /// Total program size, measured in pages
    ///
    /// (same as VmSize in /proc/<pid>/status)
    pub size: u64,
    /// Resident set size, measured in pages
    ///
    /// (same as VmRSS in /proc/<pid>/status)
    pub resident: u64,
    /// number of resident shared pages (i.e., backed by a file)
    ///
    /// (same as RssFile+RssShmem in /proc/<pid>/status)
    pub shared: u64,
    /// Text (code)
    pub text: u64,
    /// library (unused since Linux 2.6; always 0)
    pub lib: u64,
    /// data + stack
    pub data: u64,
    /// dirty pages (unused since Linux 2.6; always 0)
    pub dt: u64,
}

impl StatM {
    fn from_reader<R: io::Read>(mut r: R) -> ProcResult<StatM> {
        let mut line = String::new();
        r.read_to_string(&mut line)?;
        let mut s = line.split_whitespace();

        let size = expect!(from_iter(&mut s));
        let resident = expect!(from_iter(&mut s));
        let shared = expect!(from_iter(&mut s));
        let text = expect!(from_iter(&mut s));
        let lib = expect!(from_iter(&mut s));
        let data = expect!(from_iter(&mut s));
        let dt = expect!(from_iter(&mut s));

        if cfg!(test) {
            assert!(s.next().is_none());
        }

        Ok(StatM {
            size,
            resident,
            shared,
            text,
            lib,
            data,
            dt,
        })
    }
}

#[cfg(test)]
mod tests;
