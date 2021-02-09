//! Concerning the current working directory of a process, from
//! `/proc/[pid]/cwd`.

use std::fs;
use std::io::Result;
use std::path::PathBuf;

use libc::pid_t;

/// Gets path of current working directory for the process with the provided
/// pid.
pub fn cwd(pid: pid_t) -> Result<PathBuf> {
    fs::read_link(format!("/proc/{}/cwd", pid))
}

/// Gets path of current working directory for the current process.
pub fn cwd_self() -> Result<PathBuf> {
    fs::read_link("/proc/self/cwd")
}

#[cfg(test)]
pub mod tests {
    use super::cwd_self;
    use std::env;

    #[test]
    fn test_cwd_self() {
        assert_eq!(env::current_dir().unwrap(), cwd_self().unwrap());
    }
}
