use serde::{
    de::{Deserializer, Error},
    Deserialize, Serialize,
};
use std::collections::HashSet;

/// Capabilities is a unique set of Capability values.
pub type Capabilities = HashSet<Capability>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
/// All available capabilities.
///
/// For the purpose of performing permission checks, traditional UNIX
/// implementations distinguish two categories of processes: privileged
/// processes (whose effective user ID is 0, referred to as superuser or root),
/// and unprivileged processes (whose effective UID is nonzero). Privileged
/// processes bypass all kernel permission checks, while unprivileged processes
/// are subject to full permission checking based on the process's credentials
/// (usually: effective UID, effective GID, and supplementary group list).
///
/// Starting with kernel 2.2, Linux divides the privileges traditionally
/// associated with superuser into distinct units, known as capabilities, which
/// can be independently enabled and disabled. Capabilities are a per-thread attribute.
pub enum Capability {
    #[serde(rename = "CAP_AUDIT_CONTROL")]
    /// Enable and disable kernel auditing; change auditing filter rules;
    /// retrieve auditing status and filtering rules.
    ///
    /// _since Linux 2.6.11_
    AuditControl,

    #[serde(rename = "CAP_AUDIT_READ")]
    /// Allow reading the audit log via multicast netlink socket.
    ///
    /// _since Linux 3.16_
    AuditRead,

    #[serde(rename = "CAP_AUDIT_WRITE")]
    /// Write records to kernel auditing log.
    ///
    /// _since Linux 2.6.11_
    AuditWrite,

    #[serde(rename = "CAP_BLOCK_SUSPEND")]
    /// Employ features that can block system suspend
    /// ([epoll(7)](https://man7.org/linux/man-pages/man7/epoll.7.html)
    /// **EPOLLWAKEUP**, `/proc/sys/wake_lock`).
    ///
    /// _since Linux 3.5_
    BlockSuspend,

    #[serde(rename = "CAP_BPF")]
    /// Employ privileged BPF operations; see
    /// [bpf(2)](https://man7.org/linux/man-pages/man2/bpf.2.html) and
    /// [bpf-helpers(7)](https://man7.org/linux/man-pages/man7/bpf-helpers.7.html).
    ///
    /// This capability was added to separate out BPF functionality from the
    /// overloaded **CAP_SYS_ADMIN** capability.
    ///
    /// _since Linux 5.8_
    Bpf,

    #[serde(rename = "CAP_CHECKPOINT_RESTORE")]
    /// - update `/proc/sys/kernel/ns_last_pid` (see
    ///   [pid_namespaces(7)](https://man7.org/linux/man-pages/man7/pid_namespaces.7.html))
    /// - employ the set_tid feature of
    ///   [clone3(2)](https://man7.org/linux/man-pages/man2/clone3.2.html)
    /// - read the contents of the symbolic links in `/proc/[pid]/map_files` for
    ///   other processes.
    ///
    /// This capability was added to separate out BPF functionality from the
    /// overloaded **CAP_SYS_ADMIN** capability.
    ///
    /// _since Linux 5.9_
    CheckpointRestore,

    #[serde(rename = "CAP_CHOWN")]
    /// Make arbitrary changes to file UIDs and GIDs (see
    /// [chown(2)](https://man7.org/linux/man-pages/man2/chown.2.html)).
    Chown,

    #[serde(rename = "CAP_DAC_OVERRIDE")]
    /// Bypass file read, write, and execute permission checks.
    ///
    /// (DAC is an abbreviation of "discretionary access control".)
    DacOverride,

    #[serde(rename = "CAP_DAC_READ_SEARCH")]
    /// - bypass file read permission checks and directory read and execute
    ///   permission checks
    /// - invoke
    ///   [open_by_handle_at(2)](https://man7.org/linux/man-pages/man2/open_by_handle_at.2.html)
    /// - use the
    ///   [linkat(2)](https://man7.org/linux/man-pages/man2/linkat.2.html)
    ///   **AT_EMPTY_PATH** flag to create a link to a file referred to by a
    ///   file descriptor.
    DacReadSearch,

