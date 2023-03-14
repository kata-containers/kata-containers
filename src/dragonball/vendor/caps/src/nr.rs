/* from <linux/capability.h> */

pub const CAP_CHOWN: u8 = 0;
pub const CAP_DAC_OVERRIDE: u8 = 1;
pub const CAP_DAC_READ_SEARCH: u8 = 2;
pub const CAP_FOWNER: u8 = 3;
pub const CAP_FSETID: u8 = 4;
pub const CAP_KILL: u8 = 5;
pub const CAP_SETGID: u8 = 6;
pub const CAP_SETUID: u8 = 7;
pub const CAP_SETPCAP: u8 = 8;
pub const CAP_LINUX_IMMUTABLE: u8 = 9;
pub const CAP_NET_BIND_SERVICE: u8 = 10;
pub const CAP_NET_BROADCAST: u8 = 11;
pub const CAP_NET_ADMIN: u8 = 12;
pub const CAP_NET_RAW: u8 = 13;
pub const CAP_IPC_LOCK: u8 = 14;
pub const CAP_IPC_OWNER: u8 = 15;
pub const CAP_SYS_MODULE: u8 = 16;
pub const CAP_SYS_RAWIO: u8 = 17;
pub const CAP_SYS_CHROOT: u8 = 18;
pub const CAP_SYS_PTRACE: u8 = 19;
pub const CAP_SYS_PACCT: u8 = 20;
pub const CAP_SYS_ADMIN: u8 = 21;
pub const CAP_SYS_BOOT: u8 = 22;
pub const CAP_SYS_NICE: u8 = 23;
pub const CAP_SYS_RESOURCE: u8 = 24;
pub const CAP_SYS_TIME: u8 = 25;
pub const CAP_SYS_TTY_CONFIG: u8 = 26;
pub const CAP_MKNOD: u8 = 27;
pub const CAP_LEASE: u8 = 28;
pub const CAP_AUDIT_WRITE: u8 = 29;
pub const CAP_AUDIT_CONTROL: u8 = 30;
pub const CAP_SETFCAP: u8 = 31;
pub const CAP_MAC_OVERRIDE: u8 = 32;
pub const CAP_MAC_ADMIN: u8 = 33;
pub const CAP_SYSLOG: u8 = 34;
pub const CAP_WAKE_ALARM: u8 = 35;
pub const CAP_BLOCK_SUSPEND: u8 = 36;
pub const CAP_AUDIT_READ: u8 = 37;
pub const CAP_PERFMON: u8 = 38;
pub const CAP_BPF: u8 = 39;
pub const CAP_CHECKPOINT_RESTORE: u8 = 40;

/* from <sys/prctl.h> */

pub const PR_GET_KEEPCAPS: i32 = 7;
pub const PR_SET_KEEPCAPS: i32 = 8;
pub const PR_CAPBSET_READ: i32 = 23;
pub const PR_CAPBSET_DROP: i32 = 24;
pub const PR_CAP_AMBIENT: i32 = 47;
pub const PR_CAP_AMBIENT_IS_SET: i32 = 1;
pub const PR_CAP_AMBIENT_RAISE: i32 = 2;
pub const PR_CAP_AMBIENT_LOWER: i32 = 3;
pub const PR_CAP_AMBIENT_CLEAR_ALL: i32 = 4;

/* from <unistd.h> */

#[cfg(target_arch = "x86")]
pub const CAPGET: i32 = 184;
#[cfg(target_arch = "x86")]
pub const CAPSET: i32 = 185;

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub const CAPGET: i64 = 125;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
pub const CAPSET: i64 = 126;

#[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
pub const CAPGET: i32 = 0x40000000 + 125;
#[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
pub const CAPSET: i32 = 0x40000000 + 126;

#[cfg(target_arch = "aarch64")]
pub const CAPGET: i64 = 90;
#[cfg(target_arch = "aarch64")]
pub const CAPSET: i64 = 91;

#[cfg(target_arch = "powerpc")]
pub const CAPGET: i32 = 183;
#[cfg(target_arch = "powerpc")]
pub const CAPSET: i32 = 184;

#[cfg(target_arch = "powerpc64")]
pub const CAPGET: i64 = 183;
#[cfg(target_arch = "powerpc64")]
pub const CAPSET: i64 = 184;

#[cfg(target_arch = "mips")]
pub const CAPGET: i32 = 4204;
#[cfg(target_arch = "mips")]
pub const CAPSET: i32 = 4205;

#[cfg(target_arch = "mips64")]
pub const CAPGET: i64 = 5123;
#[cfg(target_arch = "mips64")]
pub const CAPSET: i64 = 5124;

#[cfg(target_arch = "arm")]
pub const CAPGET: i32 = 184;
#[cfg(target_arch = "arm")]
pub const CAPSET: i32 = 185;

#[cfg(target_arch = "s390x")]
pub const CAPGET: i64 = 184;
#[cfg(target_arch = "s390x")]
pub const CAPSET: i64 = 185;

#[cfg(target_arch = "sparc")]
pub const CAPGET: i64 = 21;
#[cfg(target_arch = "sparc")]
pub const CAPSET: i64 = 22;

#[cfg(target_arch = "sparc64")]
pub const CAPGET: i64 = 21;
#[cfg(target_arch = "sparc64")]
pub const CAPSET: i64 = 22;

#[cfg(target_arch = "riscv64")]
pub const CAPGET: i64 = 90;
#[cfg(target_arch = "riscv64")]
pub const CAPSET: i64 = 91;

#[cfg(target_arch = "loongarch64")]
pub const CAPGET: i64 = 90;
#[cfg(target_arch = "loongarch64")]
pub const CAPSET: i64 = 91;
