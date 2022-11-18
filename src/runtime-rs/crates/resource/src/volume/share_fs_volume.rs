// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{path::Path, sync::Arc};

use anyhow::{anyhow, Context, Result};

use super::Volume;
use crate::share_fs::{ShareFs, ShareFsVolumeConfig};
use kata_types::mount;

// copy file to container's rootfs if filesystem sharing is not supported, otherwise
// bind mount it in the shared directory.
// Ignore /dev, directories and all other device files. We handle
// only regular files in /dev. It does not make sense to pass the host
// device nodes to the guest.
// skip the volumes whose source had already set to guest share dir.
pub(crate) struct ShareFsVolume {
    mounts: Vec<oci::Mount>,
    storages: Vec<agent::Storage>,
}

impl ShareFsVolume {
    pub(crate) async fn new(
        share_fs: &Option<Arc<dyn ShareFs>>,
        m: &oci::Mount,
        cid: &str,
    ) -> Result<Self> {
        let file_name = Path::new(&m.source).file_name().unwrap().to_str().unwrap();
        let file_name = generate_mount_path(cid, file_name);

        let mut volume = Self {
            mounts: vec![],
            storages: vec![],
        };
        match share_fs {
            None => {
                let src = match std::fs::canonicalize(&m.source) {
                    Err(err) => {
                        return Err(anyhow!(format!(
                            "failed to canonicalize file {} {:?}",
                            &m.source, err
                        )))
                    }
                    Ok(src) => src,
                };

                if src.is_file() {
                    // TODO: copy file
                    debug!(sl!(), "FIXME: copy file {}", &m.source);
                } else {
                    debug!(
                        sl!(),
                        "Ignoring non-regular file as FS sharing not supported. mount: {:?}", m
                    );
                }
            }
            Some(share_fs) => {
                let share_fs_mount = share_fs.get_share_fs_mount();
                let mount_result = share_fs_mount
                    .share_volume(ShareFsVolumeConfig {
                        cid: cid.to_string(),
                        source: m.source.clone(),
                        target: file_name,
                        readonly: m.options.iter().any(|o| *o == "ro"),
                        mount_options: m.options.clone(),
                        mount: m.clone(),
                        is_rafs: false,
                    })
                    .await
                    .context("share fs volume")?;

                // set storages for the volume
                volume.storages = mount_result.storages;

                // set mount for the volume
                volume.mounts.push(oci::Mount {
                    destination: m.destination.clone(),
                    r#type: "bind".to_string(),
                    source: mount_result.guest_path,
                    options: m.options.clone(),
                });
            }
        }
        Ok(volume)
    }
}

impl Volume for ShareFsVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(self.mounts.clone())
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(self.storages.clone())
    }

    fn cleanup(&self) -> Result<()> {
        todo!()
    }
}

pub(crate) fn is_share_fs_volume(m: &oci::Mount) -> bool {
    (m.r#type == "bind" || m.r#type == mount::KATA_EPHEMERAL_VOLUME_TYPE)
        && !is_host_device(&m.destination)
}

fn is_host_device(dest: &str) -> bool {
    if dest == "/dev" {
        return true;
    }

    if dest.starts_with("/dev/") {
        let src = match std::fs::canonicalize(dest) {
            Err(_) => return false,
            Ok(src) => src,
        };

        if src.is_file() {
            return false;
        }

        return true;
    }

    false
}

// Note, don't generate random name, attaching rafs depends on the predictable name.
pub fn generate_mount_path(id: &str, file_name: &str) -> String {
    let mut nid = String::from(id);
    if nid.len() > 10 {
        nid = nid.chars().take(10).collect();
    }

    let mut uid = uuid::Uuid::new_v4().to_string();
    let uid_vec: Vec<&str> = uid.splitn(2, '-').collect();
    uid = String::from(uid_vec[0]);

    format!("{}-{}-{}", nid, uid, file_name)
}
