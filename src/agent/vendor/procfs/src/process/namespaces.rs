use rustix::fs::{AtFlags, Mode, OFlags};
use std::{collections::HashMap, ffi::OsString, path::PathBuf};

#[cfg(feature = "serde1")]
use serde::{Deserialize, Serialize};

use crate::ProcResult;

use super::Process;

impl Process {
    /// Describes namespaces to which the process with the corresponding PID belongs.
    /// Doc reference: <https://man7.org/linux/man-pages/man7/namespaces.7.html>
    /// The namespace type is the key for the HashMap, i.e 'net', 'user', etc.
    pub fn namespaces(&self) -> ProcResult<HashMap<OsString, Namespace>> {
        let mut namespaces = HashMap::new();
        let dir_ns = wrap_io_error!(
            self.root.join("ns"),
            rustix::fs::openat(
                &self.fd,
                "ns",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty()
            )
        )?;
        let dir = wrap_io_error!(self.root.join("ns"), rustix::fs::Dir::read_from(&dir_ns))?;
        for entry in dir {
            let entry = entry.map_err(|_| build_internal_error!(format!("Unable to get ns dir entry")))?;
            match entry.file_name().to_bytes() {
                b"." | b".." => continue,
                _ => {}
            };

            let path = self.root.join("ns").join(entry.file_name().to_string_lossy().as_ref());
            let ns_type = OsString::from(entry.file_name().to_string_lossy().as_ref());
            let stat = rustix::fs::statat(&dir_ns, entry.file_name(), AtFlags::empty())
                .map_err(|_| build_internal_error!(format!("Unable to stat {:?}", path)))?;

            if let Some(n) = namespaces.insert(
                ns_type.clone(),
                Namespace {
                    ns_type,
                    path,
                    identifier: stat.st_ino,
                    device_id: stat.st_dev,
                },
            ) {
                return Err(build_internal_error!(format!(
                    "NsType appears more than once {:?}",
                    n.ns_type
                )));
            }
        }

        Ok(namespaces)
    }
}

/// Information about a namespace
///
/// See also the [Process::namespaces()] method
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde1", derive(Serialize, Deserialize))]
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
