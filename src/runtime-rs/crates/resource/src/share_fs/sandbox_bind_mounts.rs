// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
// Note:
// sandbox_bind_mounts supports kinds of mount patterns, for example:
// (1) "/path/to", with default readonly mode.
// (2) "/path/to:ro", same as (1).
// (3) "/path/to:rw", with readwrite mode.
//
// sandbox_bind_mounts: ["/path/to", "/path/to:rw", "/mnt/to:ro"]
//

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};

use super::utils::{do_get_host_path, mkdir_with_permissions};
use kata_sys_util::{fs::get_base_name, mount};
use kata_types::mount::{SANDBOX_BIND_MOUNTS_DIR, SANDBOX_BIND_MOUNTS_RO, SANDBOX_BIND_MOUNTS_RW};
use nix::mount::MsFlags;

#[derive(Clone, Default, Debug)]
pub struct SandboxBindMounts {
    sid: String,
    host_mounts_path: PathBuf,
    sandbox_bindmounts: Vec<String>,
}

impl SandboxBindMounts {
    pub fn new(sid: String, sandbox_bindmounts: Vec<String>) -> Result<Self> {
        // /run/kata-containers/shared/sandboxes/<sid>/rw/passthrough/sandbox-mounts
        let bindmounts_path =
            do_get_host_path(SANDBOX_BIND_MOUNTS_DIR, sid.as_str(), "", true, false);
        let host_mounts_path = PathBuf::from(bindmounts_path);

        Ok(SandboxBindMounts {
            sid,
            host_mounts_path,
            sandbox_bindmounts,
        })
    }

