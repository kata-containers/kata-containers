/*!
A pure-Rust library to work with Linux capabilities.

It provides support for manipulating capabilities available on modern Linux
kernels. It supports traditional POSIX sets (Effective, Inheritable, Permitted)
as well as Linux-specific Ambient and Bounding capabilities sets.

```rust
type ExResult<T> = Result<T, Box<dyn std::error::Error + 'static>>;

fn manipulate_caps() -> ExResult<()> {
    use caps::{Capability, CapSet};

    if caps::has_cap(None, CapSet::Permitted, Capability::CAP_SYS_NICE)? {
        caps::drop(None, CapSet::Effective, Capability::CAP_SYS_NICE)?;
        let effective = caps::read(None, CapSet::Effective)?;
        assert_eq!(effective.contains(&Capability::CAP_SYS_NICE), false);

        caps::clear(None, CapSet::Effective)?;
        let cleared = caps::read(None, CapSet::Effective)?;
        assert_eq!(cleared.is_empty(), true);
    };

    Ok(())
}
```
!*/

pub mod errors;
pub mod runtime;
pub mod securebits;

// Implementation of Bounding set.
mod ambient;
// Implementation of POSIX sets.
mod base;
// Implementation of Bounding set.
mod bounding;
// All kernel-related constants.
mod nr;

use crate::errors::CapsError;
use std::iter::FromIterator;

/// Linux capabilities sets.
///
/// All capabilities sets supported by Linux, including standard
/// POSIX and custom ones. See `capabilities(7)`.
#[derive(Debug, Clone, Copy)]
pub enum CapSet {
    /// Ambient capabilities set (from Linux 4.3).
    Ambient,
    /// Bounding capabilities set (from Linux 2.6.25)
    Bounding,
    /// Effective capabilities set (from POSIX)
    Effective,
    /// Inheritable capabilities set (from POSIX)
    Inheritable,
    /// Permitted capabilities set (from POSIX)
    Permitted,
}

