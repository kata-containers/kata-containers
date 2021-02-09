//! Process resource limit information from `/proc/[pid]/limits`.

use std::fs::File;
use std::io::Result;
use std::str::{self};
use std::time::Duration;

use libc::pid_t;
use nom::{
    IResult,
    space,
};

use parsers::{
    map_result,
    parse_u64,
    parse_usize,
    read_to_end
};

fn parse_limit<'a, P, T>(input: &'a [u8], value_parser: P) -> IResult<&'a [u8], Limit<T>>
where P: Fn(&[u8]) -> IResult<&[u8], T> {
    let parse_field = closure!(&'a [u8], alt!(
         tag!("unlimited") => { |_| None }
       | value_parser => { |value| Some(value) }
    ));

    map!(input, separated_pair!(parse_field, space, parse_field),
         |(soft, hard)| Limit { soft: soft, hard: hard })
}

fn duration_from_micros(micros: u64) -> Duration {
    let micros_per_sec = 1_000_000;
    let nanos_per_micro = 1000;
    let secs = micros / micros_per_sec;
    let nanos = ((micros % micros_per_sec) as u32) * nanos_per_micro;
    Duration::new(secs, nanos)
}

named!(parse_limit_usize( &[u8] ) -> Limit<usize>, apply!(parse_limit, parse_usize));
named!(parse_limit_u64( &[u8] ) -> Limit<u64>, apply!(parse_limit, parse_u64));
named!(parse_limit_seconds( &[u8] ) -> Limit<Duration>,
       map!(apply!(parse_limit, parse_u64),
            | Limit { soft, hard } | {
                Limit {
                    soft: soft.map(Duration::from_secs),
                    hard: hard.map(Duration::from_secs),
                }
            }
       ));
named!(parse_limit_micros( &[u8] ) -> Limit<Duration>,
       map!(apply!(parse_limit, parse_u64),
            | Limit { soft, hard } | {
                Limit {
                    soft: soft.map(duration_from_micros),
                    hard: hard.map(duration_from_micros),
                }
            }
       ));

named!(parse_limits( &[u8] ) -> Limits,
    ws!(do_parse!(
        tag!("Limit") >> tag!("Soft Limit") >> tag!("Hard Limit") >> tag!("Units") >>
        tag!("Max cpu time")          >> max_cpu_time: parse_limit_seconds        >> tag!("seconds")    >>
        tag!("Max file size")         >> max_file_size: parse_limit_u64           >> tag!("bytes")      >>
        tag!("Max data size")         >> max_data_size: parse_limit_usize         >> tag!("bytes")      >>
        tag!("Max stack size")        >> max_stack_size: parse_limit_usize        >> tag!("bytes")      >>
        tag!("Max core file size")    >> max_core_file_size: parse_limit_usize    >> tag!("bytes")      >>
        tag!("Max resident set")      >> max_resident_set: parse_limit_usize      >> tag!("bytes")      >>
        tag!("Max processes")         >> max_processes: parse_limit_usize         >> tag!("processes")  >>
        tag!("Max open files")        >> max_open_files: parse_limit_usize        >> tag!("files")      >>
        tag!("Max locked memory")     >> max_locked_memory: parse_limit_usize     >> tag!("bytes")      >>
        tag!("Max address space")     >> max_address_space: parse_limit_usize     >> tag!("bytes")      >>
        tag!("Max file locks")        >> max_file_locks: parse_limit_usize        >> tag!("locks")      >>
        tag!("Max pending signals")   >> max_pending_signals: parse_limit_usize   >> tag!("signals")    >>
        tag!("Max msgqueue size")     >> max_msgqueue_size: parse_limit_usize     >> tag!("bytes")      >>
        tag!("Max nice priority")     >> max_nice_priority: parse_limit_usize     >>
        tag!("Max realtime priority") >> max_realtime_priority: parse_limit_usize >>
        tag!("Max realtime timeout")  >> max_realtime_timeout: parse_limit_micros >> tag!("us")         >>
        (Limits {
            max_cpu_time: max_cpu_time,
            max_file_size: max_file_size,
            max_data_size: max_data_size,
            max_stack_size: max_stack_size,
            max_core_file_size: max_core_file_size,
            max_resident_set: max_resident_set,
            max_processes: max_processes,
            max_open_files: max_open_files,
            max_locked_memory: max_locked_memory,
            max_address_space: max_address_space,
            max_file_locks: max_file_locks,
            max_pending_signals: max_pending_signals,
            max_msgqueue_size: max_msgqueue_size,
            max_nice_priority: max_nice_priority,
            max_realtime_priority: max_realtime_priority,
            max_realtime_timeout: max_realtime_timeout,
        })
    ))
);