    #[serde(rename = "CAP_FOWNER")]
    /// - Bypass permission checks on operations that normally require the
    ///   filesystem UID of the process to match the UID of the file (e.g.,
    ///   [chmod(2)](https://man7.org/linux/man-pages/man2/chmod.2.html),
    ///   [utime(2)](https://man7.org/linux/man-pages/man2/utime.2.html)),
    ///   excluding those operations covered by **CAP_DAC_OVERRIDE** and
    ///   **CAP_DAC_READ_SEARCH**
    /// - set inode flags (see
    ///   [ioctl_iflags(2)](https://man7.org/linux/man-pages/man2/ioctl_iflags.2.html))
    ///   on arbitrary files
    /// - set Access Control Lists (ACLs) on arbitrary files
    /// - ignore directory sticky bit on file deletion
    /// - modify user extended attributes on sticky directory owned by any user
    /// - specify **O_NOATIME** for arbitrary files in
    ///   [open(2)](https://man7.org/linux/man-pages/man2/open.2.html) and
    ///   [fcntl(2)](https://man7.org/linux/man-pages/man2/fcntl.2.html)
    ///
    /// Overrides all restrictions about allowed operations on files, where
    /// file owner ID must be equal to the user ID, except where CAP_FSETID
    /// is applicable. It doesn't override MAC and DAC restrictions.
    Fowner,

    #[serde(rename = "CAP_FSETID")]
    /// - don't clear set-user-ID and set-group-ID mode bits when a file is
    ///   modified
    /// - set the set-group-ID bit for a file whose GID does not match the
    ///   filesystem or any of the supplementary GIDs of the calling process
    Fsetid,

    #[serde(rename = "CAP_IPC_LOCK")]
    /// - lock memory
    ///   ([mlock(2)](https://man7.org/linux/man-pages/man2/mlock.2.html),
    ///   [mlockall(2)](https://man7.org/linux/man-pages/man2/mlockall.2.html),
    ///   [mmap(2)](https://man7.org/linux/man-pages/man2/mmap.2.html),
    ///   [shmctl(2)](https://man7.org/linux/man-pages/man2/shmctl.2.html))
    /// - allocate memory using huge pages
    ///   ([memfd_create(2)](https://man7.org/linux/man-pages/man2/memfd_create.2.html)
    ///   [mmap(2)](https://man7.org/linux/man-pages/man2/mmap.2.html),
    ///   [shmctl(2)](https://man7.org/linux/man-pages/man2/shmctl.2.html))
    IpcLock,

    #[serde(rename = "CAP_IPC_OWNER")]
    /// Bypass permission checks for operations on System V IPC objects.
    IpcOwner,

    #[serde(rename = "CAP_KILL")]
    /// Bypass permission checks for sending signals (see
    /// [kill(2)](https://man7.org/linux/man-pages/man2/kill.2.html)). This
    /// includes use of the
    /// [ioctl(2)](https://man7.org/linux/man-pages/man2/ioctl.2.html)
    /// **KDSIGACCEPT** operation.
    Kill,

    #[serde(rename = "CAP_LEASE")]
    /// Establish leases on arbitrary files (see
    /// [fcntl(2)](https://man7.org/linux/man-pages/man2/fcntl.2.html)).
    ///
    /// _since Linux 2.4_
    Lease,

    #[serde(rename = "CAP_LINUX_IMMUTABLE")]
    /// Set the **FS_APPEND_FL** and **FS_IMMUTABLE_FL** inode flags (see
    /// [ioctl_iflags(2)](https://man7.org/linux/man-pages/man2/ioctl_iflags.2.html)).
    LinuxImmutable,

    #[serde(rename = "CAP_MAC_ADMIN")]
    /// Allow MAC configuration or state changes.
    ///
    /// Implemented for the Smack Linux Security Module (LSM).
    ///
    /// _since Linux 2.6.25_
    MacAdmin,

