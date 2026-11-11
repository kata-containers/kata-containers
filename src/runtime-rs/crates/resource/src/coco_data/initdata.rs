// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use hypervisor::BlockConfigModern;
use kata_types::prefix_with_rootless_dir;

/// The path /run/kata-containers/shared/initdata, combined with the sandbox ID,
/// will form the default directory for storing the initdata image.
pub const DEFAULT_KATA_SHARED_INIT_DATA_PATH: &str = "/run/kata-containers/shared/initdata";

/// kata initdata image
pub const KATA_INIT_DATA_IMAGE: &str = "initdata.image";

/// InitDataConfig which is a tuple of Block Device Config and its digest of the encoded
/// string included in the disk. And, both of them will come up at the same time.
#[derive(Clone, Debug)]
pub struct InitDataConfig(pub BlockConfigModern, pub String);

/// The path /run/kata-containers/shared/initdata, combined with the sandbox ID,
/// will form the directory for storing the initdata image.
/// The directory will be prefixed with the rootless directory when running in rootless mode
pub fn kata_shared_init_data_path() -> String {
    prefix_with_rootless_dir(DEFAULT_KATA_SHARED_INIT_DATA_PATH)
}

/// Remove the directory holding a sandbox's initdata image. The path derives
/// from the sandbox ID, so cleanup needs no record to find it, and a sandbox
/// that never had initdata has nothing there.
pub fn remove_initdata_dir(sid: &str) -> Result<()> {
    remove_sandbox_dir(Path::new(&kata_shared_init_data_path()), sid)
}

fn remove_sandbox_dir(base: &Path, sid: &str) -> Result<()> {
    // An empty ID would name the directory every sandbox's image lives under.
    if sid.is_empty() {
        return Err(anyhow!("no sandbox ID to remove the initdata directory of"));
    }
    let dir = base.join(sid);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("remove {}", dir.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_initdata_directory_is_removed_once_and_never_the_base() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("sid");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join(KATA_INIT_DATA_IMAGE), b"initdata").unwrap();

        remove_sandbox_dir(base.path(), "sid").unwrap();
        assert!(!dir.exists());
        // A retry, or a sandbox without initdata, finds nothing to remove.
        remove_sandbox_dir(base.path(), "sid").unwrap();

        remove_sandbox_dir(base.path(), "").unwrap_err();
        assert!(base.path().exists());
    }
}
