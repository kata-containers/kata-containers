//! Process status information information from `/proc/[pid]/status`.

use std::fs::File;
use std::io::Result;

use libc::{gid_t, mode_t, pid_t, uid_t};
use nom::{IResult, line_ending, multispace, not_line_ending, space};

use parsers::{
    map_result,
    parse_i32,
    parse_bit,
    parse_i32s,
    parse_kb,
    parse_line,
    parse_u32,
    parse_u32_mask_list,
    parse_u32_octal,
    parse_u32s,
    parse_u64,
    parse_u64_hex,
    read_to_end
};
use pid::State;

/// The Secure Computing state of a process.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SeccompMode {
    Disabled,
    Strict,
    Filter,
}

impl Default for SeccompMode {
    fn default() -> SeccompMode {
        SeccompMode::Disabled
    }
}

named!(parse_seccomp_mode<SeccompMode>,
       alt!(tag!("0") => { |_| SeccompMode::Disabled }
          | tag!("1") => { |_| SeccompMode::Strict   }
          | tag!("2") => { |_| SeccompMode::Filter   }));

/// Process status information.
///
/// See `man 5 proc` and `Linux/fs/proc/array.c`.
#[derive(Default, Debug, PartialEq, Eq, Hash)]
pub struct Status {
    /// Filename of the executable.
    pub command: String,
    /// File mode creation mask (since Linux 4.7).
    pub umask: mode_t,
    /// Current state of the process.
    pub state: State,
    /// Process ID (i.e., Thread Group ID).
    pub pid: pid_t,
    /// NUMA group ID.
    pub numa_gid: pid_t,
    /// Thread ID.
    pub tid: pid_t,
    /// Process ID of parent process.
    pub ppid: pid_t,
    /// Process ID of the process tracing this process (0 if not being traced).
    pub tracer_pid: pid_t,
    /// Real user ID.
    pub uid_real: uid_t,
    /// Effective user ID.
    pub uid_effective: uid_t,
    /// Saved user ID.
    pub uid_saved: uid_t,
    /// Filesystem user ID.
    pub uid_fs: uid_t,
    /// Real group ID.
    pub gid_real: gid_t,
    /// Effective group ID.
    pub gid_effective: gid_t,
    /// Saved group ID.
    pub gid_saved: gid_t,
    /// Filesystem group ID.
    pub gid_fs: gid_t,
    /// Number of file descriptor slots currently allocated.
    pub fd_allocated: u32,
    /// Supplementary group list.
    pub groups: Vec<gid_t>,
    /// Process IDs for each namespace which the process belongs to.
    pub ns_pids: Vec<pid_t>,
    /// Thread IDs for each namespace which the process belongs to.
    pub ns_tids: Vec<pid_t>,
    /// Process group IDs for each namespace which the process belongs to.
    pub ns_pgids: Vec<pid_t>,
    /// Session IDs of the process for each namespace to which it belongs.
    pub ns_sids: Vec<pid_t>,
    /// Peak virtual memory size (kB).
    pub vm_peak: usize,
    /// Virtual memory size (kB).
    pub vm_size: usize,
    /// Locked memory size (kB) (see mlock(3)).
    pub vm_locked: usize,
    /// Pinned memory size (since Linux 3.2). These are pages that can't be moved because
    /// something needs to directly access physical memory.
    pub vm_pin: usize,
    /// Peak resident size (kB) ("high water mark").
    pub vm_hwm: usize,
    /// Resident set size (kB). Comprised of `vm_rss_anon`, `vm_rss_file`,
    /// and `vm_rss_shared`.
    pub vm_rss: usize,
    /// Size of resident anonymous memory (kB) (since Linux 4.5).
    pub vm_rss_anon: usize,
    /// Size of resident file mappings (kB) (since Linux 4.5).
    pub vm_rss_file: usize,
    /// Size of resident shared memory (kB) (since Linux 4.5). Includes SysV
    /// shm, mapping of tmpfs and shared anonymous mappings.
    pub vm_rss_shared: usize,
    /// Size of data segments (kB).
    pub vm_data: usize,
    /// Size of stack segments (kB).
    pub vm_stack: usize,
    /// Size of text (executable) segments (kB).
    pub vm_exe: usize,
    /// Shared library code size (kB).
    pub vm_lib: usize,
    /// Page table entries size (since Linux 2.6.10).
    pub vm_pte: usize,
    /// Size of second-level page tables (since Linux 4.0).
    pub vm_pmd: usize,
    /// Swapped-out-virtual memory size (since Linux 2.6.34).
    pub vm_swap: usize,
    /// Size of hugetlb memory portions (since Linux 4.4).
    pub hugetlb_pages: usize,
    /// Number of threads in process containing this thread.
    pub threads: u32,
    /// The number of currently queued signals for this real user ID
    /// (see the description of RLIMIT_SIGPENDING in getrlimit(2)).
    pub sig_queued: u64,
    /// The resource limit on the number of queued signals for this process.
    pub sig_queued_max: u64,
    /// Number of signals pending for the thread (see pthreads(7)).
    pub sig_pending_thread: u64,
    /// Number of signals pending for the process (see signal(7)).
    pub sig_pending_process: u64,
    /// Mask indicating signals being blocked.
    pub sig_blocked: u64,
    /// Mask indicating signals being ignored.
    pub sig_ignored: u64,
    /// Mask indicating signals being caught.
    pub sig_caught: u64,
    /// Mask of capabilities enabled in inheritable sets (see capabilities(7)).
    pub cap_inherited: u64,
    /// Mask of capabilities enabled in permitted sets.
    pub cap_permitted: u64,
    /// Mask of capabilities enabled in effective sets.
    pub cap_effective: u64,
    /// Capability Bounding set (since Linux 2.6.26).
    pub cap_bounding: u64,
    /// Ambient capability set (since Linux 4.3).
    pub cap_ambient: u64,
    /// Whether the process can acquire new privileges (since Linux 4.10)
    pub no_new_privs: bool,
    /// Secure Computing mode of the process (since Linux 3.8, see seccomp(2)).
    /// This field is provided only if the kernel was built with the
    /// `CONFIG_SECCOMP` kernel configuration option enabled.
    pub seccomp: SeccompMode,
    /// CPUs on which this process may run (since Linux 2.6.24, see cpuset(7)).
    ///
    /// The slice represents a bitmask in the same format as `BitVec`.
    pub cpus_allowed: Box<[u8]>,
    /// Memory nodes allowed to this process (since Linux 2.6.24, see cpuset(7)).
    ///
    /// The slice represents a bitmask in the same format as `BitVec`.
    pub mems_allowed: Box<[u8]>,
    /// Number of voluntary context switches.
    pub voluntary_ctxt_switches: u64,
    /// Number of involuntary context switches.
    pub nonvoluntary_ctxt_switches: u64,
}