    #[serde(rename = "CAP_MAC_OVERRIDE")]
    /// Override Mandatory Access Control (MAC).
    ///
    /// Implemented for the Smack Linux Security Module (LSM).
    ///
    /// _since Linux 2.6.25_
    MacOverride,

    #[serde(rename = "CAP_MKNOD")]
    /// Create special files using
    /// [mknod(2)](https://man7.org/linux/man-pages/man2/mknod.2.html).
    ///
    /// _since Linux 2.4_
    Mknod,

    #[serde(rename = "CAP_NET_ADMIN")]
    /// Perform various network-related operations:
    /// - interface configuration
    /// - administration of IP firewall, masquerading, and accounting
    /// - modify routing tables
    /// - bind to any address for transparent proxying
    /// - set type-of-service (TOS)
    /// - clear driver statistics
    /// - set promiscuous mode
    /// - enabling multicasting
    /// - use
    ///   [setsockopt(2)](https://man7.org/linux/man-pages/man2/setsockopt.2.html)
    ///   to set the following socket options: **SO_DEBUG**, **SO_MARK**,
    ///   **SO_PRIORITY** (for a priority outside the range 0 to 6),
    ///   **SO_RCVBUFFORCE** and **SO_SNDBUFFORCE**
    NetAdmin,

    #[serde(rename = "CAP_NET_BIND_SERVICE")]
    /// Bind a socket to Internet domain privileged ports (port numbers less
    /// than 1024).
    NetBindService,

    #[serde(rename = "CAP_NET_BROADCAST")]
    /// (Unused) Make socket broadcasts, and listen to multicasts.
    NetBroadcast,

    #[serde(rename = "CAP_NET_RAW")]
    /// - use RAW and PACKET sockets
    /// - bind to any address for transparent proxying
    NetRaw,

    #[serde(rename = "CAP_PERFMON")]
    /// Employ various performance-monitoring mechanisms, including:
    /// - call
    ///   [perf_event_open(2)](https://man7.org/linux/man-pages/man2/perf_event_open.2.html)
    /// - employ various BPF operations that have performance implications
    ///
    /// This capability was added to separate out performance monitoring
    /// functionality from the overloaded **CAP_SYS_ADMIN** capability. See also
    /// the kernel source file `Documentation/admin-guide/perf-security.rst`.
    ///
    /// _since Linux 5.8_
    Perfmon,

    #[serde(rename = "CAP_SETGID")]
    /// - make arbitrary manipulations of process GIDs and supplementary GID list
    /// - forge GID when passing socket credentials via UNIX domain sockets
    /// - write a group ID mapping in a user namespace (see
    ///   [user_namespaces(7)](https://man7.org/linux/man-pages/man7/user_namespaces.7.html))
    Setgid,

    #[serde(rename = "CAP_SETFCAP")]
    /// Set arbitrary capabilities on a file.
    ///
    /// _since Linux 2.6.24_
    Setfcap,

    #[serde(rename = "CAP_SETPCAP")]
    /// If file capabilities are supported (i.e., since LinuxIDMapping 2.6.24):
    /// add any capability from the calling thread's bounding set to its
    /// inheritable set; drop capabilities from the bounding set (via
    /// [prctl(2)](https://man7.org/linux/man-pages/man2/prctl.2.html)
    /// **PR_CAPBSET_DROP**); make changes to the `securebits` flags.
    ///
    /// If file capabilities are not supported (i.e., kernels before Linux
    /// 2.6.24): grant or remove any capability in the caller's permitted
    /// capability set to or from any other process. (This property of
    /// **CAP_SETPCAP** is not available when the kernel is configured to
    /// support file capabilities, since **CAP_SETPCAP** has entirely different
    /// semantics for such kernels.)
    Setpcap,