/// Linux capabilities.
///
/// All capabilities supported by Linux, including standard
/// POSIX and custom ones. See `capabilities(7)`.
#[allow(clippy::manual_non_exhaustive)]
#[allow(non_camel_case_types)]
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
#[repr(u8)]
#[cfg_attr(
    feature = "serde_support",
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum Capability {
    /// `CAP_CHOWN` (from POSIX)
    CAP_CHOWN = nr::CAP_CHOWN,
    /// `CAP_DAC_OVERRIDE` (from POSIX)
    CAP_DAC_OVERRIDE = nr::CAP_DAC_OVERRIDE,
    /// `CAP_DAC_READ_SEARCH` (from POSIX)
    CAP_DAC_READ_SEARCH = nr::CAP_DAC_READ_SEARCH,
    /// `CAP_FOWNER` (from POSIX)
    CAP_FOWNER = nr::CAP_FOWNER,
    /// `CAP_FSETID` (from POSIX)
    CAP_FSETID = nr::CAP_FSETID,
    /// `CAP_KILL` (from POSIX)
    CAP_KILL = nr::CAP_KILL,
    /// `CAP_SETGID` (from POSIX)
    CAP_SETGID = nr::CAP_SETGID,
    /// `CAP_SETUID` (from POSIX)
    CAP_SETUID = nr::CAP_SETUID,
    /// `CAP_SETPCAP` (from Linux)
    CAP_SETPCAP = nr::CAP_SETPCAP,
    CAP_LINUX_IMMUTABLE = nr::CAP_LINUX_IMMUTABLE,
    CAP_NET_BIND_SERVICE = nr::CAP_NET_BIND_SERVICE,
    CAP_NET_BROADCAST = nr::CAP_NET_BROADCAST,
    CAP_NET_ADMIN = nr::CAP_NET_ADMIN,
    CAP_NET_RAW = nr::CAP_NET_RAW,
    CAP_IPC_LOCK = nr::CAP_IPC_LOCK,
    CAP_IPC_OWNER = nr::CAP_IPC_OWNER,
    /// `CAP_SYS_MODULE` (from Linux)
    CAP_SYS_MODULE = nr::CAP_SYS_MODULE,
    /// `CAP_SYS_RAWIO` (from Linux)
    CAP_SYS_RAWIO = nr::CAP_SYS_RAWIO,
    /// `CAP_SYS_CHROOT` (from Linux)
    CAP_SYS_CHROOT = nr::CAP_SYS_CHROOT,
    /// `CAP_SYS_PTRACE` (from Linux)
    CAP_SYS_PTRACE = nr::CAP_SYS_PTRACE,
    /// `CAP_SYS_PACCT` (from Linux)
    CAP_SYS_PACCT = nr::CAP_SYS_PACCT,
    /// `CAP_SYS_ADMIN` (from Linux)
    CAP_SYS_ADMIN = nr::CAP_SYS_ADMIN,
    /// `CAP_SYS_BOOT` (from Linux)
    CAP_SYS_BOOT = nr::CAP_SYS_BOOT,
    /// `CAP_SYS_NICE` (from Linux)
    CAP_SYS_NICE = nr::CAP_SYS_NICE,
    /// `CAP_SYS_RESOURCE` (from Linux)
    CAP_SYS_RESOURCE = nr::CAP_SYS_RESOURCE,
    /// `CAP_SYS_TIME` (from Linux)
    CAP_SYS_TIME = nr::CAP_SYS_TIME,
    /// `CAP_SYS_TTY_CONFIG` (from Linux)
    CAP_SYS_TTY_CONFIG = nr::CAP_SYS_TTY_CONFIG,
    /// `CAP_SYS_MKNOD` (from Linux, >= 2.4)
    CAP_MKNOD = nr::CAP_MKNOD,
    /// `CAP_LEASE` (from Linux, >= 2.4)
    CAP_LEASE = nr::CAP_LEASE,
    CAP_AUDIT_WRITE = nr::CAP_AUDIT_WRITE,
    /// `CAP_AUDIT_CONTROL` (from Linux, >= 2.6.11)
    CAP_AUDIT_CONTROL = nr::CAP_AUDIT_CONTROL,
    CAP_SETFCAP = nr::CAP_SETFCAP,
    CAP_MAC_OVERRIDE = nr::CAP_MAC_OVERRIDE,
    CAP_MAC_ADMIN = nr::CAP_MAC_ADMIN,
    /// `CAP_SYSLOG` (from Linux, >= 2.6.37)
    CAP_SYSLOG = nr::CAP_SYSLOG,
    /// `CAP_WAKE_ALARM` (from Linux, >= 3.0)
    CAP_WAKE_ALARM = nr::CAP_WAKE_ALARM,
    CAP_BLOCK_SUSPEND = nr::CAP_BLOCK_SUSPEND,
    /// `CAP_AUDIT_READ` (from Linux, >= 3.16).
    CAP_AUDIT_READ = nr::CAP_AUDIT_READ,
    /// `CAP_PERFMON` (from Linux, >= 5.8).
    CAP_PERFMON = nr::CAP_PERFMON,
    /// `CAP_BPF` (from Linux, >= 5.8).
    CAP_BPF = nr::CAP_BPF,
    /// `CAP_CHECKPOINT_RESTORE` (from Linux, >= 5.9).
    CAP_CHECKPOINT_RESTORE = nr::CAP_CHECKPOINT_RESTORE,
    #[doc(hidden)]
    __Nonexhaustive,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let name = match *self {
            Capability::CAP_CHOWN => "CAP_CHOWN",
            Capability::CAP_DAC_OVERRIDE => "CAP_DAC_OVERRIDE",
            Capability::CAP_DAC_READ_SEARCH => "CAP_DAC_READ_SEARCH",
            Capability::CAP_FOWNER => "CAP_FOWNER",
            Capability::CAP_FSETID => "CAP_FSETID",
            Capability::CAP_KILL => "CAP_KILL",
            Capability::CAP_SETGID => "CAP_SETGID",
            Capability::CAP_SETUID => "CAP_SETUID",
            Capability::CAP_SETPCAP => "CAP_SETPCAP",
            Capability::CAP_LINUX_IMMUTABLE => "CAP_LINUX_IMMUTABLE",
            Capability::CAP_NET_BIND_SERVICE => "CAP_NET_BIND_SERVICE",
            Capability::CAP_NET_BROADCAST => "CAP_NET_BROADCAST",
            Capability::CAP_NET_ADMIN => "CAP_NET_ADMIN",
            Capability::CAP_NET_RAW => "CAP_NET_RAW",
            Capability::CAP_IPC_LOCK => "CAP_IPC_LOCK",
            Capability::CAP_IPC_OWNER => "CAP_IPC_OWNER",
            Capability::CAP_SYS_MODULE => "CAP_SYS_MODULE",
            Capability::CAP_SYS_RAWIO => "CAP_SYS_RAWIO",
            Capability::CAP_SYS_CHROOT => "CAP_SYS_CHROOT",
            Capability::CAP_SYS_PTRACE => "CAP_SYS_PTRACE",
            Capability::CAP_SYS_PACCT => "CAP_SYS_PACCT",
            Capability::CAP_SYS_ADMIN => "CAP_SYS_ADMIN",
            Capability::CAP_SYS_BOOT => "CAP_SYS_BOOT",
            Capability::CAP_SYS_NICE => "CAP_SYS_NICE",
            Capability::CAP_SYS_RESOURCE => "CAP_SYS_RESOURCE",
            Capability::CAP_SYS_TIME => "CAP_SYS_TIME",
            Capability::CAP_SYS_TTY_CONFIG => "CAP_SYS_TTY_CONFIG",
            Capability::CAP_MKNOD => "CAP_MKNOD",
            Capability::CAP_LEASE => "CAP_LEASE",
            Capability::CAP_AUDIT_WRITE => "CAP_AUDIT_WRITE",
            Capability::CAP_AUDIT_CONTROL => "CAP_AUDIT_CONTROL",
            Capability::CAP_SETFCAP => "CAP_SETFCAP",
            Capability::CAP_MAC_OVERRIDE => "CAP_MAC_OVERRIDE",
            Capability::CAP_MAC_ADMIN => "CAP_MAC_ADMIN",
            Capability::CAP_SYSLOG => "CAP_SYSLOG",
            Capability::CAP_WAKE_ALARM => "CAP_WAKE_ALARM",
            Capability::CAP_BLOCK_SUSPEND => "CAP_BLOCK_SUSPEND",
            Capability::CAP_AUDIT_READ => "CAP_AUDIT_READ",
            Capability::CAP_PERFMON => "CAP_PERFMON",
            Capability::CAP_BPF => "CAP_BPF",
            Capability::CAP_CHECKPOINT_RESTORE => "CAP_CHECKPOINT_RESTORE",
            Capability::__Nonexhaustive => unreachable!("invalid capability"),
        };
        write!(f, "{}", name)
    }
}

