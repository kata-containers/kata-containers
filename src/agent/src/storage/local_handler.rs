// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use crate::storage::{new_device, parse_options, StorageContext, StorageHandler, MODE_SETGID};
use anyhow::{Context, Result};
use kata_types::device::DRIVER_LOCAL_TYPE;
use kata_types::mount::{StorageDevice, KATA_MOUNT_OPTION_FS_GID};
use nix::unistd::Gid;
use protocols::agent::Storage;
use tracing::instrument;

#[derive(Debug)]
pub struct LocalHandler {}

#[async_trait::async_trait]
impl StorageHandler for LocalHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_LOCAL_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        _ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        fs::create_dir_all(&storage.mount_point).context(format!(
            "failed to create dir all {:?}",
            &storage.mount_point
        ))?;

        let opts = parse_options(&storage.options);

        let mut need_set_fsgid = false;
        if let Some(fsgid) = opts.get(KATA_MOUNT_OPTION_FS_GID) {
            let gid = fsgid.parse::<u32>()?;

            nix::unistd::chown(storage.mount_point.as_str(), None, Some(Gid::from_raw(gid)))?;
            need_set_fsgid = true;
        }

        if let Some(mode) = opts.get("mode") {
            let mut permission = fs::metadata(&storage.mount_point)?.permissions();

            let mut o_mode = u32::from_str_radix(mode, 8)?;

            if need_set_fsgid {
                // set SetGid mode mask.
                o_mode |= MODE_SETGID;
            }
            permission.set_mode(o_mode);

            fs::set_permissions(&storage.mount_point, permission)?;
        }

        new_device("".to_string())
    }
}
