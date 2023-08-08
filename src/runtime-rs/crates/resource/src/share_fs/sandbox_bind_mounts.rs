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
                .map_err(|e| {
                    for p in &mounted_list {
                        nix::mount::umount(p).unwrap_or_else(|x| {
                            format!("do umount failed: {:?}", x);
                        });
                    }
                    e
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

        if fs::metadata(self.host_mounts_path.clone())?.is_dir() {
            fs::remove_dir_all(self.host_mounts_path.clone()).context(format!(
                "remove sandbox bindmount point {:?}.",
                self.host_mounts_path.clone()
            ))?;
        }

        Ok(())
    }
}
