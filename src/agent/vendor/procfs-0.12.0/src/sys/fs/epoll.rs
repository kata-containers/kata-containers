use crate::{read_value, write_value, ProcResult};

/// Get the limit on the total number of file descriptors that a user can register across all epoll instances.
///
/// The limit is per real user ID.  Each registered file descriptor costs roughtly 90 bytes on a 32-bit kernel,
/// and roughly 160 bytes on a 64-bit kernel.  Currently, the default value for `max_user_watches` is 1/25 (4%)
/// of the available low memory, divided by the registration cost in bytes.
///
/// (Since Linux 2.6.28)
pub fn max_user_watches() -> ProcResult<u64> {
    read_value("/proc/sys/fs/epoll/max_user_watches")
}

/// Sets the limit on the total number of file descriptors that a user can register across all epoll instances.
pub fn set_max_user_watches(val: u64) -> ProcResult<()> {
    write_value("/proc/sys/fs/epoll/max_user_watches", val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KernelVersion;

    #[test]
    fn test_max_user_watches() {
        if KernelVersion::current().unwrap() >= KernelVersion::new(2, 6, 28) {
            println!("{}", max_user_watches().unwrap());
        }
    }
}
