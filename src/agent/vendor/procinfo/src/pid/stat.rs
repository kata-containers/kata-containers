//! Process status information from `/proc/[pid]/stat`.

use std::fs::File;
use std::io::Result;
use std::str::{self, FromStr};

use libc::{clock_t, pid_t};
use nom::{self, IResult, line_ending, space};
use pid::State;

use parsers::{
    map_result,
    parse_clock,
    parse_i32,
    parse_u32,
    parse_u64,
    parse_usize,
    read_to_end
};

/// Process status information.
///
/// See `man 5 proc` and `Linux/fs/proc/array.c`.
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct Stat {
    /// Process ID (i.e., Thread Group ID).
    pub pid: pid_t,
    /// Filename of the executable.
    pub command: String,
    /// Current state of the process.
    pub state: State,
    /// Process ID of parent process.
    pub ppid: pid_t,
    /// Process group ID of the process.
    pub pgrp: pid_t,
    /// Session ID of the process.
    pub session: pid_t,
    /// The controlling terminal of the process. (The minor device number is contained in the
    /// combination of bits 31 to 20 and 7 to 0; the major device number is in bits 15 to 8.)
    pub tty_nr: pid_t,
    /// Process group ID of the controlling terminal of the process.
    pub tty_pgrp: pid_t,
    /// The kernel flags word of the process. For bit meanings, see the `PF_*` defines in the Linux
    /// kernel source file `include/linux/sched.h`. Details depend on the kernel version.
    pub flags: u32,
    /// The number of minor faults the process has made which have not required loading a memory
    /// page from disk.
    pub minflt: usize,
    /// The number of minor faults that the process's waited-for children have made.
    pub cminflt: usize,
    /// The number of major faults the process has made which have required loading a memory page
    /// from disk.
    pub majflt: usize,
    /// The number of major faults that the process's waited-for children have made.
    pub cmajflt: usize,
    /// Amount of time that this process has been scheduled in user mode, measured in clock ticks
    /// (divide by `sysconf(_SC_CLK_TCK)`). This includes guest time, `guest_time` (time spent
    /// running a virtual CPU, see below), so that applications that are not aware of the guest
    /// time field do not lose that time from their calculations.
    pub utime: clock_t,
    /// Amount of time that this process has been scheduled in kernel mode, measured in clock ticks
    /// (divide by `sysconf(_SC_CLK_TCK)`).
    pub stime: clock_t,
    /// Amount of time that this process's waited-for children have been scheduled in user mode,
    /// measured in clock ticks (divide by `sysconf(_SC_CLK_TCK)`). (See also `times(2)`.)  This
    /// includes guest time, `cguest_time` (time spent running a virtual CPU, see below).
    pub cutime: clock_t,
    /// Amount of time that this process's waited-for children have been scheduled in kernel mode,
    /// measured in clock ticks (divide by `sysconf(_SC_CLK_TCK)`).
    pub cstime: clock_t,
    /// For processes running a real-time scheduling policy (policy below; see
    /// `sched_setscheduler(2)`), this is the negated scheduling priority, minus one; that is, a
    /// number in the range -2 to -100, corresponding to real-time priorities 1 to 99. For
    /// processes running under a non-real-time scheduling policy, this is the raw nice value
    /// (`setpriority(2)`) as represented in the kernel. The kernel stores nice values as numbers
    /// in the range 0 (high) to 39 (low), corresponding to the user-visible nice range of -20 to
    /// 19.
    pub priority: i32,
    /// The nice value (see `setpriority(2)`), a value in the range 19 (low priority) to -20 (high
    /// priority).
    pub nice: i32,
    /// Number of threads in this process (since Linux 2.6).
    pub num_threads: i32,
    /// The time the process started after system boot, expressed in clock ticks (divide by
    /// `sysconf(_SC_CLK_TCK)`).
    pub start_time: u64,
    /// Virtual memory size in bytes.
    pub vsize: usize,
    /// Resident Set Size: number of pages the process has in real memory. This is just the pages
    /// which count toward text, data, or stack space. This does not include pages which have not
    /// been demand-loaded in, or which are swapped out.
    pub rss: usize,
    /// Current soft limit in bytes on the rss of the process; see the description of `RLIMIT_RSS`
    /// in `getrlimit(2)`.
    pub rsslim: usize,
    /// The address above which program text can run.
    pub start_code: usize,
    /// The address below which program text can run.
    pub end_code: usize,
    /// The address of the start (i.e., bottom) of the stack.
    pub startstack: usize,
    /// The current value of ESP (stack pointer), as found in the kernel stack page for the process.
    pub kstkeep: usize,
    /// The current EIP (instruction pointer).
    pub kstkeip: usize,
    /// The bitmap of pending signals. Obsolete, because it does not provide information on
    /// real-time signals; use `/proc/[pid]/status` instead.
    pub signal: usize,
    /// The bitmap of blocked signals. Obsolete, because it does not provide information on
    /// real-time signals; use `/proc/[pid]/status` instead.
    pub blocked: usize,
    /// The bitmap of ignored signals. Obsolete, because it does not provide information on
    /// real-time signals; use `/proc/[pid]/status` instead.
    pub sigignore: usize,
    /// The bitmap of caught signals. Obsolete, because it does not provide information on
    /// real-time signals; use /proc/[pid]/status instead.
    pub sigcatch: usize,
    /// This is the "channel" in which the process is waiting. It is the address of a location in
    /// the kernel where the process is sleeping. The corresponding symbolic name can be found in
    /// `/proc/[pid]/wchan`.
    pub wchan: usize,
    /// Signal to be sent to parent when we die.
    pub exit_signal: i32,
    /// CPU number last executed on.
    pub processor: u32,
    /// Real-time scheduling priority, a number in the range 1 to 99 for processes scheduled under
    /// a real-time policy, or 0, for non-real-time processes (see `sched_setscheduler(2)`).
    pub rt_priority: u32,
    /// Scheduling policy (see `sched_setscheduler(2)`). Decode using the `SCHED_*` constants in
    /// `linux/sched.h`.
    pub policy: u32,
    /// Aggregated block I/O delays, measured in clock ticks (centiseconds). Since Linux 2.6.18.
    pub delayacct_blkio_ticks: u64,
    /// Guest time of the process (time spent running a virtual CPU for a guest operating system),
    /// measured in clock ticks (divide by `sysconf(_SC_CLK_TCK)`). Since Linux 2.6.24.
    pub guest_time: clock_t,
    /// Guest time of the process's children, measured in clock ticks (divide by
    /// `sysconf(_SC_CLK_TCK)`). Since linux 2.6.24.
    pub cguest_time: clock_t,
    /// Address above which program initialized and uninitialized (BSS) data are placed. Since
    /// Linux 3.3.
    pub start_data: usize,
    /// Address below which program initialized and uninitialized (BSS) data are placed. Since
    /// Linux 3.3.
    pub end_data: usize,
    /// Address above which program heap can be expanded with `brk(2)`. Since Linux 3.3.
    pub start_brk: usize,
    /// Address above which program command-line arguments (argv) are placed. Since Linux 3.5.
    pub arg_start: usize,
    /// Address below program command-line arguments (argv) are placed. Since Linux 3.5.
    pub arg_end: usize,
    /// Address above which program environment is placed. Since Linux 3.5.
    pub env_start: usize,
    /// Address below which program environment is placed. Since Linux 3.5.
    pub env_end: usize,
    /// The thread's exit status in the form reported by `waitpid(2)`. Since Linux 3.5.
    pub exit_code: i32,
}

