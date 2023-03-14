#![deny(unsafe_code)]

use super::rlim_type::{RawRlim, Rlim};

use std::fs;
use std::io::{self, BufRead};
use std::num::ParseIntError;
use std::path::Path;

use libc::pid_t;

/// \[Linux\] A process's resource limits. It is parsed from the **proc** filesystem.
///
/// See <https://man7.org/linux/man-pages/man5/proc.5.html>.
///
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ProcLimits {
    /// Max cpu time. See also [Resource::CPU](struct.Resource.html#associatedconstant.CPU).
    pub max_cpu_time: Option<ProcLimit>,
    /// Max file size. See also [Resource::FSIZE](struct.Resource.html#associatedconstant.FSIZE).
    pub max_file_size: Option<ProcLimit>,
    /// Max data size. See also [Resource::DATA](struct.Resource.html#associatedconstant.DATA).
    pub max_data_size: Option<ProcLimit>,
    /// Max stack size. See also [Resource::STACK](struct.Resource.html#associatedconstant.STACK).
    pub max_stack_size: Option<ProcLimit>,
    /// Max core file size. See also [Resource::CORE](struct.Resource.html#associatedconstant.CORE).
    pub max_core_file_size: Option<ProcLimit>,
    /// Max resident set. See also [Resource::RSS](struct.Resource.html#associatedconstant.RSS).
    pub max_resident_set: Option<ProcLimit>,
    /// Max processes. See also [Resource::NPROC](struct.Resource.html#associatedconstant.NPROC).
    pub max_processes: Option<ProcLimit>,
    /// Max open files. See also [Resource::NOFILE](struct.Resource.html#associatedconstant.NOFILE).
    pub max_open_files: Option<ProcLimit>,
    /// Max locked memory. See also [Resource::MEMLOCK](struct.Resource.html#associatedconstant.MEMLOCK).
    pub max_locked_memory: Option<ProcLimit>,
    /// Max address space. See also [Resource::AS](struct.Resource.html#associatedconstant.AS).
    pub max_address_space: Option<ProcLimit>,
    /// Max file locks. See also [Resource::LOCKS](struct.Resource.html#associatedconstant.LOCKS).
    pub max_file_locks: Option<ProcLimit>,
    /// Max pending signals. See also [Resource::SIGPENDING](struct.Resource.html#associatedconstant.SIGPENDING).
    pub max_pending_signals: Option<ProcLimit>,
    /// Max msgqueue size. See also [Resource::MSGQUEUE](struct.Resource.html#associatedconstant.MSGQUEUE).
    pub max_msgqueue_size: Option<ProcLimit>,
    /// Max nice priority. See also [Resource::NICE](struct.Resource.html#associatedconstant.NICE).
    pub max_nice_priority: Option<ProcLimit>,
    /// Max realtime priority. See also [Resource::RTPRIO](struct.Resource.html#associatedconstant.RTPRIO).
    pub max_realtime_priority: Option<ProcLimit>,
    /// Max realtime timeout. See also [Resource::RTTIME](struct.Resource.html#associatedconstant.RTTIME).
    pub max_realtime_timeout: Option<ProcLimit>,
}

/// \[Linux\] A process's resource limit field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcLimit {
    /// Soft limit. `None` indicates `unlimited`.
    pub soft_limit: Option<Rlim>,
    /// Hard limit. `None` indicates `unlimited`.
    pub hard_limit: Option<Rlim>,
}

impl ProcLimits {
    /// Reads the current process's resource limits from `/proc/self/limits`.
    ///
    /// # Errors
    /// Returns an error if any IO operation failed.
    ///
    /// Returns an error if the file format is invalid.
    ///
    pub fn read_self() -> io::Result<Self> {
        Self::read_proc_fs("/proc/self/limits")
    }

