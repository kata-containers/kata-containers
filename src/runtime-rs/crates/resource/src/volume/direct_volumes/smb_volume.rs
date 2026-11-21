use std::path::PathBuf;

use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_types::{
    device::{DRIVER_SMB_TYPE, SMB_KRB5_CCACHE_DRIVER_OPTION},
    mount::DirectVolumeMountInfo,
};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::volume::{
    direct_volumes::KATA_SMB_VOLUME_TYPE,
    utils::{
        build_bind_mount_options, filter_block_storage_options, generate_shared_path,
        KATA_MOUNT_BIND_TYPE,
    },
    Volume,
};

#[derive(Clone)]
pub(crate) struct SmbVolume {
    storage: agent::Storage,
    mount: oci::Mount,
}

impl SmbVolume {
    pub(crate) async fn new(
        mount: &oci::Mount,
        mount_info: &DirectVolumeMountInfo,
        read_only: bool,
        sid: &str,
    ) -> Result<Self> {
        let mut volume = Self::from_mount_info(mount, mount_info, read_only, String::new())?;
        let volume_id = Uuid::new_v4().to_string();
        let guest_path = generate_shared_path(mount.destination().clone(), &volume_id, sid)
            .await
            .context("generate SMB guest mount path")?;
        volume.storage.mount_point = guest_path.clone();
        volume.mount.set_source(Some(PathBuf::from(guest_path)));
        info!(sl!(), "+++++SmbVolume: prepared guest CIFS storage";
            "source" => &volume.storage.source,
            "mount-point" => &volume.storage.mount_point);
        Ok(volume)
    }

    fn from_mount_info(
        mount: &oci::Mount,
        mount_info: &DirectVolumeMountInfo,
        read_only: bool,
        guest_path: String,
    ) -> Result<Self> {
        ensure!(
            mount_info.volume_type == KATA_SMB_VOLUME_TYPE,
            "invalid SMB volume type"
        );
        ensure!(
            mount_info.fs_type == "cifs",
            "SMB volume requires cifs filesystem"
        );
        ensure!(
            mount_info
                .device
                .strip_prefix("//")
                .and_then(|path| path.split_once('/'))
                .is_some_and(|(server, share)| !server.is_empty() && !share.is_empty()),
            "SMB volume requires a UNC server/share source"
        );

        let mut storage_options = filter_block_storage_options(&mount_info.options);
        if let Some(sensitive_options) = mount_info.metadata.get("sensitiveMountOptions") {
            if !sensitive_options.is_empty() {
                storage_options.push(sensitive_options.clone());
            }
        }
        if read_only && !storage_options.iter().any(|o| o == "ro") {
            storage_options.push("ro".to_string());
        }
        let driver_options = mount_info
            .metadata
            .get("krb5Ccache")
            .filter(|ccache| !ccache.is_empty())
            .map(|ccache| vec![format!("{SMB_KRB5_CCACHE_DRIVER_OPTION}={ccache}")])
            .unwrap_or_default();

        let storage = agent::Storage {
            driver: DRIVER_SMB_TYPE.to_string(),
            driver_options,
            source: mount_info.device.clone(),
            fs_type: mount_info.fs_type.clone(),
            options: storage_options,
            mount_point: guest_path.clone(),
            ..Default::default()
        };
        // CIFS options apply only to the guest storage mount, not the container bind mount.
        let mut guest_mount = oci::Mount::default();
        guest_mount.set_destination(mount.destination().clone());
        guest_mount.set_typ(Some(KATA_MOUNT_BIND_TYPE.to_string()));
        guest_mount.set_source(Some(PathBuf::from(guest_path)));
        guest_mount.set_options(Some(build_bind_mount_options(&[], mount.options())));
        Ok(Self {
            storage,
            mount: guest_mount,
        })
    }
}

#[async_trait]
impl Volume for SmbVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![self.storage.clone()])
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount_info() -> DirectVolumeMountInfo {
        serde_json::from_value(serde_json::json!({
            "volume-type": "azurefile",
            "device": "//server/share/folder",
            "fstype": "cifs",
            "metadata": {"sensitiveMountOptions": "username=account,password=key=="},
            "options": ["vers=3.0", "addr=192.0.2.1", "bind"]
        }))
        .unwrap()
    }

    #[test]
    fn test_smb_storage_and_container_bind() {
        let mut mount = oci::Mount::default();
        mount.set_destination(PathBuf::from("/mnt/data"));
        mount.set_options(Some(vec!["rbind".to_string(), "ro".to_string()]));
        let volume = SmbVolume::from_mount_info(
            &mount,
            &mount_info(),
            true,
            "/guest/smb-volume".to_string(),
        )
        .unwrap();
        let storage = volume.get_storage().unwrap().remove(0);
        assert_eq!(storage.driver, "smb");
        assert_eq!(storage.source, "//server/share/folder");
        assert_eq!(storage.fs_type, "cifs");
        assert_eq!(storage.mount_point, "/guest/smb-volume");
        assert_eq!(
            storage.options,
            vec![
                "vers=3.0",
                "addr=192.0.2.1",
                "username=account,password=key==",
                "ro",
            ]
        );
        let guest_mount = volume.get_volume_mount().unwrap().remove(0);
        assert_eq!(guest_mount.destination(), mount.destination());
        assert_eq!(
            guest_mount.source(),
            &Some(PathBuf::from("/guest/smb-volume"))
        );
        assert_eq!(guest_mount.typ().as_deref(), Some("bind"));
        assert_eq!(
            guest_mount.options(),
            &Some(vec!["rbind".to_string(), "ro".to_string()])
        );
        assert_eq!(volume.get_device_id().unwrap(), None);
    }

    #[test]
    fn test_smb_without_sensitive_options() {
        let mut info = mount_info();
        info.metadata.clear();
        let volume = SmbVolume::from_mount_info(
            &oci::Mount::default(),
            &info,
            false,
            "/guest/smb-volume".to_string(),
        )
        .unwrap();
        assert_eq!(volume.storage.options, vec!["vers=3.0", "addr=192.0.2.1"]);
        assert!(volume.storage.driver_options.is_empty());
    }

    #[test]
    fn test_smb_forwards_krb5_ccache() {
        let mut info = mount_info();
        info.metadata
            .insert("krb5Ccache".to_string(), "BQQAAA==".to_string());
        let volume = SmbVolume::from_mount_info(
            &oci::Mount::default(),
            &info,
            false,
            "/guest/smb-volume".to_string(),
        )
        .unwrap();
        assert_eq!(volume.storage.driver_options, vec!["krb5_ccache=BQQAAA=="]);
        assert!(!volume.storage.options.iter().any(|o| o.contains("BQQAAA")));
    }

    #[test]
    fn test_smb_rejects_invalid_metadata() {
        for (volume_type, fs_type, device) in [
            ("directvol", "cifs", "//server/share"),
            ("azurefile", "nfs", "//server/share"),
            ("azurefile", "cifs", "/dev/sda"),
            ("azurefile", "cifs", "//server/"),
            ("azurefile", "cifs", "///share"),
        ] {
            let mut info = mount_info();
            info.volume_type = volume_type.to_string();
            info.fs_type = fs_type.to_string();
            info.device = device.to_string();
            assert!(SmbVolume::from_mount_info(
                &oci::Mount::default(),
                &info,
                false,
                "/guest/smb-volume".to_string(),
            )
            .is_err());
        }
    }
}