impl std::str::FromStr for Capability {
    type Err = CapsError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "CAP_CHOWN" => Ok(Capability::CAP_CHOWN),
            "CAP_DAC_OVERRIDE" => Ok(Capability::CAP_DAC_OVERRIDE),
            "CAP_DAC_READ_SEARCH" => Ok(Capability::CAP_DAC_READ_SEARCH),
            "CAP_FOWNER" => Ok(Capability::CAP_FOWNER),
            "CAP_FSETID" => Ok(Capability::CAP_FSETID),
            "CAP_KILL" => Ok(Capability::CAP_KILL),
            "CAP_SETGID" => Ok(Capability::CAP_SETGID),
            "CAP_SETUID" => Ok(Capability::CAP_SETUID),
            "CAP_SETPCAP" => Ok(Capability::CAP_SETPCAP),
            "CAP_LINUX_IMMUTABLE" => Ok(Capability::CAP_LINUX_IMMUTABLE),
            "CAP_NET_BIND_SERVICE" => Ok(Capability::CAP_NET_BIND_SERVICE),
            "CAP_NET_BROADCAST" => Ok(Capability::CAP_NET_BROADCAST),
            "CAP_NET_ADMIN" => Ok(Capability::CAP_NET_ADMIN),
            "CAP_NET_RAW" => Ok(Capability::CAP_NET_RAW),
            "CAP_IPC_LOCK" => Ok(Capability::CAP_IPC_LOCK),
            "CAP_IPC_OWNER" => Ok(Capability::CAP_IPC_OWNER),
            "CAP_SYS_MODULE" => Ok(Capability::CAP_SYS_MODULE),
            "CAP_SYS_RAWIO" => Ok(Capability::CAP_SYS_RAWIO),
            "CAP_SYS_CHROOT" => Ok(Capability::CAP_SYS_CHROOT),
            "CAP_SYS_PTRACE" => Ok(Capability::CAP_SYS_PTRACE),
            "CAP_SYS_PACCT" => Ok(Capability::CAP_SYS_PACCT),
            "CAP_SYS_ADMIN" => Ok(Capability::CAP_SYS_ADMIN),
            "CAP_SYS_BOOT" => Ok(Capability::CAP_SYS_BOOT),
            "CAP_SYS_NICE" => Ok(Capability::CAP_SYS_NICE),
            "CAP_SYS_RESOURCE" => Ok(Capability::CAP_SYS_RESOURCE),
            "CAP_SYS_TIME" => Ok(Capability::CAP_SYS_TIME),
            "CAP_SYS_TTY_CONFIG" => Ok(Capability::CAP_SYS_TTY_CONFIG),
            "CAP_MKNOD" => Ok(Capability::CAP_MKNOD),
            "CAP_LEASE" => Ok(Capability::CAP_LEASE),
            "CAP_AUDIT_WRITE" => Ok(Capability::CAP_AUDIT_WRITE),
            "CAP_AUDIT_CONTROL" => Ok(Capability::CAP_AUDIT_CONTROL),
            "CAP_SETFCAP" => Ok(Capability::CAP_SETFCAP),
            "CAP_MAC_OVERRIDE" => Ok(Capability::CAP_MAC_OVERRIDE),
            "CAP_MAC_ADMIN" => Ok(Capability::CAP_MAC_ADMIN),
            "CAP_SYSLOG" => Ok(Capability::CAP_SYSLOG),
            "CAP_WAKE_ALARM" => Ok(Capability::CAP_WAKE_ALARM),
            "CAP_BLOCK_SUSPEND" => Ok(Capability::CAP_BLOCK_SUSPEND),
            "CAP_AUDIT_READ" => Ok(Capability::CAP_AUDIT_READ),
            "CAP_PERFMON" => Ok(Capability::CAP_PERFMON),
            "CAP_BPF" => Ok(Capability::CAP_BPF),
            "CAP_CHECKPOINT_RESTORE" => Ok(Capability::CAP_CHECKPOINT_RESTORE),
            _ => Err(format!("invalid capability: {}", s).into()),
        }
    }
}

