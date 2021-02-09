use crate::{FileWrapper, ProcResult};
use std::io::{BufRead, BufReader};

/// Disk IO stat information
///
/// To fully understand these fields, please see the [iostats.txt](https://www.kernel.org/doc/Documentation/iostats.txt)
/// kernel documentation
// Doc reference: https://www.kernel.org/doc/Documentation/ABI/testing/procfs-diskstats
// Doc reference: https://www.kernel.org/doc/Documentation/iostats.txt
#[derive(Debug)]
pub struct DiskStat {
    /// The device major number
    pub major: i32,

    /// The device minor number
    pub minor: i32,

    /// Device name
    pub name: String,

    /// Reads completed successfully
    ///
    /// This is the total number rof reads comopleted successfully
    pub reads: usize,

    /// Reads merged
    ///
    /// The number of adjacent reads that have been merged for efficiency.
    pub merged: usize,

    /// Sectors read successfully
    ///
    /// This is the total number of sectors read successfully.
    pub sectors_read: usize,

    /// Time spent reading (ms)
    pub time_reading: usize,

    /// writes completed
    pub writes: usize,

    /// writes merged
    ///
    /// The number of adjacent writes that have been merged for efficiency.
    pub writes_merged: usize,

    /// Sectors written successfully
    pub sectors_written: usize,

    /// Time spent writing (ms)
    pub time_writing: usize,

    /// I/Os currently in progress
    pub in_progress: usize,

    /// Time spent doing I/Os (ms)
    pub time_in_progress: usize,

    /// Weighted time spent doing I/Os (ms)
    pub weighted_time_in_progress: usize,

    /// Discards completed successfully
    ///
    /// (since kernel 4.18)
    pub discards: Option<usize>,

    /// Discards merged
    pub discards_merged: Option<usize>,

    /// Sectors discarded
    pub sectors_discarded: Option<usize>,

    /// Time spent discarding
    pub time_discarding: Option<usize>,

    /// Flush requests completed successfully
    ///
    /// (since kernel 5.5)
    pub flushes: Option<usize>,

    /// Time spent flushing
    pub time_flushing: Option<usize>,
}

/// Get disk IO stat info from /proc/diskstats
pub fn diskstats() -> ProcResult<Vec<DiskStat>> {
    let file = FileWrapper::open("/proc/diskstats")?;
    let reader = BufReader::new(file);
    let mut v = Vec::new();

    for line in reader.lines() {
        let line = line?;
        v.push(DiskStat::from_line(&line)?);
    }
    Ok(v)
}

impl DiskStat {
    pub fn from_line(line: &str) -> ProcResult<DiskStat> {
        let mut s = line.trim().split_whitespace();

        let major = from_str!(i32, expect!(s.next()));
        let minor = from_str!(i32, expect!(s.next()));
        let name = expect!(s.next()).to_string();
        let reads = from_str!(usize, expect!(s.next()));
        let merged = from_str!(usize, expect!(s.next()));
        let sectors_read = from_str!(usize, expect!(s.next()));
        let time_reading = from_str!(usize, expect!(s.next()));
        let writes = from_str!(usize, expect!(s.next()));
        let writes_merged = from_str!(usize, expect!(s.next()));
        let sectors_written = from_str!(usize, expect!(s.next()));
        let time_writing = from_str!(usize, expect!(s.next()));
        let in_progress = from_str!(usize, expect!(s.next()));
        let time_in_progress = from_str!(usize, expect!(s.next()));
        let weighted_time_in_progress = from_str!(usize, expect!(s.next()));
        let discards = s.next().and_then(|s| usize::from_str_radix(s, 10).ok());
        let discards_merged = s.next().and_then(|s| usize::from_str_radix(s, 10).ok());
        let sectors_discarded = s.next().and_then(|s| usize::from_str_radix(s, 10).ok());
        let time_discarding = s.next().and_then(|s| usize::from_str_radix(s, 10).ok());
        let flushes = s.next().and_then(|s| usize::from_str_radix(s, 10).ok());
        let time_flushing = s.next().and_then(|s| usize::from_str_radix(s, 10).ok());

        Ok(DiskStat {
            major,
            minor,
            name,
            reads,
            merged,
            sectors_read,
            time_reading,
            writes,
            writes_merged,
            sectors_written,
            time_writing,
            in_progress,
            time_in_progress,
            weighted_time_in_progress,
            discards,
            discards_merged,
            sectors_discarded,
            time_discarding,
            flushes,
            time_flushing,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn diskstat() {
        for disk in super::diskstats().unwrap() {
            println!("{:?}", disk);
        }
    }
}
