//! Functions for switching the running process’s user or group.

use std::io;
use libc::{uid_t, gid_t, c_int};

use base::{get_effective_uid, get_effective_gid};


// NOTE: for whatever reason, it seems these are not available in libc on BSD platforms, so they
//       need to be included manually
extern {
    fn setreuid(ruid: uid_t, euid: uid_t) -> c_int;
    fn setregid(rgid: gid_t, egid: gid_t) -> c_int;
}


/// Sets the **current user** for the running process to the one with the
/// given user ID.
///
/// Typically, trying to switch to anyone other than the user already running
/// the process requires root privileges.
///
/// # libc functions used
///
/// - [`setuid`](https://docs.rs/libc/*/libc/fn.setuid.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during the
/// `setuid` call.
///
/// # Examples
///
/// ```no_run
/// use users::switch::set_current_uid;
///
/// set_current_uid(1001);
/// // current user ID is 1001
/// ```
pub fn set_current_uid(uid: uid_t) -> io::Result<()> {
    match unsafe { libc::setuid(uid) } {
         0 => Ok(()),
        -1 => Err(io::Error::last_os_error()),
         n => unreachable!("setuid returned {}", n)
    }
}

/// Sets the **current group** for the running process to the one with the
/// given group ID.
///
/// Typically, trying to switch to any group other than the group already
/// running the process requires root privileges.
///
/// # libc functions used
///
/// - [`setgid`](https://docs.rs/libc/*/libc/fn.setgid.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during the
/// `setgid` call.
///
/// # Examples
///
/// ```no_run
/// use users::switch::set_current_gid;
///
/// set_current_gid(1001);
/// // current group ID is 1001
/// ```
pub fn set_current_gid(gid: gid_t) -> io::Result<()> {
    match unsafe { libc::setgid(gid) } {
         0 => Ok(()),
        -1 => Err(io::Error::last_os_error()),
         n => unreachable!("setgid returned {}", n)
    }
}

/// Sets the **effective user** for the running process to the one with the
/// given user ID.
///
/// Typically, trying to switch to anyone other than the user already running
/// the process requires root privileges.
///
/// # libc functions used
///
/// - [`seteuid`](https://docs.rs/libc/*/libc/fn.seteuid.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during the
/// `seteuid` call.
///
/// # Examples
///
/// ```no_run
/// use users::switch::set_effective_uid;
///
/// set_effective_uid(1001);
/// // current effective user ID is 1001
/// ```
pub fn set_effective_uid(uid: uid_t) -> io::Result<()> {
    match unsafe { libc::seteuid(uid) } {
         0 => Ok(()),
        -1 => Err(io::Error::last_os_error()),
         n => unreachable!("seteuid returned {}", n)
    }
}

/// Sets the **effective group** for the running process to the one with the
/// given group ID.
///
/// Typically, trying to switch to any group other than the group already
/// running the process requires root privileges.
///
/// # libc functions used
///
/// - [`setegid`](https://docs.rs/libc/*/libc/fn.setegid.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during the
/// `setegid` call.
///
/// # Examples
///
/// ```no_run
/// use users::switch::set_effective_gid;
///
/// set_effective_gid(1001);
/// // current effective group ID is 1001
/// ```
pub fn set_effective_gid(gid: gid_t) -> io::Result<()> {
    match unsafe { libc::setegid(gid) } {
         0 => Ok(()),
        -1 => Err(io::Error::last_os_error()),
         n => unreachable!("setegid returned {}", n)
    }
}

/// Sets both the **current user** and the **effective user** for the running
/// process to the ones with the given user IDs.
///
/// Typically, trying to switch to anyone other than the user already running
/// the process requires root privileges.
///
/// # libc functions used
///
/// - [`setreuid`](https://docs.rs/libc/*/libc/fn.setreuid.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during the
/// `setreuid` call.
///
/// # Examples
///
/// ```no_run
/// use users::switch::set_both_uid;
///
/// set_both_uid(1001, 1001);
/// // current user ID and effective user ID are 1001
/// ```
pub fn set_both_uid(ruid: uid_t, euid: uid_t) -> io::Result<()> {
    match unsafe { setreuid(ruid, euid) } {
         0 => Ok(()),
        -1 => Err(io::Error::last_os_error()),
         n => unreachable!("setreuid returned {}", n)
    }
}

/// Sets both the **current group** and the **effective group** for the
/// running process to the ones with the given group IDs.
///
/// Typically, trying to switch to any group other than the group already
/// running the process requires root privileges.
///
/// # libc functions used
///
/// - [`setregid`](https://docs.rs/libc/*/libc/fn.setregid.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during the
/// `setregid` call.
///
/// # Examples
///
/// ```no_run
/// use users::switch::set_both_gid;
///
/// set_both_gid(1001, 1001);
/// // current user ID and effective group ID are 1001
/// ```
pub fn set_both_gid(rgid: gid_t, egid: gid_t) -> io::Result<()> {
    match unsafe { setregid(rgid, egid) } {
         0 => Ok(()),
        -1 => Err(io::Error::last_os_error()),
         n => unreachable!("setregid returned {}", n)
    }
}

/// Guard returned from a `switch_user_group` call.
pub struct SwitchUserGuard {
    uid: uid_t,
    gid: gid_t,
}

impl Drop for SwitchUserGuard {
    fn drop(&mut self) {
        set_effective_gid(self.gid).expect("Failed to set effective gid");
        set_effective_uid(self.uid).expect("Failed to set effective uid");
    }
}

/// Sets the **effective user** and the **effective group** for the current
/// scope.
///
/// Typically, trying to switch to any user or group other than the ones already
/// running the process requires root privileges.
///
/// # Security considerations
///
/// - Because Rust does not guarantee running the destructor, it’s a good idea
///   to call [`std::mem::drop`](https://doc.rust-lang.org/std/mem/fn.drop.html)
///   on the guard manually in security-sensitive situations.
/// - This function switches the group before the user to prevent the user’s
///   privileges being dropped before trying to change the group (look up
///   `POS36-C`).
/// - This function will panic upon failing to set either walue, so the
///   program does not continue executing with too many privileges.
///
/// # libc functions used
///
/// - [`seteuid`](https://docs.rs/libc/*/libc/fn.seteuid.html)
/// - [`setegid`](https://docs.rs/libc/*/libc/fn.setegid.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during either
/// the `seteuid` or `setegid` calls.
///
/// # Examples
///
/// ```no_run
/// use users::switch::switch_user_group;
/// use std::mem::drop;
///
/// {
///     let guard = switch_user_group(1001, 1001);
///     // current and effective user and group IDs are 1001
///     drop(guard);
/// }
/// // back to the old values
/// ```
pub fn switch_user_group(uid: uid_t, gid: gid_t) -> io::Result<SwitchUserGuard> {
    let current_state = SwitchUserGuard {
        gid: get_effective_gid(),
        uid: get_effective_uid(),
    };

    set_effective_gid(gid)?;
    set_effective_uid(uid)?;
    Ok(current_state)
}
