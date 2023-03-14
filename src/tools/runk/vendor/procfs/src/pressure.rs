//! Pressure stall information retreived from `/proc/pressure/cpu`,
//! `/proc/pressure/memory` and `/proc/pressure/io`
//! may not be available on kernels older than 4.20.0
//! For reference: https://lwn.net/Articles/759781/
//!
//! See also: https://www.kernel.org/doc/Documentation/accounting/psi.txt

use crate::{ProcError, ProcResult};
use std::collections::HashMap;

/// Pressure stall information for either CPU, memory, or IO.
///
/// See also: https://www.kernel.org/doc/Documentation/accounting/psi.txt
#[derive(Debug, Clone)]
pub struct PressureRecord {
    /// 10 second window
    ///
    /// The percentage of time, over a 10 second window, that either some or all tasks were stalled
    /// waiting for a resource.
    pub avg10: f32,
    /// 60 second window
    ///
    /// The percentage of time, over a 60 second window, that either some or all tasks were stalled
    /// waiting for a resource.
    pub avg60: f32,
    /// 300 second window
    ///
    /// The percentage of time, over a 300 second window, that either some or all tasks were stalled
    /// waiting for a resource.
    pub avg300: f32,
    /// Total stall time (in microseconds).
    pub total: u64,
}

/// CPU pressure information
#[derive(Debug, Clone)]
pub struct CpuPressure {
    pub some: PressureRecord,
}

impl CpuPressure {
    /// Get CPU pressure information
    pub fn new() -> ProcResult<CpuPressure> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let file = File::open("/proc/pressure/cpu")?;
        let mut reader = BufReader::new(file);

        let mut some = String::new();
        reader.read_line(&mut some)?;

        Ok(CpuPressure {
            some: parse_pressure_record(&some)?,
        })
    }
}

/// Memory pressure information
#[derive(Debug, Clone)]
pub struct MemoryPressure {
    /// This record indicates the share of time in which at least some tasks are stalled
    pub some: PressureRecord,
    /// This record indicates this share of time in which all non-idle tasks are stalled
    /// simultaneously.
    pub full: PressureRecord,
}

impl MemoryPressure {
    /// Get memory pressure information
    pub fn new() -> ProcResult<MemoryPressure> {
        let (some, full) = get_pressure("memory")?;

        Ok(MemoryPressure { some, full })
    }
}

/// IO pressure information
#[derive(Debug, Clone)]
pub struct IoPressure {
    /// This record indicates the share of time in which at least some tasks are stalled
    pub some: PressureRecord,
    /// This record indicates this share of time in which all non-idle tasks are stalled
    /// simultaneously.
    pub full: PressureRecord,
}

impl IoPressure {
    /// Get IO pressure information
    pub fn new() -> ProcResult<IoPressure> {
        let (some, full) = get_pressure("io")?;

        Ok(IoPressure { some, full })
    }
}

fn get_f32(map: &HashMap<&str, &str>, value: &str) -> ProcResult<f32> {
    map.get(value).map_or_else(
        || Err(ProcError::Incomplete(None)),
        |v| v.parse::<f32>().map_err(|_| ProcError::Incomplete(None)),
    )
}

fn get_total(map: &HashMap<&str, &str>) -> ProcResult<u64> {
    map.get("total").map_or_else(
        || Err(ProcError::Incomplete(None)),
        |v| v.parse::<u64>().map_err(|_| ProcError::Incomplete(None)),
    )
}

fn parse_pressure_record(line: &str) -> ProcResult<PressureRecord> {
    let mut parsed = HashMap::new();

    if !line.starts_with("some") && !line.starts_with("full") {
        return Err(ProcError::Incomplete(None));
    }

    let values = &line[5..];

    for kv_str in values.split_whitespace() {
        let kv_split = kv_str.split('=');
        let vec: Vec<&str> = kv_split.collect();
        if vec.len() == 2 {
            parsed.insert(vec[0], vec[1]);
        }
    }

    Ok(PressureRecord {
        avg10: get_f32(&parsed, "avg10")?,
        avg60: get_f32(&parsed, "avg60")?,
        avg300: get_f32(&parsed, "avg300")?,
        total: get_total(&parsed)?,
    })
}

fn get_pressure(pressure_file: &str) -> ProcResult<(PressureRecord, PressureRecord)> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(format!("/proc/pressure/{}", pressure_file))?;
    let mut reader = BufReader::new(file);

    let mut some = String::new();
    reader.read_line(&mut some)?;
    let mut full = String::new();
    reader.read_line(&mut full)?;

    Ok((parse_pressure_record(&some)?, parse_pressure_record(&full)?))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::f32::EPSILON;
    use std::path::Path;

    #[allow(clippy::manual_range_contains)]
    fn valid_percentage(value: f32) -> bool {
        value >= 0.00 && value < 100.0
    }

    #[test]
    fn test_parse_pressure_record() {
        let record = parse_pressure_record("full avg10=2.10 avg60=0.12 avg300=0.00 total=391926").unwrap();

        assert!(record.avg10 - 2.10 < EPSILON);
        assert!(record.avg60 - 0.12 < EPSILON);
        assert!(record.avg300 - 0.00 < EPSILON);
        assert_eq!(record.total, 391_926);
    }

    #[test]
    fn test_parse_pressure_record_errs() {
        assert!(parse_pressure_record("avg10=2.10 avg60=0.12 avg300=0.00 total=391926").is_err());
        assert!(parse_pressure_record("some avg10=2.10 avg300=0.00 total=391926").is_err());
        assert!(parse_pressure_record("some avg10=2.10 avg60=0.00 avg300=0.00").is_err());
    }

    #[test]
    fn test_mem_pressure() {
        if !Path::new("/proc/pressure/memory").exists() {
            return;
        }

        let mem_psi = MemoryPressure::new().unwrap();
        assert!(valid_percentage(mem_psi.some.avg10));
        assert!(valid_percentage(mem_psi.some.avg60));
        assert!(valid_percentage(mem_psi.some.avg300));

        assert!(valid_percentage(mem_psi.full.avg10));
        assert!(valid_percentage(mem_psi.full.avg60));
        assert!(valid_percentage(mem_psi.full.avg300));
    }

    #[test]
    fn test_io_pressure() {
        if !Path::new("/proc/pressure/io").exists() {
            return;
        }

        let io_psi = IoPressure::new().unwrap();
        assert!(valid_percentage(io_psi.some.avg10));
        assert!(valid_percentage(io_psi.some.avg60));
        assert!(valid_percentage(io_psi.some.avg300));

        assert!(valid_percentage(io_psi.full.avg10));
        assert!(valid_percentage(io_psi.full.avg60));
        assert!(valid_percentage(io_psi.full.avg300));
    }

    #[test]
    fn test_cpu_pressure() {
        if !Path::new("/proc/pressure/cpu").exists() {
            return;
        }

        let cpu_psi = CpuPressure::new().unwrap();
        assert!(valid_percentage(cpu_psi.some.avg10));
        assert!(valid_percentage(cpu_psi.some.avg60));
        assert!(valid_percentage(cpu_psi.some.avg300));
    }
}