impl Capability {
    /// Returns the bitmask corresponding to this capability value.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn bitmask(&self) -> u64 {
        1u64 << (*self as u8)
    }

    /// Returns the index of this capability, i.e. its kernel-defined value.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn index(&self) -> u8 {
        *self as u8
    }
}

/// An `HashSet` specialized on `Capability`.
pub type CapsHashSet = std::collections::HashSet<Capability>;

/// Check if a thread contains a capability in a set.
///
/// Check if set `cset` for thread `tid` contains capability `cap`.
/// If `tid` is `None`, this operates on current thread (tid=0).
/// It cannot check Ambient or Bounding capabilities of other processes.
pub fn has_cap(tid: Option<i32>, cset: CapSet, cap: Capability) -> Result<bool, CapsError> {
    let t = tid.unwrap_or(0);
    match cset {
        CapSet::Ambient if t == 0 => ambient::has_cap(cap),
        CapSet::Bounding if t == 0 => bounding::has_cap(cap),
        CapSet::Effective | CapSet::Inheritable | CapSet::Permitted => base::has_cap(t, cset, cap),
        _ => Err("operation not supported".into()),
    }
}

/// Return all capabilities in a set for a thread.
///
/// Return current content of set `cset` for thread `tid`.
/// If `tid` is `None`, this operates on current thread (tid=0).
/// It cannot read Ambient or Bounding capabilities of other processes.
pub fn read(tid: Option<i32>, cset: CapSet) -> Result<CapsHashSet, CapsError> {
    let t = tid.unwrap_or(0);
    match cset {
        CapSet::Ambient if t == 0 => ambient::read(),
        CapSet::Bounding if t == 0 => bounding::read(),
        CapSet::Effective | CapSet::Inheritable | CapSet::Permitted => base::read(t, cset),
        _ => Err("operation not supported".into()),
    }
}