    #[serde(rename = "CAP_SETUID")]
    /// - make arbitrary manipulations of process UIDs
    ///   ([setuid(2)](https://man7.org/linux/man-pages/man2/setuid.2.html),
    ///   [setreuid(2)](https://man7.org/linux/man-pages/man2/setreuid.2.html),
    ///   [setresuid(2)](https://man7.org/linux/man-pages/man2/setresuid.2.html),
    ///   [setfsuid(2)](https://man7.org/linux/man-pages/man2/setfsuid.2.html))
    /// - forge UID when passing socket credentials via UNIX domain sockets
    /// - write a user ID mapping in a user namespace (see
    ///   [user_namespaces(7)](https://man7.org/linux/man-pages/man7/user_namespaces.7.html))
    Setuid,

    #[serde(rename = "CAP_SYS_ADMIN")]
    /// - perform a range of system administration operations including:
    ///   [quotactl(2)](https://man7.org/linux/man-pages/man2/quotactl.2.html),
    ///   [mount(2)](https://man7.org/linux/man-pages/man2/mount.2.html),
    ///   [umount(2)](https://man7.org/linux/man-pages/man2/umount.2.html),
    ///   [pivot_root(2)](https://man7.org/linux/man-pages/man2/pivot_root.2.html),
    ///   [swapon(2)](https://man7.org/linux/man-pages/man2/swapon.2.html),
    ///   [swapoff(2)](https://man7.org/linux/man-pages/man2/swapoff.2.html),
    ///   [sethostname(2)](https://man7.org/linux/man-pages/man2/sethostname.2.html),
    ///   and [setdomainname(2)](https://man7.org/linux/man-pages/man2/setdomainname.2.html)
    /// - perform privileged
    ///   [syslog(2)](https://man7.org/linux/man-pages/man2/syslog.2.html)
    ///   operations (since Linux 2.6.37, **CAP_SYSLOG** should be used to
    ///   permit such operations)
    /// - perform **VM86_REQUEST_IRQ vm86**(2) command
    /// - access the same checkpoint/restore functionality that is governed by
    ///   **CAP_CHECKPOINT_RESTORE** (but the latter, weaker capability is
    ///   preferred for accessing that functionality)
    /// - perform the same BPF operations as are governed by **CAP_BPF** (but
    ///   the latter, weaker capability is preferred for accessing that
    ///   functionality).
    /// - employ the same performance monitoring mechanisms as are governed by
    ///   **CAP_PERFMON** (but the latter, weaker capability is preferred for
    ///   accessing that functionality).
    /// - perform **IPC_SET** and **IPC_RMID** operations on arbitrary System V
    ///   IPC objects
    /// - override **RLIMIT_NPROC** resource limit
    /// - perform operations on `trusted` and `security` extended attributes
    ///   (see [xattr(7)](https://man7.org/linux/man-pages/man7/xattr.7.html))
    /// - use
    ///   [lookup_dcookie(2)](https://man7.org/linux/man-pages/man2/lookup_dcookie.2.html)
    /// - use
    ///   [ioprio_set(2)](https://man7.org/linux/man-pages/man2/ioprio_set.2.html)
    ///   to assign **IOPRIO_CLASS_RT** and (before Linux 2.6.25)
    ///   **IOPRIO_CLASS_IDLE** I/O scheduling classes
    /// - forge PID when passing socket credentials via UNIX domain sockets
    /// - exceed `/proc/sys/fs/file-max`, the system-wide limit on the number of
    ///   open files, in system calls that open files (e.g.,
    ///   [accept(2)](https://man7.org/linux/man-pages/man2/accept.2.html),
    ///   [execve(2)](https://man7.org/linux/man-pages/man2/execve.2.html),
    ///   [open(2)](https://man7.org/linux/man-pages/man2/open.2.html),
    ///   [pipe(2)](https://man7.org/linux/man-pages/man2/pipe.2.html))
    /// - employ **CLONE_*** flags that create new namespaces with
    ///   [clone(2)](https://man7.org/linux/man-pages/man2/clone.2.html) and
    ///   [unshare(2)](https://man7.org/linux/man-pages/man2/unshare.2.html)
    ///   (but, since Linux 3.8, creating user namespaces does not require any
    ///   capability)
    /// - access privileged `perf` event information
    /// - call [setns(2)](https://man7.org/linux/man-pages/man2/setns.2.html)
    ///   (requires **CAP_SYS_ADMIN** in the `target` namespace)
    /// - call
    ///   [fanotify_init(2)](https://man7.org/linux/man-pages/man2/fanotify_init.2.html)
    /// - perform privileged **KEYCTL_CHOWN** and **KEYCTL_SETPERM**
    ///   [keyctl(2)](https://man7.org/linux/man-pages/man2/keyctl.2.html)
    ///   operations
    /// - perform
    ///   [madvise(2)](https://man7.org/linux/man-pages/man2/madvise.2.html)
    ///   **MADV_HWPOISON** operation
    /// - employ the **TIOCSTI ioctl**(2) to insert characters into the input
    ///   queue of a terminal other than the caller's controlling terminal
    /// - employ the obsolete
    ///   [nfsservctl(2)](https://man7.org/linux/man-pages/man2/nfsservctl.2.html)
    ///   system call
    /// - employ the obsolete
    ///   [bdflush(2)](https://man7.org/linux/man-pages/man2/bdflush.2.html)
    ///   system call
    /// - perform various privileged block-device
    ///   [ioctl(2)](https://man7.org/linux/man-pages/man2/ioctl.2.html)
    ///   operations
    /// - perform various privileged filesystem
    ///   [ioctl(2)](https://man7.org/linux/man-pages/man2/ioctl.2.html)
    ///   operations
    /// - perform privileged
    ///   [ioctl(2)](https://man7.org/linux/man-pages/man2/ioctl.2.html)
    ///   operations on the `/dev/random` device (see
    ///   [random(4)](https://man7.org/linux/man-pages/man4/random.4.html))
    /// - install a
    ///   [seccomp(2)](https://man7.org/linux/man-pages/man2/seccomp.2.html)
    ///   filter without first having to set the `no_new_privs` thread attribute
    /// - modify allow/deny rules for device control groups
    /// - employ the
    ///   [ptrace(2)](https://man7.org/linux/man-pages/man2/ptrace.2.html)
    ///   **PTRACE_SECCOMP_GET_FILTER** operation to dump tracee's seccomp
    ///   filters
    /// - employ the
    ///   [ptrace(2)](https://man7.org/linux/man-pages/man2/ptrace.2.html)
    ///   **PTRACE_SETOPTIONS** operation to suspend the tracee's seccomp
    ///   protections (i.e., the **PTRACE_O_SUSPEND_SECCOMP** flag)
    /// - perform administrative operations on many device drivers
    /// - modify autogroup nice values by writing to `/proc/[pid]/autogroup`
    ///   (see [sched(7)](https://man7.org/linux/man-pages/man7/sched.7.html))
    SysAdmin,