/// Parse the status state format.
named!(parse_status_state<State>,
       alt!(tag!("R (running)") => { |_| State::Running  }
          | tag!("S (sleeping)") => { |_| State::Sleeping }
          | tag!("D (disk sleep)") => { |_| State::Waiting }
          | tag!("T (stopped)") => { |_| State::Stopped }
          | tag!("t (tracing stop)") => { |_| State::TraceStopped }
          | tag!("X (dead)") => { |_| State::Dead }
          | tag!("Z (zombie)") => { |_| State::Zombie }));

named!(parse_command<String>,   delimited!(tag!("Name:\t"),      parse_line,         line_ending));
named!(parse_umask<mode_t>,     delimited!(tag!("Umask:\t"),     parse_u32_octal,    line_ending));
named!(parse_state<State>,      delimited!(tag!("State:\t"),     parse_status_state, line_ending));
named!(parse_pid<pid_t>,        delimited!(tag!("Tgid:\t"),      parse_i32,          line_ending));
named!(parse_numa_gid<pid_t>,   delimited!(tag!("Ngid:\t"),      parse_i32,          line_ending));
named!(parse_tid<pid_t>,        delimited!(tag!("Pid:\t"),       parse_i32,          line_ending));
named!(parse_ppid<pid_t>,       delimited!(tag!("PPid:\t"),      parse_i32,          line_ending));
named!(parse_tracer_pid<pid_t>, delimited!(tag!("TracerPid:\t"), parse_i32,          line_ending));

named!(parse_uid<(uid_t, uid_t, uid_t, uid_t)>, chain!(tag!("Uid:\t") ~ real: parse_u32 ~ space ~ effective: parse_u32
                                                                      ~ space ~ saved: parse_u32 ~ space ~ fs: parse_u32 ~ line_ending,
                                                                   || { (real, effective, saved, fs) }));