/// Set a capability set for a thread to a new value.
///
/// All and only capabilities in `value` will be set for set `cset` for thread `tid`.
/// If `tid` is `None`, this operates on current thread (tid=0).
/// It cannot manipulate Ambient set of other processes.
/// Capabilities cannot be set in Bounding set.
pub fn set(tid: Option<i32>, cset: CapSet, value: &CapsHashSet) -> Result<(), CapsError> {
    let t = tid.unwrap_or(0);
    match cset {
        CapSet::Ambient if t == 0 => ambient::set(value),
        CapSet::Effective | CapSet::Inheritable | CapSet::Permitted => base::set(t, cset, value),
        _ => Err("operation not supported".into()),
    }
}

/// Clear all capabilities in a set for a thread.
///
/// All capabilities will be cleared from set `cset` for thread `tid`.
/// If `tid` is `None`, this operates on current thread (tid=0).
/// It cannot manipulate Ambient or Bounding set of other processes.
pub fn clear(tid: Option<i32>, cset: CapSet) -> Result<(), CapsError> {
    let t = tid.unwrap_or(0);
    match cset {
        CapSet::Ambient if t == 0 => ambient::clear(),
        CapSet::Bounding if t == 0 => bounding::clear(),
        CapSet::Effective | CapSet::Permitted | CapSet::Inheritable => base::clear(t, cset),
        _ => Err("operation not supported".into()),
    }
}

/// Raise a single capability in a set for a thread.
///
/// Capabilities `cap` will be raised from set `cset` of thread `tid`.
/// If `tid` is `None`, this operates on current thread (tid=0).
/// It cannot manipulate Ambient set of other processes.
/// Capabilities cannot be raised in Bounding set.
pub fn raise(tid: Option<i32>, cset: CapSet, cap: Capability) -> Result<(), CapsError> {
    let t = tid.unwrap_or(0);
    match cset {
        CapSet::Ambient if t == 0 => ambient::raise(cap),
        CapSet::Effective | CapSet::Permitted | CapSet::Inheritable => base::raise(t, cset, cap),
        _ => Err("operation not supported".into()),
    }
}

/// Drop a single capability from a set for a thread.
///
/// Capabilities `cap` will be dropped from set `cset` of thread `tid`.
/// If `tid` is `None`, this operates on current thread (tid=0).
/// It cannot manipulate Ambient and Bounding sets of other processes.
pub fn drop(tid: Option<i32>, cset: CapSet, cap: Capability) -> Result<(), CapsError> {
    let t = tid.unwrap_or(0);
    match cset {
        CapSet::Ambient if t == 0 => ambient::drop(cap),
        CapSet::Bounding if t == 0 => bounding::drop(cap),
        CapSet::Effective | CapSet::Permitted | CapSet::Inheritable => base::drop(t, cset, cap),
        _ => Err("operation not supported".into()),
    }
}

