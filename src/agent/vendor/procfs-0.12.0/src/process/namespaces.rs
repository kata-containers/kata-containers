use std::{
    ffi::{CString, OsString},
    fs::{self},
    os::unix::prelude::OsStrExt,
    path::PathBuf,
};

use crate::ProcResult;

use super::Process;

impl Process {
    /// Describes namespaces to which the process with the corresponding PID belongs.
    /// Doc reference: https://man7.org/linux/man-pages/man7/namespaces.7.html
    pub fn namespaces(&self) -> ProcResult<Vec<Namespace>> {
        let ns = self.root.join("ns");
        let mut namespaces = Vec::new();
        for entry in fs::read_dir(ns)? {
            let entry = entry?;
            let path = entry.path();
            let ns_type = entry.file_name();
            let cstr = CString::new(path.as_os_str().as_bytes()).unwrap();

            let mut stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::stat64(cstr.as_ptr(), &mut stat) } != 0 {
                return Err(build_internal_error!(format!("Unable to stat {:?}", path)));
            }

            namespaces.push(Namespace {
                ns_type,
                path,
                identifier: stat.st_ino,
                device_id: stat.st_dev,
            })
        }

        Ok(namespaces)
    }
}

/// Information about a namespace
///
/// See also the [Process::namespaces()] method
#[derive(Debug, Clone)]
pub struct Namespace {
    /// Namespace type
    pub ns_type: OsString,
    /// Handle to the namespace
    pub path: PathBuf,
    /// Namespace identifier (inode number)
    pub identifier: u64,
    /// Device id of the namespace
    pub device_id: u64,
}

impl PartialEq for Namespace {
    fn eq(&self, other: &Self) -> bool {
        // see https://lore.kernel.org/lkml/87poky5ca9.fsf@xmission.com/
        self.identifier == other.identifier && self.device_id == other.device_id
    }
}

impl Eq for Namespace {}

#[cfg(test)]
mod tests {
    use crate::process::Process;

    #[test]
    fn test_namespaces() {
        let myself = Process::myself().unwrap();
        let namespaces = myself.namespaces().unwrap();
        print!("{:?}", namespaces);
    }
}
