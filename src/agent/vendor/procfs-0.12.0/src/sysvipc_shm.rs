use std::io;

use libc::pid_t;

use super::{FileWrapper, ProcResult};
use std::str::FromStr;

/// A shared memory segment parsed from `/proc/sysvipc/shm`
/// Relation with `[crate::process::process::MMapPath::Vsys]`
#[derive(Debug, Clone)]
#[allow(non_snake_case)]
pub struct Shm {
    /// Segment key
    pub key: i32,
    /// Segment ID, unique
    pub shmid: u64,
    /// Access permissions, as octal
    pub perms: u16,
    /// Size in bytes
    pub size: u32,
    /// Creator PID
    pub cpid: pid_t,
    /// Last operator PID
    pub lpid: pid_t,
    /// Number of attached processes
    pub nattch: u32,
    /// User ID
    pub uid: u16,
    /// Group ID
    pub gid: u16,
    /// Creator UID
    pub cuid: u16,
    /// Creator GID
    pub cgid: u16,
    /// Time of last `shmat` (attach), epoch
    pub atime: u64,
    /// Time of last `shmdt` (detach), epoch
    pub dtime: u64,
    /// Time of last permission change, epoch
    pub ctime: u64,
    /// Current part of the shared memory resident in memory
    pub rss: u64,
    /// Current part of the shared memory in SWAP
    pub swap: u64,
}

impl Shm {
    /// Reads and parses the `/proc/sysvipc/shm`, returning an error if there are problems.
    pub fn new() -> ProcResult<Vec<Shm>> {
        let f = FileWrapper::open("/proc/sysvipc/shm")?;

        Shm::from_reader(f)
    }

    /// Get Meminfo from a custom Read instead of the default `/proc/sysvipc/shm`.
    pub fn from_reader<R: io::Read>(r: R) -> ProcResult<Vec<Shm>> {
        use std::io::{BufRead, BufReader};

        let reader = BufReader::new(r);
        let mut vec = Vec::new();

        // See printing code here:
        // https://elixir.bootlin.com/linux/latest/source/ipc/shm.c#L1737
        for line in reader.lines().skip(1) {
            let line = expect!(line);
            let mut s = line.split_whitespace();

            let key = expect!(i32::from_str(expect!(s.next())));
            let shmid = expect!(u64::from_str(expect!(s.next())));
            let perms = expect!(u16::from_str(expect!(s.next())));
            let size = expect!(u32::from_str(expect!(s.next())));
            let cpid = expect!(pid_t::from_str(expect!(s.next())));
            let lpid = expect!(pid_t::from_str(expect!(s.next())));
            let nattch = expect!(u32::from_str(expect!(s.next())));
            let uid = expect!(u16::from_str(expect!(s.next())));
            let gid = expect!(u16::from_str(expect!(s.next())));
            let cuid = expect!(u16::from_str(expect!(s.next())));
            let cgid = expect!(u16::from_str(expect!(s.next())));
            let atime = expect!(u64::from_str(expect!(s.next())));
            let dtime = expect!(u64::from_str(expect!(s.next())));
            let ctime = expect!(u64::from_str(expect!(s.next())));
            let rss = expect!(u64::from_str(expect!(s.next())));
            let swap = expect!(u64::from_str(expect!(s.next())));

            let shm = Shm {
                key,
                shmid,
                perms,
                size,
                cpid,
                lpid,
                nattch,
                uid,
                gid,
                cuid,
                cgid,
                atime,
                dtime,
                ctime,
                rss,
                swap,
            };

            vec.push(shm);
        }

        Ok(vec)
    }
}