/// A resource limit, including a soft and hard bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Limit<T> {
    /// The soft resource limit.
    ///
    /// The kernel enforces that resource usage does not exceed this value.
    pub soft: Option<T>,

    /// The hard resource limit.
    ///
    /// The kernel allows the soft limit to be raised until this limit using
    /// `setrlimit`.
    pub hard: Option<T>,
}

/// Process limits information
/// See `man 2 getrlimit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Limits {
    /// The maximum CPU time a process can use.
    pub max_cpu_time: Limit<Duration>,
    /// The maximum size of files that the process may create in bytes.
    pub max_file_size: Limit<u64>,
    /// The maximum size of the process's data segment in bytes.
    pub max_data_size: Limit<usize>,
    /// The maximum size of the process stack in bytes.
    pub max_stack_size: Limit<usize>,
    /// Maximum size of a core file in bytes.
    pub max_core_file_size: Limit<usize>,
    /// Specifies the limit of the process's resident set in bytes.
    pub max_resident_set: Limit<usize>,
    /// The maximum number of processes (or, more precisely on Linux, threads)
    /// that can be created for the real user ID of the calling process.
    pub max_processes: Limit<usize>,
    /// Specifies a value one greater than the maximum file descriptor
    /// number that can be opened by this process.
    pub max_open_files: Limit<usize>,
    /// The maximum number of bytes of memory that may be locked into RAM.
    pub max_locked_memory: Limit<usize>,
    /// The maximum size of the process's virtual memory (address space) in bytes.
    pub max_address_space: Limit<usize>,
    /// A limit on the combined number of locks and leases that this process may
    /// establish.
    pub max_file_locks: Limit<usize>,
    /// Specifies the limit on the number of signals that may be queued for the
    /// real user ID of the calling process.
    pub max_pending_signals: Limit<usize>,
    /// Specifies the limit on the number of bytes that can be allocated for
    /// POSIX message queues for the real user ID of the calling process.
    pub max_msgqueue_size: Limit<usize>,
    /// Specifies a ceiling to which the process's nice value can be raised.
    pub max_nice_priority: Limit<usize>,
    /// Specifies a limit on the amount of CPU time that a process scheduled
    /// under a real-time scheduling policy may consume without making a blocking
    /// system call.
    pub max_realtime_priority: Limit<usize>,
    /// Specifies a ceiling on the real-time priority that may be set for this process.
    pub max_realtime_timeout: Limit<Duration>,
}

fn limits_file(file: &mut File) -> Result<Limits> {
    let mut buf = [0; 2048]; // A typical limits file is about 1350 bytes
    map_result(parse_limits(try!(read_to_end(file, &mut buf))))
}

/// Returns resource limit information from the process with the provided pid.
pub fn limits(pid: pid_t) -> Result<Limits> {
    limits_file(&mut try!(File::open(&format!("/proc/{}/limits", pid))))
}

/// Returns resource limit information for the current process.
pub fn limits_self() -> Result<Limits> {
    limits_file(&mut try!(File::open("/proc/self/limits")))
}

#[cfg(test)]
pub mod tests {

    use std::time::Duration;

    use parsers::tests::unwrap;
    use super::{limits, limits_self, parse_limits};

    /// Test that the system limit file can be parsed.
    #[test]
    fn test_limits() {
        limits_self().unwrap();
        limits(1).unwrap();
    }

