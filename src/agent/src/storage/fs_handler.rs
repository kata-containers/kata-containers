// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::storage::{common_storage_handler, new_device, StorageContext, StorageHandler};
use anyhow::{anyhow, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use kata_types::device::{
    DRIVER_OVERLAYFS_TYPE, DRIVER_SMB_TYPE, DRIVER_VIRTIOFS_TYPE, SMB_KRB5_CCACHE_DRIVER_OPTION,
};
use kata_types::mount::{StorageDevice, KATA_VOLUME_OVERLAYFS_CREATE_DIR};
use protocols::agent::Storage;
use tracing::instrument;

#[derive(Debug)]
pub struct OverlayfsHandler {}

fn ensure_directory_exists(path: &Path) -> Result<()> {
    match fs::create_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        Err(err) if err.raw_os_error() == Some(libc::ENOSYS) => {
            if path.is_dir() {
                Ok(())
            } else {
                Err(err).context(format!(
                    "failed to create overlay directory {}: filesystem does not support mkdir",
                    path.display()
                ))
            }
        }
        Err(err) => Err(err).context(format!(
            "failed to create overlay directory {}",
            path.display()
        )),
    }
}

#[async_trait::async_trait]
impl StorageHandler for OverlayfsHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_OVERLAYFS_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        if storage
            .options
            .iter()
            .any(|e| e == "io.katacontainers.fs-opt.overlay-rw")
        {
            let cid = ctx
                .cid
                .clone()
                .ok_or_else(|| anyhow!("No container id in rw overlay"))?;
            let cpath = Path::new(crate::rpc::CONTAINER_BASE).join(cid);
            let work = cpath.join("work");
            let upper = cpath.join("upper");

            fs::create_dir_all(&work).context("Creating overlay work directory")?;
            fs::create_dir_all(&upper).context("Creating overlay upper directory")?;

            storage.fstype = "overlay".into();
            storage
                .options
                .push(format!("upperdir={}", upper.to_string_lossy()));
            storage
                .options
                .push(format!("workdir={}", work.to_string_lossy()));
        }
        let overlay_create_dir_prefix = &(KATA_VOLUME_OVERLAYFS_CREATE_DIR.to_string() + "=");
        for driver_option in &storage.driver_options {
            if let Some(dir) = driver_option
                .as_str()
                .strip_prefix(overlay_create_dir_prefix)
            {
                ensure_directory_exists(Path::new(dir))?;
            }
        }
        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

#[derive(Debug)]
pub struct VirtioFsHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioFsHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_VIRTIOFS_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

/// Mounts an SMB/cifs direct-assigned volume (e.g. Azure Files) inside the
/// guest from the direct-volume metadata relayed by the runtime. The source is
/// a UNC path (//server/share), fstype is cifs; credentials/options are carried
/// in `storage.options`.
#[derive(Debug)]
pub struct SmbHandler {}

/// Guest Kerberos cache read by cifs.upcall for uid 0; matches the guest /etc/krb5.conf.
const SMB_KRB5_CCACHE_PATH: &str = "/run/kata-containers/krb5cc_0";
const KRB5_CCACHE_VERSION_4: usize = 0x0504;

static KRB5_CCACHE_LOCK: Mutex<()> = Mutex::new(());

fn read_be(data: &[u8], off: &mut usize, len: usize) -> Result<usize> {
    let bytes = data
        .get(*off..off.saturating_add(len))
        .ok_or_else(|| anyhow!("truncated kerberos ccache"))?;
    *off += len;
    Ok(bytes.iter().fold(0, |acc, b| (acc << 8) | usize::from(*b)))
}

fn skip(data: &[u8], off: &mut usize, len: usize) -> Result<()> {
    ensure!(len <= data.len() - *off, "truncated kerberos ccache");
    *off += len;
    Ok(())
}

