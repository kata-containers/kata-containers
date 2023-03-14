//! This modules contains functions for kernel variables related to filesystems

use crate::{read_file, read_value, write_value, ProcResult};
use std::time::Duration;

pub mod binfmt_misc;
pub mod epoll;

/// Information about the status of the directory cache (dcache)
#[derive(Debug, Clone)]
pub struct DEntryState {
    /// The number of allocated dentries (dcache entries)
    ///
    /// Unused in Linux 2.2
    pub nr_dentry: u32,

    /// The number of unused dentries.
    pub nr_unused: u32,

    /// The age after which dcache entries can be reclaimied when memory is short
    pub age_limit: Duration,

    /// Is true when the kernel has called `shrink_dcache_pages()` and the dcache isn't pruned yet.
    pub want_pages: bool,
}

impl DEntryState {
    fn from_str(s: &str) -> ProcResult<DEntryState> {
        let mut s = s.split_whitespace();
        let nr_dentry = from_str!(u32, expect!(s.next()));
        let nr_unused = from_str!(u32, expect!(s.next()));
        let age_limit_sec = from_str!(u32, expect!(s.next()));
        let want_pages = from_str!(u32, expect!(s.next()));

        Ok(DEntryState {
            nr_dentry,
            nr_unused,
            age_limit: Duration::from_secs(age_limit_sec as u64),
            want_pages: want_pages != 0,
        })
    }
}

/// Get information about the status of the directory cache (dcache)
///
/// Linux Linux 2.2
pub fn dentry_state() -> ProcResult<DEntryState> {
    let s: String = read_file("/proc/sys/fs/dentry-state")?;

    DEntryState::from_str(&s)
}

/// Get the system-wide limit on the number of open files for all processes.
///
/// System calls that fail when encoun‐ tering this limit fail with the error `ENFILE`.
pub fn file_max() -> ProcResult<usize> {
    read_value("/proc/sys/fs/file-max")
}

/// Set the system-wide limit on the number of open files for all processes.
pub fn set_file_max(max: usize) -> ProcResult<()> {
    write_value("/proc/sys/fs/file-max", max)
}
#[derive(Debug, Clone)]
pub struct FileState {
    /// The number of allocated file handles.
    ///
    /// (i.e. the number of files presently opened)
    pub allocated: u64,

    /// The number of free file handles.
    pub free: u64,

    /// The maximum number of file handles.
    ///
    /// This may be u64::MAX
    pub max: u64,
}

pub fn file_nr() -> ProcResult<FileState> {
    let s = read_file("/proc/sys/fs/file-nr")?;
    let mut s = s.split_whitespace();
    let allocated = from_str!(u64, expect!(s.next()));
    let free = from_str!(u64, expect!(s.next()));
    let max = from_str!(u64, expect!(s.next()));

    Ok(FileState { allocated, free, max })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dentry() {
        let d = dentry_state().unwrap();
        println!("{:?}", d);
    }

    #[test]
    fn filenr() {
        let f = file_nr().unwrap();
        println!("{:?}", f);
    }
}