named!(parse_command<String>,
       map_res!(map_res!(preceded!(char!('('),
                                   take_until_right_and_consume!(")")),
                         str::from_utf8),
                FromStr::from_str));

/// Parse the stat state format.
named!(parse_stat_state<State>,
       alt!(tag!("R") => { |_| State::Running  }
          | tag!("S") => { |_| State::Sleeping }
          | tag!("D") => { |_| State::Waiting }
          | tag!("Z") => { |_| State::Zombie }
          | tag!("T") => { |_| State::Stopped }
          | tag!("t") => { |_| State::TraceStopped }
          | tag!("W") => { |_| State::Paging }
          | tag!("X") => { |_| State::Dead }
          | tag!("x") => { |_| State::Dead }
          | tag!("K") => { |_| State::Wakekill }
          | tag!("W") => { |_| State::Waking }
          | tag!("P") => { |_| State::Parked }));

// Note: this is implemented as a function insted of via `chain!` to reduce the
// stack depth in rustc by limiting the generated AST's depth. This is a work
// around for
//   https://github.com/rust-lang/rust/issues/35408
// where rustc overflows its stack. The bug affects at least rustc 1.12.
fn parse_stat(input: &[u8]) -> IResult<&[u8], Stat> {
    /// Helper macro for space terminated parser.
    macro_rules! s {
        ($i:expr, $f:expr) => (terminated!($i, call!($f), space))
    }
    /// Helper macro for line-ending terminated parser.
    macro_rules! l {
        ($i:expr, $f:expr) => (terminated!($i, call!($f), line_ending))
    }

    let rest = input;

    let (rest, pid)                   = try_parse!(rest, s!(parse_i32        ));
    let (rest, command)               = try_parse!(rest, s!(parse_command    ));
    let (rest, state)                 = try_parse!(rest, s!(parse_stat_state ));
    let (rest, ppid)                  = try_parse!(rest, s!(parse_i32        ));
    let (rest, pgrp)                  = try_parse!(rest, s!(parse_i32        ));
    let (rest, session)               = try_parse!(rest, s!(parse_i32        ));
    let (rest, tty_nr)                = try_parse!(rest, s!(parse_i32        ));
    let (rest, tty_pgrp)              = try_parse!(rest, s!(parse_i32        ));
    let (rest, flags)                 = try_parse!(rest, s!(parse_u32        ));
    let (rest, minflt)                = try_parse!(rest, s!(parse_usize      ));
    let (rest, cminflt)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, majflt)                = try_parse!(rest, s!(parse_usize      ));
    let (rest, cmajflt)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, utime)                 = try_parse!(rest, s!(parse_clock      ));
    let (rest, stime)                 = try_parse!(rest, s!(parse_clock      ));
    let (rest, cutime)                = try_parse!(rest, s!(parse_clock      ));
    let (rest, cstime)                = try_parse!(rest, s!(parse_clock      ));
    let (rest, priority)              = try_parse!(rest, s!(parse_i32        ));
    let (rest, nice)                  = try_parse!(rest, s!(parse_i32        ));
    let (rest, num_threads)           = try_parse!(rest, s!(parse_i32        ));
    let (rest, _itrealvalue)          = try_parse!(rest, s!(parse_i32        ));
    let (rest, start_time)            = try_parse!(rest, s!(parse_u64        ));
    let (rest, vsize)                 = try_parse!(rest, s!(parse_usize      ));
    let (rest, rss)                   = try_parse!(rest, s!(parse_usize      ));
    let (rest, rsslim)                = try_parse!(rest, s!(parse_usize      ));
    let (rest, start_code)            = try_parse!(rest, s!(parse_usize      ));
    let (rest, end_code)              = try_parse!(rest, s!(parse_usize      ));
    let (rest, startstack)            = try_parse!(rest, s!(parse_usize      ));
    let (rest, kstkeep)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, kstkeip)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, signal)                = try_parse!(rest, s!(parse_usize      ));
    let (rest, blocked)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, sigignore)             = try_parse!(rest, s!(parse_usize      ));
    let (rest, sigcatch)              = try_parse!(rest, s!(parse_usize      ));
    let (rest, wchan)                 = try_parse!(rest, s!(parse_usize      ));
    let (rest, _nswap)                = try_parse!(rest, s!(parse_usize      ));
    let (rest, _cnswap)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, exit_signal)           = try_parse!(rest, s!(parse_i32        ));
    let (rest, processor)             = try_parse!(rest, s!(parse_u32        ));
    let (rest, rt_priority)           = try_parse!(rest, s!(parse_u32        ));
    let (rest, policy)                = try_parse!(rest, s!(parse_u32        ));
    let (rest, delayacct_blkio_ticks) = try_parse!(rest, s!(parse_u64        ));
    let (rest, guest_time)            = try_parse!(rest, s!(parse_clock      ));
    let (rest, cguest_time)           = try_parse!(rest, s!(parse_clock      ));
    let (rest, start_data)            = try_parse!(rest, s!(parse_usize      ));
    let (rest, end_data)              = try_parse!(rest, s!(parse_usize      ));
    let (rest, start_brk)             = try_parse!(rest, s!(parse_usize      ));
    let (rest, arg_start)             = try_parse!(rest, s!(parse_usize      ));
    let (rest, arg_end)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, env_start)             = try_parse!(rest, s!(parse_usize      ));
    let (rest, env_end)               = try_parse!(rest, s!(parse_usize      ));
    let (rest, exit_code)             = try_parse!(rest, l!(parse_i32        ));

    IResult::Done(rest, Stat {
        pid                   : pid,
        command               : command,
        state                 : state,
        ppid                  : ppid,
        pgrp                  : pgrp,
        session               : session,
        tty_nr                : tty_nr,
        tty_pgrp              : tty_pgrp,
        flags                 : flags,
        minflt                : minflt,
        cminflt               : cminflt,
        majflt                : majflt,
        cmajflt               : cmajflt,
        utime                 : utime,
        stime                 : stime,
        cutime                : cutime,
        cstime                : cstime,
        priority              : priority,
        nice                  : nice,
        num_threads           : num_threads,
        start_time            : start_time,
        vsize                 : vsize,
        rss                   : rss,
        rsslim                : rsslim,
        start_code            : start_code,
        end_code              : end_code,
        startstack            : startstack,
        kstkeep               : kstkeep,
        kstkeip               : kstkeip,
        signal                : signal,
        blocked               : blocked,
        sigignore             : sigignore,
        sigcatch              : sigcatch,
        wchan                 : wchan,
        exit_signal           : exit_signal,
        processor             : processor,
        rt_priority           : rt_priority,
        policy                : policy,
        delayacct_blkio_ticks : delayacct_blkio_ticks,
        guest_time            : guest_time,
        cguest_time           : cguest_time,
        start_data            : start_data,
        end_data              : end_data,
        start_brk             : start_brk,
        arg_start             : arg_start,
        arg_end               : arg_end,
        env_start             : env_start,
        env_end               : env_end,
        exit_code             : exit_code,
    })
}

