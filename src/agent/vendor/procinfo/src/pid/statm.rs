//! Process memory usage information from `/proc/[pid]/statm`.

use std::fs::File;
use std::io::Result;

use libc::pid_t;
use nom::{digit, line_ending, space};

use parsers::{map_result, parse_usize, read_to_end};

/// Process memory usage information.
///
/// All values are in units of pages.
///
/// See `man 5 proc` and `Linux/fs/proc/array.c`.
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct Statm {
    /// Total virtual memory size.
    pub size: usize,
    /// Resident non-swapped memory.
    pub resident: usize,
    /// Shared memory.
    pub share: usize,
    /// Resident executable memory.
    pub text: usize,
    /// Resident data and stack memory.
    pub data: usize,
}

/// Parses the statm file format.
named!(parse_statm<Statm>,
    chain!(size: parse_usize     ~ space ~
           resident: parse_usize ~ space ~
           share: parse_usize    ~ space ~
           text: parse_usize     ~ space ~
           digit                 ~ space ~         // lib - unused since linux 2.6
           data: parse_usize     ~ space ~
           digit                 ~ line_ending,    // dt - unused since linux 2.6
           || { Statm { size: size,
                        resident: resident,
                        share: share,
                        text: text,
                        data: data } }));

/// Parses the provided statm file.
fn statm_file(file: &mut File) -> Result<Statm> {
    let mut buf = [0; 256]; // A typical statm file is about 25 bytes
    map_result(parse_statm(try!(read_to_end(file, &mut buf))))
}

/// Returns memory status information for the process with the provided pid.
pub fn statm(pid: pid_t) -> Result<Statm> {
    statm_file(&mut try!(File::open(&format!("/proc/{}/statm", pid))))
}

/// Returns memory status information for the current process.
pub fn statm_self() -> Result<Statm> {
    statm_file(&mut try!(File::open("/proc/self/statm")))
}

#[cfg(test)]
mod tests {
    use parsers::tests::unwrap;
    use super::{parse_statm, statm, statm_self};

    /// Test that the system statm files can be parsed.
    #[test]
    fn test_statm() {
        statm_self().unwrap();
        statm(1).unwrap();
    }

    #[test]
    fn test_parse_statm() {
        let statm_text = b"11837 2303 1390 330 0 890 0\n";
        let statm = unwrap(parse_statm(statm_text));
        assert_eq!(11837, statm.size);
        assert_eq!(2303, statm.resident);
        assert_eq!(1390, statm.share);
        assert_eq!(330, statm.text);
        assert_eq!(890, statm.data);
    }
}

#[cfg(all(test, rustc_nightly))]
mod benches {
    extern crate test;

    use std::fs::File;

    use parsers::read_to_end;
    use super::{parse_statm, statm};

    #[bench]
    fn bench_statm(b: &mut test::Bencher) {
        b.iter(|| test::black_box(statm(1)));
    }

    #[bench]
    fn bench_statm_parse(b: &mut test::Bencher) {
        let mut buf = [0; 256];
        let statm = read_to_end(&mut File::open("/proc/1/statm").unwrap(), &mut buf).unwrap();
        b.iter(|| test::black_box(parse_statm(statm)));
    }
}