    fn parse_sandbox_bind_mounts<'a>(&self, bindmnt_src: &'a str) -> Result<(&'a str, &'a str)> {
        // get the bindmount's r/w mode
        let bindmount_mode = if bindmnt_src.ends_with(SANDBOX_BIND_MOUNTS_RW) {
            SANDBOX_BIND_MOUNTS_RW
        } else {
            SANDBOX_BIND_MOUNTS_RO
        };

        // get the true bindmount from the string
        let bindmount = bindmnt_src.trim_end_matches(bindmount_mode);

        Ok((bindmount_mode, bindmount))
    }

    pub fn setup_sandbox_bind_mounts(&self) -> Result<()> {
        let mut mounted_list: Vec<PathBuf> = Vec::new();
        let mut mounted_map: HashMap<String, bool> = HashMap::new();
        for src in &self.sandbox_bindmounts {
            let (bindmount_mode, bindmount) = self
                .parse_sandbox_bind_mounts(src)
                .context("parse sandbox bind mounts failed")?;

            // get the basename of the canonicalized mount path mnt_name: dirX
            let mnt_name = get_base_name(bindmount)?
                .into_string()
                .map_err(|e| anyhow!("failed to get base name {:?}", e))?;

            // if repeated mounted, do umount it and return error
            if mounted_map.insert(mnt_name.clone(), true).is_some() {
                for p in &mounted_list {
                    nix::mount::umount(p)
                        .context("mounted_map insert one repeated mounted, do umount it")?;
                }

                return Err(anyhow!(
                    "sandbox-bindmounts: path {} is already specified.",
                    bindmount
                ));
            }

            // mount_dest: /run/kata-containers/shared/sandboxes/<sid>/rw/passthrough/sandbox-mounts/dirX
            let mount_dest = self.host_mounts_path.clone().join(mnt_name.as_str());
            mkdir_with_permissions(self.host_mounts_path.clone().to_path_buf(), 0o750).context(
                format!(
                    "create host mounts path {:?}",
                    self.host_mounts_path.clone()
                ),
            )?;

            info!(
                sl!(),
                "sandbox-bindmounts mount_src: {:?} => mount_dest: {:?}", bindmount, &mount_dest
            );

            // mount -o bind,ro host_shared mount_dest
            // host_shared: ${bindmount}
            mount::bind_mount_unchecked(Path::new(bindmount), &mount_dest, true, MsFlags::MS_SLAVE)
                .inspect_err(|_| {
                    for p in &mounted_list {
                        nix::mount::umount(p).unwrap_or_else(|e| {
                            error!(sl!(), "do umount failed: {:?}", e);
                        });
                    }
                })?;

            // default sandbox bind mounts mode is ro.
            if bindmount_mode == SANDBOX_BIND_MOUNTS_RO {
                info!(sl!(), "sandbox readonly bind mount.");
                // dest_ro: /run/kata-containers/shared/sandboxes/<sid>/ro/passthrough/sandbox-mounts
                let mount_dest_ro =
                    do_get_host_path(SANDBOX_BIND_MOUNTS_DIR, &self.sid, "", true, true);
                let sandbox_bindmounts_ro = [mount_dest_ro, mnt_name.clone()].join("/");

                mount::bind_remount(sandbox_bindmounts_ro, true)
                    .context("remount ro directory with ro permission")?;
            }

            mounted_list.push(mount_dest);
        }

        Ok(())
    }

    pub fn cleanup_sandbox_bind_mounts(&self) -> Result<()> {
        for src in &self.sandbox_bindmounts {
            let parsed_mnts = self
                .parse_sandbox_bind_mounts(src)
                .context("parse sandbox bind mounts")?;

            let mnt_name = get_base_name(parsed_mnts.1)?
                .into_string()
                .map_err(|e| anyhow!("failed to convert to string{:?}", e))?;

            // /run/kata-containers/shared/sandboxes/<sid>/passthrough/rw/sandbox-mounts/dir
            let mnt_dest = self.host_mounts_path.join(mnt_name.as_str());
            mount::umount_timeout(mnt_dest, 0).context("umount bindmount failed")?;
        }

        let is_dir = match fs::metadata(&self.host_mounts_path) {
            Ok(md) => md.is_dir(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
        };
        if is_dir {
            fs::remove_dir_all(self.host_mounts_path.clone()).context(format!(
                "remove sandbox bindmount point {:?}.",
                self.host_mounts_path.clone()
            ))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{in_private_mount_namespace, is_mounted, set_immutable, SharedTmpfs};

    #[test]
    #[ignore = "requires root and mount namespace capabilities"]
    fn test_cleanup_retry_after_partial_failure() {
        if !in_private_mount_namespace(
            "share_fs::sandbox_bind_mounts::tests::test_cleanup_retry_after_partial_failure",
        ) {
            return;
        }
        let tmpfs = SharedTmpfs::new();
        let sid = tmpfs.sandbox();
        let mut sources = vec![];
        for name in ["ro-src", "rw-src"] {
            let source = tmpfs.path().join(name);
            fs::create_dir(&source).unwrap();
            fs::write(source.join("sentinel"), name).unwrap();
            sources.push(source);
        }
        let bind_mounts = SandboxBindMounts::new(
            sid,
            vec![
                sources[0].display().to_string(),
                format!("{}{}", sources[1].display(), SANDBOX_BIND_MOUNTS_RW),
            ],
        )
        .unwrap();
        bind_mounts.setup_sandbox_bind_mounts().unwrap();
        let targets = ["ro-src", "rw-src"].map(|n| bind_mounts.host_mounts_path.join(n));
        assert!(targets.iter().all(is_mounted));

        set_immutable(&bind_mounts.host_mounts_path, true);
        let result = bind_mounts.cleanup_sandbox_bind_mounts();
        set_immutable(&bind_mounts.host_mounts_path, false);
        assert!(result.is_err());
        assert!(!targets.iter().any(is_mounted));

        bind_mounts.cleanup_sandbox_bind_mounts().unwrap();
        assert!(!bind_mounts.host_mounts_path.exists());
        for source in &sources {
            assert!(source.join("sentinel").exists());
        }

        bind_mounts.cleanup_sandbox_bind_mounts().unwrap();
    }
}