    #[serde(rename = "CAP_SYS_BOOT")]
    /// Use [reboot(2)](https://man7.org/linux/man-pages/man2/reboot.2.html) and
    /// [kexec_load(2)](https://man7.org/linux/man-pages/man2/kexec_load.2.html).
    SysBoot,

    #[serde(rename = "CAP_SYS_CHROOT")]
    /// - use [chroot(2)](https://man7.org/linux/man-pages/man2/chroot.2.html)
    /// - change mount namespaces using
    ///   [setns(2)](https://man7.org/linux/man-pages/man2/setns.2.html)
    SysChroot,

    #[serde(rename = "CAP_SYS_MODULE")]
    /// - load and unload kernel modules (see
    ///   [init_module(2)](https://man7.org/linux/man-pages/man2/init_module.2.html)
    ///   and
    ///   [delete_module(2)](https://man7.org/linux/man-pages/man2/delete_module.2.html))
    /// - in kernels before 2.6.25: drop capabilities from the system-wide
    ///   capability bounding set
    SysModule,

    #[serde(rename = "CAP_SYS_NICE")]
    /// - lower the process nice value
    ///   ([nice(2)](https://man7.org/linux/man-pages/man2/nice.2.html),
    ///   [setpriority(2)](https://man7.org/linux/man-pages/man2/setpriority.2.html))
    ///   and change the nice value for arbitrary processes
    /// - set real-time scheduling policies for calling process, and set
    ///   scheduling policies and priorities for arbitrary processes
    ///   ([sched_setscheduler(2)](https://man7.org/linux/man-pages/man2/sched_setscheduler.2.html),
    ///   [sched_setparam(2)](https://man7.org/linux/man-pages/man2/sched_setparam.2.html),
    ///   [sched_setattr(2)](https://man7.org/linux/man-pages/man2/sched_setattr.2.html))
    /// - set CPU affinity for arbitrary processes
    ///   ([sched_setaffinity(2)](https://man7.org/linux/man-pages/man2/sched_setaffinity.2.html))
    /// - set I/O scheduling class and priority for arbitrary processes
    ///   ([ioprio_set(2)](https://man7.org/linux/man-pages/man2/ioprio_set.2.html))
    /// - apply
    ///   [migrate_pages(2)](https://man7.org/linux/man-pages/man2/migrate_pages.2.html)
    ///   to arbitrary processes and allow processes to be migrated to arbitrary
    ///   nodes
    /// - apply
    ///   [move_pages(2)](https://man7.org/linux/man-pages/man2/move_pages.2.html)
    ///   to arbitrary processes
    /// - use the **MPOL_MF_MOVE_ALL** flag with
    ///   [mbind(2)](https://man7.org/linux/man-pages/man2/mbind.2.html) and
    ///   [move_pages(2)](https://man7.org/linux/man-pages/man2/move_pages.2.html)
    SysNice,

