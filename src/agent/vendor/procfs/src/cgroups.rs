use crate::ProcResult;

use super::process::Process;

#[derive(Debug)]
/// Container group controller information.
///
/// See also the [cgroups()] method.
pub struct CGroupController {
    /// The name of the controller.
    pub name: String,
    /// The  unique  ID  of  the  cgroup hierarchy on which this controller is mounted.
    ///
    /// If multiple cgroups v1 controllers are bound to the same  hierarchy, then each will show
    /// the same hierarchy ID in this field.  The value in this field will be 0 if:
    ///
    /// * the controller is not mounted on a cgroups v1 hierarchy;
    /// * the controller is bound to the cgroups v2 single unified hierarchy; or
    /// * the controller is disabled (see below).
    pub hierarchy: u32,
    /// The number of control groups in this hierarchy using this controller.
    pub num_cgroups: u32,
    /// This field contains the value `true` if this controller is enabled, or `false` if it has been disabled
    pub enabled: bool,
}

/// Information about the cgroup controllers that are compiled into the kernel
///
/// (since Linux 2.6.24)
// This is returning a vector, but if each subsystem name is unique, maybe this can be a hashmap
// instead
pub fn cgroups() -> ProcResult<Vec<CGroupController>> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open("/proc/cgroups")?;
    let reader = BufReader::new(file);

    let mut vec = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') {
            continue;
        }

        let mut s = line.split_whitespace();
        let name = expect!(s.next(), "name").to_owned();
        let hierarchy = from_str!(u32, expect!(s.next(), "hierarchy"));
        let num_cgroups = from_str!(u32, expect!(s.next(), "num_cgroups"));
        let enabled = expect!(s.next(), "enabled") == "1";

        vec.push(CGroupController {
            name,
            hierarchy,
            num_cgroups,
            enabled,
        });
    }

    Ok(vec)
}

/// Information about a process cgroup
///
/// See also the [Process::cgroups()] method.
#[derive(Debug)]
pub struct ProcessCgroup {
    /// For cgroups version 1 hierarchies, this field contains a  unique  hierarchy  ID  number
    /// that  can  be  matched  to  a  hierarchy  ID  in /proc/cgroups.  For the cgroups version 2
    /// hierarchy, this field contains the value 0.
    pub hierarchy: u32,
    /// For cgroups version 1 hierarchies, this field contains a comma-separated list of the
    /// controllers bound to the hierarchy.
    ///
    /// For the cgroups version 2 hierarchy, this field is empty.
    pub controllers: Vec<String>,

    /// This field contains the pathname of the control group in the hierarchy to which the process
    /// belongs.
    ///
    /// This pathname is  relative  to  the mount point of the hierarchy.
    pub pathname: String,
}

impl Process {
    /// Describes control groups to which the process with the corresponding PID belongs.
    ///
    /// The displayed information differs for cgroupsversion 1 and version 2 hierarchies.
    pub fn cgroups(&self) -> ProcResult<Vec<ProcessCgroup>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let file = File::open(self.root.join("cgroup"))?;
        let reader = BufReader::new(file);

        let mut vec = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.starts_with('#') {
                continue;
            }

            let mut s = line.split(':');
            let hierarchy = from_str!(u32, expect!(s.next(), "hierarchy"));
            let controllers = expect!(s.next(), "controllers")
                .split(',')
                .map(|s| s.to_owned())
                .collect();
            let pathname = expect!(s.next(), "path").to_owned();

            vec.push(ProcessCgroup {
                hierarchy,
                controllers,
                pathname,
            });
        }

        Ok(vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgroups() {
        let groups = cgroups().unwrap();
        println!("{:?}", groups);
    }

    #[test]
    fn test_process_cgroups() {
        let myself = Process::myself().unwrap();
        let groups = myself.cgroups();
        println!("{:?}", groups);
    }
}