named!(parse_gid<(gid_t, gid_t, gid_t, gid_t)>, chain!(tag!("Gid:\t") ~ real: parse_u32 ~ space ~ effective: parse_u32
                                                                      ~ space ~ saved: parse_u32 ~ space ~ fs: parse_u32 ~ line_ending,
                                                                   || { (real, effective, saved, fs) }));

named!(parse_fd_allocated<u32>,   delimited!(tag!("FDSize:\t"), parse_u32,  line_ending));
named!(parse_groups<Vec<gid_t> >, delimited!(tag!("Groups:\t"), parse_u32s, multispace));

named!(parse_ns_pids<Vec<pid_t> >,  delimited!(tag!("NStgid:\t"), parse_i32s, line_ending));
named!(parse_ns_tids<Vec<pid_t> >,  delimited!(tag!("NSpid:\t"),  parse_i32s, line_ending));
named!(parse_ns_pgids<Vec<pid_t> >, delimited!(tag!("NSpgid:\t"), parse_i32s, line_ending));
named!(parse_ns_sids<Vec<pid_t> >,  delimited!(tag!("NSsid:\t"),  parse_i32s, line_ending));

named!(parse_vm_peak<usize>,        delimited!(tag!("VmPeak:"),       parse_kb, line_ending));
named!(parse_vm_size<usize>,        delimited!(tag!("VmSize:"),       parse_kb, line_ending));
named!(parse_vm_locked<usize>,      delimited!(tag!("VmLck:"),        parse_kb, line_ending));
named!(parse_vm_pin<usize>,         delimited!(tag!("VmPin:"),        parse_kb, line_ending));
named!(parse_vm_hwm<usize>,         delimited!(tag!("VmHWM:"),        parse_kb, line_ending));
named!(parse_vm_rss<usize>,         delimited!(tag!("VmRSS:"),        parse_kb, line_ending));
named!(parse_vm_rss_anon<usize>,    delimited!(tag!("RssAnon:"),      parse_kb, line_ending));
named!(parse_vm_rss_file<usize>,    delimited!(tag!("RssFile:"),      parse_kb, line_ending));
named!(parse_vm_rss_shared<usize>,  delimited!(tag!("RssShmem:"),     parse_kb, line_ending));
named!(parse_vm_data<usize>,        delimited!(tag!("VmData:"),       parse_kb, line_ending));
named!(parse_vm_stack<usize>,       delimited!(tag!("VmStk:"),        parse_kb, line_ending));
named!(parse_vm_exe<usize>,         delimited!(tag!("VmExe:"),        parse_kb, line_ending));
named!(parse_vm_lib<usize>,         delimited!(tag!("VmLib:"),        parse_kb, line_ending));
named!(parse_vm_pte<usize>,         delimited!(tag!("VmPTE:"),        parse_kb, line_ending));
named!(parse_vm_pmd<usize>,         delimited!(tag!("VmPMD:"),        parse_kb, line_ending));
named!(parse_vm_swap<usize>,        delimited!(tag!("VmSwap:"),       parse_kb, line_ending));
named!(parse_hugetlb_pages<usize>,  delimited!(tag!("HugetlbPages:"), parse_kb, line_ending));

named!(parse_threads<u32>, delimited!(tag!("Threads:\t"), parse_u32, line_ending));

named!(parse_sig_queued<(u64, u64)>, delimited!(tag!("SigQ:\t"), separated_pair!(parse_u64, tag!("/"), parse_u64), line_ending));

named!(parse_sig_pending_thread<u64>,  delimited!(tag!("SigPnd:\t"), parse_u64_hex, line_ending));
named!(parse_sig_pending_process<u64>, delimited!(tag!("ShdPnd:\t"), parse_u64_hex, line_ending));
named!(parse_sig_blocked<u64>,         delimited!(tag!("SigBlk:\t"), parse_u64_hex, line_ending));
named!(parse_sig_ignored<u64>,         delimited!(tag!("SigIgn:\t"), parse_u64_hex, line_ending));
named!(parse_sig_caught<u64>,          delimited!(tag!("SigCgt:\t"), parse_u64_hex, line_ending));

named!(parse_cap_inherited<u64>, delimited!(tag!("CapInh:\t"), parse_u64_hex, line_ending));
named!(parse_cap_permitted<u64>, delimited!(tag!("CapPrm:\t"), parse_u64_hex, line_ending));
named!(parse_cap_effective<u64>, delimited!(tag!("CapEff:\t"), parse_u64_hex, line_ending));
named!(parse_cap_bounding<u64>,  delimited!(tag!("CapBnd:\t"), parse_u64_hex, line_ending));
named!(parse_cap_ambient<u64>,  delimited!(tag!("CapAmb:\t"), parse_u64_hex, line_ending));