    #[serde(rename = "CAP_SYS_PACCT")]
    /// Use [acct(2)](https://man7.org/linux/man-pages/man2/acct.2.html).
    SysPacct,

    #[serde(rename = "CAP_SYS_PTRACE")]
    /// - trace arbitrary processes using
    ///   [ptrace(2)](https://man7.org/linux/man-pages/man2/ptrace.2.html)
    /// - apply
    ///   [get_robust_list(2)](https://man7.org/linux/man-pages/man2/get_robust_list.2.html)
    ///   to arbitrary processes
    /// - transfer data to or from the memory of arbitrary processes using
    ///   [process_vm_readv(2)](https://man7.org/linux/man-pages/man2/process_vm_readv.2.html)
    ///   and
    ///   [process_vm_writev(2)](https://man7.org/linux/man-pages/man2/process_vm_writev.2.html)
    /// - inspect processes using
    ///   [kcmp(2)](https://man7.org/linux/man-pages/man2/kcmp.2.html)
    SysPtrace,

    #[serde(rename = "CAP_SYS_RAWIO")]
    /// - perform I/O port operations
    ///   ([iopl(2)](https://man7.org/linux/man-pages/man2/iopl.2.html) and
    ///   [ioperm(2)](https://man7.org/linux/man-pages/man2/ioperm.2.html));
    /// - access `/proc/kcore`
    /// - employ the **FIBMAP ioctl**(2) operation
    /// - open devices for accessing x86 model-specific registers (MSRs, see
    ///   [msr(4)](https://man7.org/linux/man-pages/man4/msr.4.html))
    /// - update `/proc/sys/vm/mmap_min_addr`
    /// - create memory mappings at addresses below the value specified by
    ///   `/proc/sys/vm/mmap_min_addr`
    /// - map files in `/proc/bus/pci`
    /// - open `/dev/mem` and `/dev/kmem`
    /// - perform various SCSI device commands
    /// - perform certain operations on
    ///   [hpsa(4)](https://man7.org/linux/man-pages/man4/hpsa.4.html) and
    ///   [cciss(4)](https://man7.org/linux/man-pages/man4/cciss.4.html) devices
    /// - perform a range of device-specific operations on other devices
    SysRawio,