/// Return the set of all capabilities supported by this library.
pub fn all() -> CapsHashSet {
    let slice = vec![
        Capability::CAP_CHOWN,
        Capability::CAP_DAC_OVERRIDE,
        Capability::CAP_DAC_READ_SEARCH,
        Capability::CAP_FOWNER,
        Capability::CAP_FSETID,
        Capability::CAP_KILL,
        Capability::CAP_SETGID,
        Capability::CAP_SETUID,
        Capability::CAP_SETPCAP,
        Capability::CAP_LINUX_IMMUTABLE,
        Capability::CAP_NET_BIND_SERVICE,
        Capability::CAP_NET_BROADCAST,
        Capability::CAP_NET_ADMIN,
        Capability::CAP_NET_RAW,
        Capability::CAP_IPC_LOCK,
        Capability::CAP_IPC_OWNER,
        Capability::CAP_SYS_MODULE,
        Capability::CAP_SYS_RAWIO,
        Capability::CAP_SYS_CHROOT,
        Capability::CAP_SYS_PTRACE,
        Capability::CAP_SYS_PACCT,
        Capability::CAP_SYS_ADMIN,
        Capability::CAP_SYS_BOOT,
        Capability::CAP_SYS_NICE,
        Capability::CAP_SYS_RESOURCE,
        Capability::CAP_SYS_TIME,
        Capability::CAP_SYS_TTY_CONFIG,
        Capability::CAP_MKNOD,
        Capability::CAP_LEASE,
        Capability::CAP_AUDIT_WRITE,
        Capability::CAP_AUDIT_CONTROL,
        Capability::CAP_SETFCAP,
        Capability::CAP_MAC_OVERRIDE,
        Capability::CAP_MAC_ADMIN,
        Capability::CAP_SYSLOG,
        Capability::CAP_WAKE_ALARM,
        Capability::CAP_BLOCK_SUSPEND,
        Capability::CAP_AUDIT_READ,
        Capability::CAP_PERFMON,
        Capability::CAP_BPF,
        Capability::CAP_CHECKPOINT_RESTORE,
    ];
    CapsHashSet::from_iter(slice)
}

/// Convert an informal capability name into a canonical form.
///
/// This converts the input string to uppercase and ensures that it starts with
/// `CAP_`, prepending it if necessary. It performs no validity checks so the
/// output may not represent an actual capability. To check if it is, pass it
/// to [`from_str`].
///
/// [`from_str`]: enum.Capability.html#method.from_str
pub fn to_canonical(name: &str) -> String {
    let uppername = name.to_uppercase();
    if uppername.starts_with("CAP_") {
        uppername
    } else {
        ["CAP_", &uppername].concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_all_roundtrip() {
        let all = all();
        assert!(all.len() > 0);
        for c in all {
            let name = c.to_string();
            let parsed: Capability = name.parse().unwrap();
            assert_eq!(c, parsed);
        }
    }

    #[test]
    fn test_parse_invalid() {
        let p1 = Capability::from_str("CAP_FOO");
        let p1_err = p1.unwrap_err();
        assert!(p1_err.to_string().contains("invalid"));
        assert!(format!("{}", p1_err).contains("CAP_FOO"));
        let p2: Result<Capability, CapsError> = "CAP_BAR".parse();
        assert!(p2.is_err());
    }

    #[test]
    fn test_to_canonical() {
        let p1 = "foo";
        assert!(Capability::from_str(&to_canonical(p1)).is_err());
        let p2 = "sys_admin";
        assert!(Capability::from_str(&to_canonical(p2)).is_ok());
        let p3 = "CAP_SYS_CHROOT";
        assert!(Capability::from_str(&to_canonical(p3)).is_ok());
    }

    #[test]
    #[cfg(feature = "serde_support")]
    fn test_serde() {
        let p1 = Capability::from_str("CAP_CHOWN").unwrap();
        let ser = serde_json::to_value(&p1).unwrap();
        let deser: Capability = serde_json::from_value(ser).unwrap();
        assert_eq!(deser, p1);
    }
}
