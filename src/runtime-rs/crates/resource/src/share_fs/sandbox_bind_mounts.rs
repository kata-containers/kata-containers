// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// @extra-virtiofs Defination
// @Annotation: io.katacontainers.config.hypervisor.extra_virtiofs
// @ContentFormat:
// <virtiofs_device_01>:<arg01,arg02,...>;<virtiofs_device_02>:<arg01,arg02,...>;...
// @Example:
// "virtiofs_device_01:-o no_open,cache=none --thread-pool-size=1;virtiofs_device_02:-o no_open,cache=always,writeback,cache_symlinks --thread-pool-size=10"

// @sandbox_bind_mounts
// @Annotation：io.katacontainers.config.runtime.sandbox_bind_mounts
// @ContentFormat:
// <virtiofs_device_01>:<host_path01:ro host_path02 ...>;<virtiofs_device_02>:<host_path03:rw host_path04 ...>; <host_path05:rw host_path06:ro ...>
// @sandbox_bind_mounts with extra_virtiofs
//    --annotation "io.katacontainers.config.hypervisor.extra_virtiofs=..." \
//    --annotation "io.katacontainers.config.runtime.sandbox_bind_mounts=..."
//

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};

use super::utils::mkdir_with_permissions;
use crate::share_fs::utils::get_host_shared_subpath;
use kata_sys_util::{fs::get_base_name, mount};
use kata_types::mount::{SANDBOX_BIND_MOUNTS_DIR, SANDBOX_BIND_MOUNTS_RO, SANDBOX_BIND_MOUNTS_RW};
use nix::mount::MsFlags;

#[derive(Clone, Default, Debug)]
pub struct SandboxBindMounts {
    sandbox_id: String,
    sandbox_bindmounts: Vec<String>,
}

impl SandboxBindMounts {
    pub fn new(sandbox_id: String, sandbox_bindmounts: Vec<String>) -> Result<Self> {
        Ok(SandboxBindMounts {
            sandbox_id,
            sandbox_bindmounts,
        })
    }

    // return Result<(Option(virtiofs device), host path, r/w mode)>
    fn parse_sandbox_bind_mounts<'a>(
        &self,
        sandbox_bindmount: &'a str,
    ) -> Result<(Option<&'a str>, &'a str, &'a str)> {
        // get the bindmount's r/w mode
        let bindmount_mode = if sandbox_bindmount.ends_with(SANDBOX_BIND_MOUNTS_RW) {
            SANDBOX_BIND_MOUNTS_RW
        } else {
            SANDBOX_BIND_MOUNTS_RO
        };

        // get the real bindmount from the string
        // virtiofs_device:/path/to2@rw -> virtiofs_device:/path/to2
        let dev_bindmount = sandbox_bindmount.trim_end_matches(bindmount_mode);
        let dev_and_path: Vec<&str> = dev_bindmount.split(':').collect();
        if dev_and_path.len() == 2 {
            Ok((Some(dev_and_path[0]), dev_and_path[1], bindmount_mode))
        } else {
            Ok((None, dev_and_path[0], bindmount_mode))
        }
    }

    pub fn setup_sandbox_bind_mounts(&self) -> Result<()> {
        let mut mounted_list: Vec<PathBuf> = Vec::new();
        let mut mounted_map: HashMap<String, bool> = HashMap::new();

        for src in &self.sandbox_bindmounts {
            // (Some(virtiofs1), /mnt/to, @rw)
            let (virtiofs_dev, host_shared, bindmount_mode) =
                self.parse_sandbox_bind_mounts(src)
                    .context("parse sandbox bind mounts")?;

            // get the basename of the canonicalized mount path mnt_name: dirX
            let mnt_name = get_base_name(host_shared)?
                .into_string()
                .map_err(|e| anyhow!("failed to convert to string{:?}", e))?;

            // if repeated mounted, do umount it and return error
            if mounted_map.insert(mnt_name.clone(), true).is_some() {
                for p in &mounted_list {
                    nix::mount::umount(p).context("one repeated mount, do umount it")?;
                }

                return Err(anyhow!(
                    "sandbox-bindmounts: path {} is already specified.",
                    host_shared
                ));
            }

            // /run/kata-containers/shared/sandboxes/<sid>/<virtiofs_device>/rw/passthrough/sandbox-mounts/dirX
            let host_mounts_target = get_host_shared_subpath(
                self.sandbox_id.as_str(),
                virtiofs_dev,
                SANDBOX_BIND_MOUNTS_DIR,
                false,
            )
            .join(&mnt_name);
            mkdir_with_permissions(host_mounts_target.clone(), 0o750)
                .context(format!("create host mounts path {:?}", host_mounts_target))?;

            info!(
                sl!(),
                "sandbox-bindmounts mount_src: {:?} => mount_dest: {:?}",
                host_shared,
                &host_mounts_target
            );

            // mount -o bind,ro host_shared mount_dest
            // host_shared: ${bindmount}
            mount::bind_mount_unchecked(
                Path::new(host_shared),
                &host_mounts_target,
                true,
                MsFlags::MS_SLAVE,
            )
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
                // /run/kata-containers/shared/sandboxes/<sid>/<virtiofs_device>/ro/passthrough/sandbox-mounts/dirX
                let bindmount_ro = get_host_shared_subpath(
                    self.sandbox_id.as_str(),
                    virtiofs_dev,
                    SANDBOX_BIND_MOUNTS_DIR,
                    true,
                )
                .join(mnt_name);

                mount::bind_remount(&bindmount_ro, true)
                    .context("remount ro directory with ro permission")?;
            }

            mounted_list.push(host_mounts_target);
        }

        Ok(())
    }

    pub fn cleanup_sandbox_bind_mounts(&self) -> Result<()> {
        let mut bindmounts_cleanup: Vec<PathBuf> = Vec::new();
        for src in &self.sandbox_bindmounts {
            // (Some(virtiofs1), /mnt/to, @rw)
            let (virtiofs_dev, host_shared, _mode) = self
                .parse_sandbox_bind_mounts(src)
                .context("parse sandbox bind mounts")?;

            // get the basename of the canonicalized mount path mnt_name: dirX
            let mnt_name = get_base_name(host_shared)?
                .into_string()
                .map_err(|e| anyhow!("failed to convert to string{:?}", e))?;

            // /run/kata-containers/shared/sandboxes/<sid>/<virtiofs_device>/rw/passthrough/sandbox-mounts/dirX
            let host_sandbox_mounts = get_host_shared_subpath(
                self.sandbox_id.as_str(),
                virtiofs_dev,
                SANDBOX_BIND_MOUNTS_DIR,
                false,
            );
            bindmounts_cleanup.push(host_sandbox_mounts.clone());

            let host_mounts_target = host_sandbox_mounts.join(&mnt_name);
            mount::umount_timeout(&host_mounts_target, 0).context("umount bindmount failed")?;
        }

        for bindmount in bindmounts_cleanup.iter() {
            if fs::metadata(bindmount)?.is_dir() {
                fs::remove_dir_all(bindmount)
                    .context(format!("remove sandbox bindmount point {:?}.", bindmount))?;
            }
        }

        Ok(())
    }
}
