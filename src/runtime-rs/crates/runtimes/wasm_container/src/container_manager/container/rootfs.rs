use kata_agent::mount::baremount;
use kata_types::mount::Mount;

use anyhow::{anyhow, Result};
use nix::mount::{mount, MsFlags};
use oci::Root;
use std::path::Path;

// root.readonly will be handled after tmpfs like proc/dev/sys mounted
// in rustjail::mount::finish_rootfs
pub fn handle_rootfs(bundle: &Path, root: &mut Root, rootfs_mounts: &Vec<Mount>) -> Result<()> {
    let rootfs_path = bundle.join("rootfs");
    let spec_root_path = Path::new(&root.path).canonicalize()?;

    root.path = rootfs_path.as_path().display().to_string();

    match rootfs_mounts.len() {
        0 => baremount(
            spec_root_path.as_path(),
            &rootfs_path,
            "bind",
            MsFlags::MS_BIND,
            "",
            &sl!(),
        )
        .map_err(|e| anyhow!(format!("failed to mount spec.root {:?}", e))),
        1 => {
            let mount_info = &rootfs_mounts[0];

            let mut options = String::new();
            for option in &mount_info.options {
                options.push_str(format!("{},", option.clone()).as_str());
            }
            options.pop();

            mount(
                Some(mount_info.source.as_str()),
                root.path.as_str(),
                Some(mount_info.fs_type.as_str()),
                MsFlags::empty(),
                Some(options.as_str()),
            )
            .map_err(|e| anyhow!(format!("failed to mount rootfs_mounts {:?}", e)))
        }
        _ => Err(anyhow!("invalid rootfs configuration")),
    }
}