    /// Reads a process's resource limits from `/proc/[pid]/limits`.
    ///
    /// # Errors
    /// Returns an error if `pid` is negative.
    ///
    /// Returns an error if any IO operation failed.
    ///
    /// Returns an error if the file format is invalid.
    ///
    pub fn read_process(pid: pid_t) -> io::Result<Self> {
        if pid < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ProcLimits: pid must be non-negative",
            ));
        }
        Self::read_proc_fs(format!("/proc/{}/limits", pid))
    }

    fn read_proc_fs(limits_path: impl AsRef<Path>) -> io::Result<Self> {
        fn parse_head(head: &str) -> Option<(usize, usize, usize)> {
            let s_idx = head.find('S')?;
            let h_idx = head[s_idx..].find('H')?;
            let u_idx = head[s_idx + h_idx..].find('U')?;
            Some((s_idx, h_idx, u_idx))
        }

        fn parse_limit_number(s: &str) -> Result<Option<Rlim>, ParseIntError> {
            match s {
                "unlimited" => Ok(None),
                _ => match s.parse::<RawRlim>() {
                    Ok(n) => Ok(Some(Rlim::from_raw(n))),
                    Err(e) => Err(e),
                },
            }
        }

        fn error_missing_table_head() -> io::Error {
            io::Error::new(io::ErrorKind::Other, "ProcLimits: missing table head")
        }

        fn error_invalid_table_head() -> io::Error {
            io::Error::new(io::ErrorKind::Other, "ProcLimits: invalid table head")
        }

        fn error_invalid_limit_number(e: ParseIntError) -> io::Error {
            let ans = io::Error::new(
                io::ErrorKind::Other,
                format!("ProcLimits: invalid limit number: {}", e),
            );
            drop(e);
            ans
        }

        fn error_duplicate_limit_field() -> io::Error {
            io::Error::new(io::ErrorKind::Other, "ProcLimits: duplicate limit field")
        }

        fn error_unknown_limit_field(s: &str) -> io::Error {
            io::Error::new(
                io::ErrorKind::Other,
                format!("ProcLimits: unknown limit field: {:?}", s),
            )
        }

        let reader = io::BufReader::new(fs::File::open(limits_path)?);
        let mut lines = reader.lines();

        let head = lines.next().ok_or_else(error_missing_table_head)??;

        let (name_len, soft_len, hard_len) =
            parse_head(&head).ok_or_else(error_invalid_table_head)?;

        let mut ans = Self::default();

        let sorted_table: [(&str, &mut Option<ProcLimit>); 16] = [
            ("max address space", &mut ans.max_address_space),
            ("max core file size", &mut ans.max_core_file_size),
            ("max cpu time", &mut ans.max_cpu_time),
            ("max data size", &mut ans.max_data_size),
            ("max file locks", &mut ans.max_file_locks),
            ("max file size", &mut ans.max_file_size),
            ("max locked memory", &mut ans.max_locked_memory),
            ("max msgqueue size", &mut ans.max_msgqueue_size),
            ("max nice priority", &mut ans.max_nice_priority),
            ("max open files", &mut ans.max_open_files),
            ("max pending signals", &mut ans.max_pending_signals),
            ("max processes", &mut ans.max_processes),
            ("max realtime priority", &mut ans.max_realtime_priority),
            ("max realtime timeout", &mut ans.max_realtime_timeout),
            ("max resident set", &mut ans.max_resident_set),
            ("max stack size", &mut ans.max_stack_size),
        ];

        for line in lines {
            let line = line?;

            let (name, line) = line.split_at(name_len);
            let (soft, line) = line.split_at(soft_len);
            let (hard, _) = line.split_at(hard_len);

            let name = name.trim().to_lowercase();
            let soft_limit = parse_limit_number(soft.trim()).map_err(error_invalid_limit_number)?;
            let hard_limit = parse_limit_number(hard.trim()).map_err(error_invalid_limit_number)?;
            let limit = ProcLimit {
                soft_limit,
                hard_limit,
            };

            match sorted_table.binary_search_by_key(&name.as_str(), |&(s, _)| s) {
                Ok(idx) => {
                    let field = &mut *sorted_table[idx].1;
                    if field.is_some() {
                        return Err(error_duplicate_limit_field());
                    }
                    *field = Some(limit)
                }
                Err(_) => return Err(error_unknown_limit_field(&name)),
            }
        }

        Ok(ans)
    }
}