    #[serde(rename = "CAP_SYS_RESOURCE")]
    /// - use reserved space on ext2 filesystems
    /// - make [ioctl(2)](https://man7.org/linux/man-pages/man2/ioctl.2.html)
    ///   calls controlling ext3 journaling
    /// - override disk quota limits
    /// - increase resource limits (see
    ///   [setrlimit(2)](https://man7.org/linux/man-pages/man2/setrlimit.2.html))
    /// - override **RLIMIT_NPROC** resource limit
    /// - override maximum number of consoles on console allocation
    /// - override maximum number of keymaps
    /// - allow more than 64hz interrupts from the real-time clock
    /// - raise `msg_qbytes` limit for a System V message queue above the limit
    ///   in `/proc/sys/kernel/msgmnb` (see
    ///   [msgop(2)](https://man7.org/linux/man-pages/man2/msgop.2.html) and
    ///   [msgctl(2)](https://man7.org/linux/man-pages/man2/msgctl.2.html))
    /// - allow the **RLIMIT_NOFILE** resource limit on the number of
    ///   "in-flight" file descriptors to be bypassed when passing file
    ///   descriptors to another process via a UNIX domain socket (see
    ///   [unix(7)](https://man7.org/linux/man-pages/man7/unix.7.html));
    /// - override the `/proc/sys/fs/pipe-size-max` limit when setting the
    ///   capacity of a pipe using the **F_SETPIPE_SZ**
    ///   [fcntl(2)](https://man7.org/linux/man-pages/man2/fcntl.2.html) command
    /// - use **F_SETPIPE_SZ** to increase the capacity of a pipe above the
    ///   limit specified by `/proc/sys/fs/pipe-max-size`
    /// - override `/proc/sys/fs/mqueue/queues_max`,
    ///   `/proc/sys/fs/mqueue/msg_max` and `/proc/sys/fs/mqueue/msgsize_max`
    ///   limits when creating POSIX message queues (see
    ///   [mq_overview(7)](https://man7.org/linux/man-pages/man7/mq_overview.7.html))
    /// - employ the
    ///   [prctl(2)](https://man7.org/linux/man-pages/man2/prctl.2.html)
    ///   **PR_SET_MM** operation
    /// - set `/proc/[pid]/oom_score_adj` to a value lower than the value last
    ///   set by a process with **CAP_SYS_RESOURCE**
    SysResource,

    #[serde(rename = "CAP_SYS_TIME")]
    /// - set system clock
    ///   ([settimeofday(2)](https://man7.org/linux/man-pages/man2/settimeofday.2.html),
    ///   [stime(2)](https://man7.org/linux/man-pages/man2/stime.2.html),
    ///   [adjtimex(2)](https://man7.org/linux/man-pages/man2/adjtimex.2.html))
    /// - set real-time (hardware) clock
    SysTime,

    #[serde(rename = "CAP_SYS_TTY_CONFIG")]
    /// - use
    ///   [vhangup(2)](https://man7.org/linux/man-pages/man2/vhangup.2.html)
    /// - employ various privileged
    ///   [ioctl(2)](https://man7.org/linux/man-pages/man2/ioctl.2.html)
    ///   operations on virtual terminals
    SysTtyConfig,

    #[serde(rename = "CAP_SYSLOG")]
    /// - perform privileged
    ///   [syslog(2)](https://man7.org/linux/man-pages/man2/syslog.2.html)
    ///   operations. See
    ///   [syslog(2)](https://man7.org/linux/man-pages/man2/syslog.2.html) for
    ///   information on which operations require privilege.
    /// - view kernel addresses exposed via `/proc` and other interfaces when
    ///   `/proc/sys/kernel/kptr_restrict` has the value 1. (See the discussion
    ///   of the `kptr_restrict` in
    ///   [proc(5)](https://man7.org/linux/man-pages/man5/proc.5.html).)
    ///
    /// _since Linux 2.6.37_
    Syslog,

