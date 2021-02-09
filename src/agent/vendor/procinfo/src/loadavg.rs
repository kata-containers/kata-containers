//! System load and task statistics from `/proc/loadavg`.

use std::fs::File;
use std::io::Result;

use libc::pid_t;
use nom::{line_ending, space};

use parsers::{map_result, parse_f32, parse_i32, parse_u32, read_to_end};

/// System load and task statistics.
///
/// The load average is the ratio of runnable and uninterruptible (waiting on IO) tasks to total
/// tasks on the system.
///
/// See `man 5 proc` and `Linux/fs/proc/loadavg.c`.
#[derive(Debug, Default, PartialEq)]
pub struct LoadAvg {
    /// Load average over the last minute.
    pub load_avg_1_min: f32,
    /// Load average of the last 5 minutes.
    pub load_avg_5_min: f32,
    /// Load average of the last 10 minutes
    pub load_avg_10_min: f32,
    /// the number of currently runnable kernel scheduling entities (processes, threads).
    pub tasks_runnable: u32,
    /// the number of kernel scheduling entities that currently exist on the system.
    pub tasks_total: u32,
    /// the PID of the process that was most recently created on the system.
    pub last_created_pid: pid_t,
}

/// Parses the loadavg file format.
named!(parse_loadavg<LoadAvg>,
       chain!(load_avg_1_min:   parse_f32   ~ space ~
              load_avg_5_min:   parse_f32   ~ space ~
              load_avg_10_min:  parse_f32   ~ space ~
              tasks_runnable:   parse_u32   ~ tag!("/") ~
              tasks_total:      parse_u32   ~ space ~
              last_created_pid: parse_i32   ~ line_ending,
              || { LoadAvg { load_avg_1_min: load_avg_1_min,
                             load_avg_5_min: load_avg_5_min,
                             load_avg_10_min: load_avg_10_min,
                             tasks_runnable: tasks_runnable,
                             tasks_total: tasks_total,
                             last_created_pid: last_created_pid } }));

/// Returns the system load average.
pub fn loadavg() -> Result<LoadAvg> {
    let mut buf = [0; 128]; // A typical loadavg file is about 32 bytes.
    let mut file = try!(File::open("/proc/loadavg"));
    map_result(parse_loadavg(try!(read_to_end(&mut file, &mut buf))))
}

#[cfg(test)]
mod tests {
    use super::{loadavg, parse_loadavg};
    use parsers::tests::unwrap;

    /// Test that the system loadavg file can be parsed.
    #[test]
    fn test_loadavg() {
        loadavg().unwrap();
    }

    #[test]
    fn test_parse_loadavg() {
        let loadavg_text = b"0.46 0.33 0.28 34/625 8435\n";
        let loadavg = unwrap(parse_loadavg(loadavg_text));
        assert_eq!(0.46, loadavg.load_avg_1_min);
        assert_eq!(0.33, loadavg.load_avg_5_min);
        assert_eq!(0.28, loadavg.load_avg_10_min);
        assert_eq!(34, loadavg.tasks_runnable);
        assert_eq!(625, loadavg.tasks_total);
        assert_eq!(8435, loadavg.last_created_pid);
    }
}

#[cfg(all(test, rustc_nightly))]
mod benches {
    extern crate test;

    use std::fs::File;

    use parsers::read_to_end;
    use super::{loadavg, parse_loadavg};

    #[bench]
    fn bench_loadavg(b: &mut test::Bencher) {
        b.iter(|| test::black_box(loadavg()));
    }

    #[bench]
    fn bench_loadavg_parse(b: &mut test::Bencher) {
        let mut buf = [0; 128];
        let statm = read_to_end(&mut File::open("/proc/loadavg").unwrap(), &mut buf).unwrap();
        b.iter(|| test::black_box(parse_loadavg(statm)));
    }
}