/// Returns the offset of the first credential in a v4 ccache (after the header and default principal).
fn krb5_ccache_creds_offset(data: &[u8]) -> Result<usize> {
    let mut off = 0;
    let version = read_be(data, &mut off, 2)?;
    ensure!(
        version == KRB5_CCACHE_VERSION_4,
        "unsupported kerberos ccache version {version:#06x}"
    );
    let header_len = read_be(data, &mut off, 2)?;
    skip(data, &mut off, header_len)?;
    read_be(data, &mut off, 4)?; // name type
    let components = read_be(data, &mut off, 4)?;
    // The realm is encoded before the components.
    for _ in 0..=components {
        let len = read_be(data, &mut off, 4)?;
        skip(data, &mut off, len)?;
    }
    Ok(off)
}

/// Adds the credentials in `ccache` to the cache at `path`, keeping tickets for other
/// servers when both caches belong to the same client.
fn store_krb5_ccache(path: &Path, ccache: &[u8]) -> Result<()> {
    let creds_off = krb5_ccache_creds_offset(ccache)?;
    let _guard = KRB5_CCACHE_LOCK
        .lock()
        .map_err(|_| anyhow!("kerberos ccache lock poisoned"))?;

    let mut merged = ccache.to_vec();
    if let Ok(existing) = fs::read(path) {
        if let Ok(off) = krb5_ccache_creds_offset(&existing) {
            if existing[..off] == ccache[..creds_off] {
                merged = existing;
                merged.extend_from_slice(&ccache[creds_off..]);
            }
        }
    }

    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("invalid kerberos ccache path"))?;
    fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(&merged)?;
    file.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[async_trait::async_trait]
impl StorageHandler for SmbHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_SMB_TYPE]
    }

    // Storage carries credentials, so keep it out of tracing spans.
    #[instrument(skip_all)]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // Temp log. will be removed in final PR.
        // +++++ marker: confirms the guest agent took the SMB/cifs path.
        info!(
            ctx.logger,
            "+++++SmbHandler: mounting cifs direct volume";
            "source" => &storage.source,
            "fstype" => &storage.fstype,
            "mount-point" => &storage.mount_point,
        );
        let ccache_prefix = format!("{SMB_KRB5_CCACHE_DRIVER_OPTION}=");
        if let Some(encoded) = storage
            .driver_options
            .iter()
            .find_map(|o| o.strip_prefix(&ccache_prefix))
        {
            let ccache = STANDARD
                .decode(encoded)
                .context("decode SMB kerberos ccache")?;
            store_krb5_ccache(Path::new(SMB_KRB5_CCACHE_PATH), &ccache)
                .context("store SMB kerberos ccache")?;
            info!(ctx.logger, "+++++SmbHandler: stored kerberos ccache";
                "path" => SMB_KRB5_CCACHE_PATH);
        }
        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use tempfile::tempdir;

    // v4 ccache with default principal "user@R" followed by `creds`.
    fn ccache(creds: &[u8]) -> Vec<u8> {
        let mut data = vec![0x05, 0x04, 0x00, 0x00];
        data.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 1]); // name type, 1 component
        data.extend_from_slice(&[0, 0, 0, 1, b'R', 0, 0, 0, 4]);
        data.extend_from_slice(b"user");
        data.extend_from_slice(creds);
        data
    }

    #[test]
    fn test_krb5_ccache_creds_offset() {
        assert_eq!(krb5_ccache_creds_offset(&ccache(b"cred")).unwrap(), 25);
        assert!(krb5_ccache_creds_offset(&[0x05, 0x03, 0, 0]).is_err());
        assert!(krb5_ccache_creds_offset(&ccache(b"")[..20]).is_err());
    }

    #[test]
    fn test_store_krb5_ccache_merges_same_client() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("krb5cc_0");
        store_krb5_ccache(&path, &ccache(b"one")).unwrap();
        store_krb5_ccache(&path, &ccache(b"two")).unwrap();
        assert_eq!(fs::read(&path).unwrap(), ccache(b"onetwo"));
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);

        let mut other = ccache(b"three");
        other[21] = b'x'; // a different client principal replaces the cache
        store_krb5_ccache(&path, &other).unwrap();
        assert_eq!(fs::read(&path).unwrap(), other);
    }
}