named!(parse_no_new_privs<bool>,       delimited!(tag!("NoNewPrivs:\t"),   parse_bit,           line_ending));
named!(parse_seccomp<SeccompMode>,     delimited!(tag!("Seccomp:\t"),      parse_seccomp_mode,  line_ending));
named!(parse_cpus_allowed<Box<[u8]> >, delimited!(tag!("Cpus_allowed:\t"), parse_u32_mask_list, line_ending));
named!(parse_mems_allowed<Box<[u8]> >, delimited!(tag!("Mems_allowed:\t"), parse_u32_mask_list, line_ending));

named!(parse_cpus_allowed_list<()>, chain!(tag!("Cpus_allowed_list:\t") ~ not_line_ending ~ line_ending, || { () }));
named!(parse_mems_allowed_list<()>, chain!(tag!("Mems_allowed_list:\t") ~ not_line_ending ~ line_ending, || { () }));

named!(parse_voluntary_ctxt_switches<u64>,    delimited!(tag!("voluntary_ctxt_switches:\t"),    parse_u64, line_ending));
named!(parse_nonvoluntary_ctxt_switches<u64>, delimited!(tag!("nonvoluntary_ctxt_switches:\t"), parse_u64, line_ending));

/// Parse the status format.
fn parse_status(i: &[u8]) -> IResult<&[u8], Status> {
    let mut status: Status = Default::default();
    map!(i,
        many0!( // TODO: use a loop here instead of many0 to avoid allocating a vec.
            alt!(parse_command      => { |value| status.command     = value }
               | parse_umask        => { |value| status.umask       = value }
               | parse_state        => { |value| status.state       = value }
               | parse_pid          => { |value| status.pid         = value }
               | parse_numa_gid     => { |value| status.numa_gid    = value }
               | parse_tid          => { |value| status.tid         = value }
               | parse_ppid         => { |value| status.ppid        = value }
               | parse_tracer_pid   => { |value| status.tracer_pid  = value }
               | parse_uid => { |(real, effective, saved, fs)| { status.uid_real = real;
                                                                 status.uid_effective = effective;
                                                                 status.uid_saved = saved;
                                                                 status.uid_fs = fs; } }
               | parse_gid => { |(real, effective, saved, fs)| { status.gid_real = real;
                                                                 status.gid_effective = effective;
                                                                 status.gid_saved = saved;
                                                                 status.gid_fs = fs; } }
               | parse_fd_allocated      => { |value| status.fd_allocated   = value }
               | parse_groups            => { |value| status.groups         = value }
               | parse_ns_pids           => { |value| status.ns_pids        = value }
               | parse_ns_tids           => { |value| status.ns_tids        = value }
               | parse_ns_pgids          => { |value| status.ns_pgids       = value }
               | parse_ns_sids           => { |value| status.ns_sids        = value }
               | parse_vm_peak           => { |value| status.vm_peak        = value }
               | parse_vm_size           => { |value| status.vm_size        = value }
               | parse_vm_locked         => { |value| status.vm_locked      = value }
               | parse_vm_pin            => { |value| status.vm_pin         = value }
               | parse_vm_hwm            => { |value| status.vm_hwm         = value }
               | parse_vm_rss            => { |value| status.vm_rss         = value }
               | parse_vm_rss_anon       => { |value| status.vm_rss_anon    = value }
               | parse_vm_rss_file       => { |value| status.vm_rss_file    = value }
               | parse_vm_rss_shared     => { |value| status.vm_rss_shared  = value }
               | parse_vm_data           => { |value| status.vm_data        = value }
               | parse_vm_stack          => { |value| status.vm_stack       = value }
               | parse_vm_exe            => { |value| status.vm_exe         = value }
               | parse_vm_lib            => { |value| status.vm_lib         = value }
               | parse_vm_pte            => { |value| status.vm_pte         = value }
               | parse_vm_pmd            => { |value| status.vm_pmd         = value }
               | parse_vm_swap           => { |value| status.vm_swap        = value }
               | parse_hugetlb_pages     => { |value| status.hugetlb_pages  = value }

               | parse_threads              => { |value| status.threads                 = value }
               | parse_sig_queued           => { |(count, max)| { status.sig_queued     = count;
                                                                  status.sig_queued_max = max } }
               | parse_sig_pending_thread   => { |value| status.sig_pending_thread      = value }
               | parse_sig_pending_process  => { |value| status.sig_pending_process     = value }
               | parse_sig_blocked          => { |value| status.sig_blocked             = value }
               | parse_sig_ignored          => { |value| status.sig_ignored             = value }
               | parse_sig_caught           => { |value| status.sig_caught              = value }

               | parse_cap_inherited => { |value| status.cap_inherited = value }
               | parse_cap_permitted => { |value| status.cap_permitted = value }
               | parse_cap_effective => { |value| status.cap_effective = value }
               | parse_cap_bounding  => { |value| status.cap_bounding  = value }
               | parse_cap_ambient   => { |value| status.cap_ambient   = value }

               | parse_no_new_privs  => { |value| status.no_new_privs  = value }
               | parse_seccomp       => { |value| status.seccomp       = value }
               | parse_cpus_allowed  => { |value| status.cpus_allowed  = value }
               | parse_cpus_allowed_list
               | parse_mems_allowed  => { |value| status.mems_allowed  = value }
               | parse_mems_allowed_list
               | parse_voluntary_ctxt_switches    => { |value| status.voluntary_ctxt_switches    = value }
               | parse_nonvoluntary_ctxt_switches => { |value| status.nonvoluntary_ctxt_switches = value }
            )
        ),
        { |_| { status }})
}