/// Parses the provided stat file.
fn stat_file(file: &mut File) -> Result<Stat> {
    let mut buf = [0; 1024]; // A typical statm file is about 300 bytes
    map_result(parse_stat(try!(read_to_end(file, &mut buf))))
}

/// Returns status information for the process with the provided pid.
pub fn stat(pid: pid_t) -> Result<Stat> {
    stat_file(&mut try!(File::open(&format!("/proc/{}/stat", pid))))
}

/// Returns status information for the current process.
pub fn stat_self() -> Result<Stat> {
    stat_file(&mut try!(File::open("/proc/self/stat")))
}

#[cfg(test)]
pub mod tests {
    use parsers::tests::unwrap;
    use pid::State;
    use super::{
        parse_command,
        parse_stat,
        stat,
        stat_self
    };

    #[test]
    fn test_parse_command() {
        assert_eq!("cat", &unwrap(parse_command(b"(cat)")));
        assert_eq!("cat )  (( )) ", &unwrap(parse_command(b"(cat )  (( )) )")));
    }

    /// Test that the system stat files can be parsed.
    #[test]
    fn test_stat() {
        stat_self().unwrap();
        stat(1).unwrap();
    }

    #[test]
    fn test_parse_stat() {
        let text = b"19853 (cat) R 19435 19853 19435 34819 19853 4218880 98 0 0 0 0 0 0 0 20 0 1 0 \
                     279674171 112295936 180 18446744073709551615 4194304 4238772 140736513999744 \
                     140736513999080 139957028908944 0 0 0 0 0 0 0 17 15 0 0 0 0 0 6339648 6341408 \
                     17817600 140736514006312 140736514006332 140736514006332 140736514007019 0\n";
        let stat = unwrap(parse_stat(text));

        assert_eq!(19853, stat.pid);
        assert_eq!("cat", &stat.command);
        assert_eq!(State::Running, stat.state);
        assert_eq!(19435, stat.ppid);
        assert_eq!(19853, stat.pgrp);
        assert_eq!(19435, stat.session);
        assert_eq!(34819, stat.tty_nr);
        assert_eq!(19853, stat.tty_pgrp);
        assert_eq!(4218880, stat.flags);
        assert_eq!(98, stat.minflt);
        assert_eq!(0, stat.cminflt);
        assert_eq!(0, stat.majflt);
        assert_eq!(0, stat.cmajflt);
        assert_eq!(0, stat.utime);
        assert_eq!(0, stat.stime);
        assert_eq!(0, stat.cutime);
        assert_eq!(0, stat.cstime);
        assert_eq!(20, stat.priority);
        assert_eq!(0, stat.nice);
        assert_eq!(1, stat.num_threads);
        assert_eq!(279674171, stat.start_time);
        assert_eq!(112295936, stat.vsize);
        assert_eq!(180, stat.rss);
        assert_eq!(18446744073709551615, stat.rsslim);
        assert_eq!(4194304, stat.start_code);
        assert_eq!(4238772, stat.end_code);
        assert_eq!(140736513999744, stat.startstack);
        assert_eq!(140736513999080, stat.kstkeep);
        assert_eq!(139957028908944, stat.kstkeip);
        assert_eq!(0, stat.signal);
        assert_eq!(0, stat.blocked);
        assert_eq!(0, stat.sigignore);
        assert_eq!(0, stat.sigcatch);
        assert_eq!(0, stat.wchan);
        assert_eq!(17, stat.exit_signal);
        assert_eq!(15, stat.processor);
        assert_eq!(0, stat.rt_priority);
        assert_eq!(0, stat.policy);
        assert_eq!(0, stat.delayacct_blkio_ticks);
        assert_eq!(0, stat.guest_time);
        assert_eq!(0, stat.cguest_time);
        assert_eq!(6339648, stat.start_data);
        assert_eq!(6341408, stat.end_data);
        assert_eq!(17817600, stat.start_brk);
        assert_eq!(140736514006312, stat.arg_start);
        assert_eq!(140736514006332, stat.arg_end);
        assert_eq!(140736514006332, stat.env_start);
        assert_eq!(140736514007019, stat.env_end);
        assert_eq!(0, stat.exit_code);
    }
}

#[cfg(all(test, rustc_nightly))]
mod benches {
    extern crate test;

    use std::fs::File;

    use parsers::read_to_end;
    use super::{parse_stat, stat};

    #[bench]
    fn bench_stat(b: &mut test::Bencher) {
        b.iter(|| test::black_box(stat(1)));
    }

    #[bench]
    fn bench_stat_parse(b: &mut test::Bencher) {
        let mut buf = [0; 256];
        let stat = read_to_end(&mut File::open("/proc/1/stat").unwrap(), &mut buf).unwrap();
        b.iter(|| test::black_box(parse_stat(stat)));
    }
}