    #[test]
    fn test_parse_limits() {
        let text = b"Limit                     Soft Limit           Hard Limit           Units         \n
                     Max cpu time              10                   60                   seconds       \n
                     Max file size             unlimited            unlimited            bytes         \n
                     Max data size             unlimited            unlimited            bytes         \n
                     Max stack size            8388608              unlimited            bytes         \n
                     Max core file size        unlimited            unlimited            bytes         \n
                     Max resident set          unlimited            unlimited            bytes         \n
                     Max processes             63632                63632                processes     \n
                     Max open files            1024                 4096                 files         \n
                     Max locked memory         65536                65536                bytes         \n
                     Max address space         unlimited            unlimited            bytes         \n
                     Max file locks            unlimited            unlimited            locks         \n
                     Max pending signals       63632                63632                signals       \n
                     Max msgqueue size         819200               819200               bytes         \n
                     Max nice priority         0                    0                                  \n
                     Max realtime priority     0                    0                                  \n
                     Max realtime timeout      500                  unlimited            us            \n";

        let limits = unwrap(parse_limits(text));

        assert_eq!(Some(Duration::new(10, 0)), limits.max_cpu_time.soft);
        assert_eq!(Some(Duration::new(60, 0)), limits.max_cpu_time.hard);

        assert_eq!(None, limits.max_file_size.soft);
        assert_eq!(None, limits.max_file_size.hard);

        assert_eq!(None, limits.max_data_size.soft);
        assert_eq!(None, limits.max_data_size.hard);

        assert_eq!(Some(8388608), limits.max_stack_size.soft);
        assert_eq!(None, limits.max_stack_size.hard);

        assert_eq!(None, limits.max_core_file_size.soft);
        assert_eq!(None, limits.max_core_file_size.hard);

        assert_eq!(None, limits.max_resident_set.soft);
        assert_eq!(None, limits.max_resident_set.hard);

        assert_eq!(Some(63632), limits.max_processes.soft);
        assert_eq!(Some(63632), limits.max_processes.hard);

        assert_eq!(Some(1024), limits.max_open_files.soft);
        assert_eq!(Some(4096), limits.max_open_files.hard);

        assert_eq!(Some(65536), limits.max_locked_memory.soft);
        assert_eq!(Some(65536), limits.max_locked_memory.hard);

        assert_eq!(None, limits.max_address_space.soft);
        assert_eq!(None, limits.max_address_space.hard);

        assert_eq!(None, limits.max_file_locks.soft);
        assert_eq!(None, limits.max_file_locks.hard);

        assert_eq!(Some(63632), limits.max_pending_signals.soft);
        assert_eq!(Some(63632), limits.max_pending_signals.hard);

        assert_eq!(Some(819200), limits.max_msgqueue_size.soft);
        assert_eq!(Some(819200), limits.max_msgqueue_size.hard);

        assert_eq!(Some(0), limits.max_nice_priority.soft);
        assert_eq!(Some(0), limits.max_nice_priority.hard);

        assert_eq!(Some(0), limits.max_realtime_priority.soft);
        assert_eq!(Some(0), limits.max_realtime_priority.hard);

        assert_eq!(Some(Duration::new(0, 500 * 1000)), limits.max_realtime_timeout.soft);
        assert_eq!(None, limits.max_realtime_timeout.hard);
    }
}

#[cfg(all(test, rustc_nightly))]
mod benches {
    extern crate test;

    use std::fs::File;

    use parsers::read_to_end;
    use super::*;

    #[bench]
    fn bench_limits(b: &mut test::Bencher) {
        b.iter(|| test::black_box(limits(1)));
    }

    #[bench]
    fn bench_limits_parse(b: &mut test::Bencher) {
        let mut buf = [0; 2048];
        let limits = read_to_end(&mut File::open("/proc/1/limits").unwrap(), &mut buf).unwrap();
        b.iter(|| test::black_box(parse_limits(limits)));
    }
}