    #[serde(rename = "CAP_WAKE_ALARM")]
    /// Trigger something that will wake up the system (set
    /// **CLOCK_REALTIME_ALARM** and **CLOCK_BOOTTIME_ALARM** timers).
    /// _since Linux 3.0_
    WakeAlarm,
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        let upper = input.to_uppercase();
        let stripped = upper.strip_prefix("CAP_").unwrap_or(&upper);
        match stripped {
            "AUDIT_CONTROL" => Ok(Self::AuditControl),
            "AUDIT_READ" => Ok(Self::AuditRead),
            "AUDIT_WRITE" => Ok(Self::AuditWrite),
            "BLOCK_SUSPEND" => Ok(Self::BlockSuspend),
            "BPF" => Ok(Self::Bpf),
            "CHECKPOINT_RESTORE" => Ok(Self::CheckpointRestore),
            "CHOWN" => Ok(Self::Chown),
            "DAC_OVERRIDE" => Ok(Self::DacOverride),
            "DAC_READ_SEARCH" => Ok(Self::DacReadSearch),
            "FOWNER" => Ok(Self::Fowner),
            "FSETID" => Ok(Self::Fsetid),
            "IPC_LOCK" => Ok(Self::IpcLock),
            "IPC_OWNER" => Ok(Self::IpcOwner),
            "KILL" => Ok(Self::Kill),
            "LEASE" => Ok(Self::Lease),
            "LINUX_IMMUTABLE" => Ok(Self::LinuxImmutable),
            "MAC_ADMIN" => Ok(Self::MacAdmin),
            "MAC_OVERRIDE" => Ok(Self::MacOverride),
            "MKNOD" => Ok(Self::Mknod),
            "NET_ADMIN" => Ok(Self::NetAdmin),
            "NET_BIND_SERVICE" => Ok(Self::NetBindService),
            "NET_BROADCAST" => Ok(Self::NetBroadcast),
            "NET_RAW" => Ok(Self::NetRaw),
            "PERFMON" => Ok(Self::Perfmon),
            "SETGID" => Ok(Self::Setgid),
            "SETFCAP" => Ok(Self::Setfcap),
            "SETPCAP" => Ok(Self::Setpcap),
            "SETUID" => Ok(Self::Setuid),
            "SYS_ADMIN" => Ok(Self::SysAdmin),
            "SYS_BOOT" => Ok(Self::SysBoot),
            "SYS_CHROOT" => Ok(Self::SysChroot),
            "SYS_MODULE" => Ok(Self::SysModule),
            "SYS_NICE" => Ok(Self::SysNice),
            "SYS_PACCT" => Ok(Self::SysPacct),
            "SYS_PTRACE" => Ok(Self::SysPtrace),
            "SYS_RAWIO" => Ok(Self::SysRawio),
            "SYS_RESOURCE" => Ok(Self::SysResource),
            "SYS_TIME" => Ok(Self::SysTime),
            "SYS_TTY_CONFIG" => Ok(Self::SysTtyConfig),
            "SYSLOG" => Ok(Self::Syslog),
            "WAKE_ALARM" => Ok(Self::WakeAlarm),
            other => Err(Error::custom(format!(
                "no variant for {} (converted to {})",
                input, other,
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;

    #[test]
    fn serialize() {
        let chown = Capability::Chown;
        let res = serde_json::to_string(&chown).expect("unable to serialize");
        assert_eq!("\"CAP_CHOWN\"", res);
    }

    #[test]
    fn deserialize() -> Result<()> {
        for case in &["SYSLOG", "CAP_SYSLOG", "cap_SYSLOG", "sySloG"] {
            let res: Capability = serde_json::from_str(&format!("\"{}\"", case))?;
            assert_eq!(Capability::Syslog, res);
        }
        Ok(())
    }

    #[test]
    fn capabilities() -> Result<()> {
        let res: Capabilities = serde_json::from_str(
            r#"[
                "syslog",
                "SYSLOG",
                "chown",
                "cap_chown"
            ]"#,
        )?;
        assert_eq!(res.len(), 2);
        assert!(res.contains(&Capability::Syslog));
        assert!(res.contains(&Capability::Chown));
        Ok(())
    }
}