/// Parses the provided status file.
fn status_file(file: &mut File) -> Result<Status> {
    let mut buf = [0; 2048]; // A typical status file is about 1000 bytes
    map_result(parse_status(try!(read_to_end(file, &mut buf))))
}

/// Returns memory status information for the process with the provided pid.
pub fn status(pid: pid_t) -> Result<Status> {
    status_file(&mut try!(File::open(&format!("/proc/{}/status", pid))))
}

/// Returns memory status information for the current process.
pub fn status_self() -> Result<Status> {
    status_file(&mut try!(File::open("/proc/self/status")))
}

#[cfg(test)]
mod tests {
    use parsers::tests::unwrap;
    use super::{SeccompMode, parse_status, status, status_self};
    use pid::State;

    /// Test that the system status files can be parsed.
    #[test]
    fn test_status() {
        status_self().unwrap();
        status(1).unwrap();
    }

    #[test]
    fn test_parse_status() {
        let status_text = b"Name:\tsystemd\n\
                            Umask:\t0022\n\
                            State:\tS (sleeping)\n\
                            Tgid:\t1\n\
                            Ngid:\t0\n\
                            Pid:\t1\n\
                            PPid:\t0\n\
                            TracerPid:\t0\n\
                            Uid:\t0\t0\t0\t0\n\
                            Gid:\t0\t0\t0\t0\n\
                            FDSize:\t64\n\
                            Groups:\t10\t1000\n\
                            NStgid:\t1\n\
                            NSpid:\t1\n\
                            NSpgid:\t1\n\
                            NSsid:\t1\n\
                            VmPeak:\t10927688 kB\n\
                            VmSize:\t   47348 kB\n\
                            VmLck:\t       0 kB\n\
                            VmPin:\t       0 kB\n\
                            VmHWM:\t    9212 kB\n\
                            VmRSS:\t    9212 kB\n\
                            RssAnon:\t            3700 kB\n\
                            RssFile:\t            5768 kB\n\
                            RssShmem:\t              0 kB\n\
                            VmData:\t   3424 kB\n\
                            VmStk:\t     136 kB\n\
                            VmExe:\t    1320 kB\n\
                            VmLib:\t    3848 kB\n\
                            VmPTE:\t     108 kB\n\
                            VmPMD:\t      12 kB\n\
                            VmSwap:\t      0 kB\n\
                            HugetlbPages:\t          0 kB\n\
                            Threads:\t1\n\
                            SigQ:\t0/257232\n\
                            SigPnd:\t0000000000000000\n\
                            ShdPnd:\t0000000000000000\n\
                            SigBlk:\t7be3c0fe28014a03\n\
                            SigIgn:\t0000000000001000\n\
                            SigCgt:\t00000001800004ec\n\
                            CapInh:\t0000000000000000\n\
                            CapPrm:\t0000003fffffffff\n\
                            CapEff:\t0000003fffffffff\n\
                            CapBnd:\t0000003fffffffff\n\
                            CapAmb:\t0000000000000000\n\
                            NoNewPrivs:\t0\n\
                            Seccomp:\t0\n\
                            Cpus_allowed:\tffff\n\
                            Cpus_allowed_list:\t0-15\n\
                            Mems_allowed:\t00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000000,00000001\n\
                            Mems_allowed_list:\t0\n\
                            voluntary_ctxt_switches:\t242129\n\
                            nonvoluntary_ctxt_switches:\t1748\n";

        let status = unwrap(parse_status(status_text));
        assert_eq!("systemd", status.command);
        assert_eq!(18, status.umask);
        assert_eq!(State::Sleeping, status.state);
        assert_eq!(1, status.pid);
        assert_eq!(0, status.numa_gid);
        assert_eq!(1, status.tid);
        assert_eq!(0, status.ppid);
        assert_eq!(0, status.tracer_pid);
        assert_eq!(0, status.uid_real);
        assert_eq!(0, status.uid_effective);
        assert_eq!(0, status.uid_saved);
        assert_eq!(0, status.uid_fs);
        assert_eq!(0, status.gid_real);
        assert_eq!(0, status.gid_effective);
        assert_eq!(0, status.gid_saved);
        assert_eq!(0, status.gid_fs);
        assert_eq!(64, status.fd_allocated);
        assert_eq!(vec![10, 1000], status.groups);
        assert_eq!(vec![1], status.ns_pids);
        assert_eq!(vec![1], status.ns_tids);
        assert_eq!(vec![1], status.ns_pgids);
        assert_eq!(vec![1], status.ns_sids);
        assert_eq!(10927688, status.vm_peak);
        assert_eq!(47348, status.vm_size);
        assert_eq!(0, status.vm_locked);
        assert_eq!(0, status.vm_pin);
        assert_eq!(9212, status.vm_hwm);
        assert_eq!(9212, status.vm_rss);
        assert_eq!(3700, status.vm_rss_anon);
        assert_eq!(5768, status.vm_rss_file);
        assert_eq!(0, status.vm_rss_shared);
        assert_eq!(3424, status.vm_data);
        assert_eq!(136, status.vm_stack);
        assert_eq!(1320, status.vm_exe);
        assert_eq!(3848, status.vm_lib);
        assert_eq!(108, status.vm_pte);
        assert_eq!(12, status.vm_pmd);
        assert_eq!(0, status.vm_swap);
        assert_eq!(0, status.hugetlb_pages);
        assert_eq!(1, status.threads);
        assert_eq!(0, status.sig_queued);
        assert_eq!(257232, status.sig_queued_max);
        assert_eq!(0x0000000000000000, status.sig_pending_thread);
        assert_eq!(0x0000000000000000, status.sig_pending_process);
        assert_eq!(0x7be3c0fe28014a03, status.sig_blocked);
        assert_eq!(0x0000000000001000, status.sig_ignored);
        assert_eq!(0x00000001800004ec, status.sig_caught);
        assert_eq!(0x0000000000000000, status.cap_inherited);
        assert_eq!(0x0000003fffffffff, status.cap_permitted);
        assert_eq!(0x0000003fffffffff, status.cap_effective);
        assert_eq!(0x0000003fffffffff, status.cap_bounding);
        assert_eq!(0x0000000000000000, status.cap_ambient);
        assert_eq!(false, status.no_new_privs);
        assert_eq!(SeccompMode::Disabled, status.seccomp);
        assert_eq!(&[0xff, 0xff, 0x00, 0x00], &*status.cpus_allowed);
        let mems_allowed: &mut [u8] = &mut [0; 64];
        mems_allowed[0] = 0x80;
        assert_eq!(mems_allowed, &*status.mems_allowed);
        assert_eq!(242129, status.voluntary_ctxt_switches);
        assert_eq!(1748, status.nonvoluntary_ctxt_switches);
    }
}

#[cfg(all(test, rustc_nightly))]
mod benches {
    extern crate test;

    use std::fs::File;

    use parsers::read_to_end;
    use super::{parse_status, status};

    #[bench]
    fn bench_status(b: &mut test::Bencher) {
        b.iter(|| test::black_box(status(1)));
    }

    #[bench]
    fn bench_status_parse(b: &mut test::Bencher) {
        let mut buf = [0; 2048];
        let status = read_to_end(&mut File::open("/proc/1/status").unwrap(), &mut buf).unwrap();
        b.iter(|| test::black_box(parse_status(status)));
    }
}
