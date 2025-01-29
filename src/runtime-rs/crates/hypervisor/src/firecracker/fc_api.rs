//Copyright (c) 2019-2022 Alibaba Cloud
//Copyright (c) 2023 Nubificus Ltd
//
//SPDX-License-Identifier: Apache-2.0

use crate::{
    firecracker::{
        inner_hypervisor::{FC_AGENT_SOCKET_NAME, ROOT},
        sl, FcInner,
    },
    kernel_param::KernelParams,
    NetworkConfig, Param,
};
use anyhow::{anyhow, Context, Result};
use dbs_utils::net::MacAddr;
use hyper::{Body, Method, Request, Response};
use hyperlocal::Uri;
use kata_sys_util::mount;
use nix::mount::MsFlags;
use serde_json::json;
use tokio::{fs, fs::File};

const REQUEST_RETRY: u32 = 500;
const FC_KERNEL: &str = "vmlinux";
const FC_ROOT_FS: &str = "rootfs";
const DRIVE_PREFIX: &str = "drive";
const DISK_POOL_SIZE: u32 = 6;

impl FcInner {
    pub(crate) fn get_resource(&self, src: &str, dst: &str) -> Result<String> {
        if self.jailed {
            self.jail_resource(src, dst)
        } else {
            Ok(src.to_string())
        }
    }

    fn jail_resource(&self, src: &str, dst: &str) -> Result<String> {
        if src.is_empty() || dst.is_empty() {
            return Err(anyhow!("invalid param src {} dst {}", src, dst));
        }

        let jailed_location = [self.vm_path.as_str(), ROOT, dst].join("/");
        mount::bind_mount_unchecked(src, jailed_location.as_str(), false, MsFlags::MS_SLAVE)
            .context("bind_mount ERROR")?;

        let mut abs_path = String::from("/");
        abs_path.push_str(dst);
        Ok(abs_path)
    }

    // Remounting jailer root to ensure it has exec permissions, since firecracker binary will
    // execute from there
    pub(crate) async fn remount_jailer_with_exec(&self) -> Result<()> {
        let localpath = [self.vm_path.clone(), ROOT.to_string()].join("/");
        let _ = fs::create_dir_all(&localpath)
            .await
            .context(format!("failed to create directory {:?}", &localpath));
        mount::bind_mount_unchecked(&localpath, &localpath, false, MsFlags::MS_SHARED)
            .context("bind mount jailer root")?;

        mount::bind_remount(&localpath, false).context("rebind mount jailer root")?;
        Ok(())
    }

    pub(crate) async fn prepare_hvsock(&mut self) -> Result<()> {
        let rel_uds_path = match self.jailed {
            false => [self.vm_path.as_str(), FC_AGENT_SOCKET_NAME].join("/"),
            true => FC_AGENT_SOCKET_NAME.to_string(),
        };

        let body_vsock: String = json!({
            "guest_cid": 3,
            "uds_path": rel_uds_path,
            "vsock_id": ROOT,
        })
        .to_string();

        self.request_with_retry(Method::PUT, "/vsock", body_vsock)
            .await?;
        Ok(())
    }

    pub(crate) async fn prepare_vmm_resources(&mut self) -> Result<()> {
        let mut kernel_params = KernelParams::new(self.config.debug_info.enable_debug);
        kernel_params.push(Param::new("pci", "off"));
        kernel_params.push(Param::new("iommu", "off"));
        let rootfs_driver = self.config.blockdev_info.block_device_driver.clone();

        kernel_params.append(&mut KernelParams::new_rootfs_kernel_params(
            &rootfs_driver,
            &self.config.boot_info.rootfs_type,
        )?);
        kernel_params.append(&mut KernelParams::from_string(
            &self.config.boot_info.kernel_params,
        ));
        let mut parameters = String::new().to_owned();

        if let Ok(param) = &kernel_params.to_string() {
            parameters.push_str(&param.to_string());
        }

        let kernel = self
            .get_resource(&self.config.boot_info.kernel, FC_KERNEL)
            .context("get resource KERNEL")?;
        let rootfs = self
            .get_resource(&self.config.boot_info.image, FC_ROOT_FS)
            .context("get resource ROOTFS")?;

        let body_config: String = json!({
            "mem_size_mib": self.config.memory_info.default_memory,
            "vcpu_count": self.config.cpu_info.default_vcpus,
        })
        .to_string();
        let body_kernel: String = json!({
            "kernel_image_path": kernel,
            "boot_args": parameters,
        })
        .to_string();

        let body_rootfs: String = json!({
            "drive_id": "rootfs",
            "path_on_host": rootfs,
            "is_root_device": false,
            "is_read_only": true
        })
        .to_string();

        info!(sl(), "Before first request");
        self.request_with_retry(Method::PUT, "/boot-source", body_kernel)
            .await?;
        self.request_with_retry(Method::PUT, "/machine-config", body_config)
            .await?;
        self.request_with_retry(Method::PUT, "/drives/rootfs", body_rootfs)
            .await?;

        let abs_path = [&self.vm_path, ROOT].join("/");

        let rel_path = "/".to_string();
        let _ = fs::create_dir_all(&abs_path)
            .await
            .context(format!("failed to create directory {:?}", &abs_path));

        // We create some placeholder drives to be used for patching block devices while the vmm is
        // running, as firecracker does not support device hotplug.
        for i in 1..DISK_POOL_SIZE {
            let full_path_name = format!("{}/drive{}", abs_path, i);

            let _ = File::create(&full_path_name)
                .await
                .context(format!("failed to create file {:?}", &full_path_name));

            let path_on_host = match self.jailed {
                false => abs_path.clone(),
                true => rel_path.clone(),
            };
            let body: String = json!({
                "drive_id": format!("drive{}",i),
                "path_on_host": format!("{}/drive{}", path_on_host, i),
                "is_root_device": false,
                "is_read_only": false
            })
            .to_string();

            self.request_with_retry(Method::PUT, &format!("/drives/drive{}", i), body)
                .await?;
        }

        Ok(())
    }
    pub(crate) async fn patch_container_rootfs(
        &mut self,
        drive_id: &str,
        drive_path: &str,
    ) -> Result<()> {
        let new_drive_id = &[DRIVE_PREFIX, drive_id].concat();
        let new_drive_path = self
            .get_resource(drive_path, new_drive_id)
            .context("get resource CONTAINER ROOTFS")?;
        let body: String = json!({
            "drive_id": format!("drive{drive_id}"),
            "path_on_host": new_drive_path
        })
        .to_string();
        self.request_with_retry(
            Method::PATCH,
            &["/drives/", &format!("drive{drive_id}")].concat(),
            body,
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn add_net_device(
        &mut self,
        config: &NetworkConfig,
        device_id: String,
    ) -> Result<()> {
        let g_mac = match &config.guest_mac {
            Some(mac) => MacAddr::from_bytes(&mac.0).ok(),
            None => None,
        };
        let body: String = json!({
            "iface_id": &device_id,
            "guest_mac": g_mac,
            "host_dev_name": &config.host_dev_name

        })
        .to_string();
        self.request_with_retry(
            Method::PUT,
            &["/network-interfaces/", &device_id].concat(),
            body,
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn request_with_retry(
        &self,
        method: Method,
        uri: &str,
        data: String,
    ) -> Result<()> {
        let url: hyper::Uri = Uri::new(&self.asock_path, uri).into();
        self.send_request_with_retry(method, url, data).await
    }

    pub(crate) async fn send_request_with_retry(
        &self,
        method: Method,
        uri: hyper::Uri,
        data: String,
    ) -> Result<()> {
        debug!(sl(), "METHOD: {:?}", method.clone());
        debug!(sl(), "URI: {:?}", uri.clone());
        debug!(sl(), "DATA: {:?}", data.clone());
        for _count in 0..REQUEST_RETRY {
            let req = Request::builder()
                .method(method.clone())
                .uri(uri.clone())
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .body(Body::from(data.clone()))?;

            match self.send_request(req).await {
                Ok(resp) => {
                    debug!(sl(), "Request sent, resp: {:?}", resp);
                    return Ok(());
                }
                Err(resp) => {
                    debug!(sl(), "Request sent with error, resp: {:?}", resp);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
            }
        }
        Err(anyhow::anyhow!(
            "After {} attempts, it still doesn't work.",
            REQUEST_RETRY
        ))
    }

    pub(crate) async fn send_request(&self, req: Request<Body>) -> Result<Response<Body>> {
        let resp = self.client.request(req).await?;

        let status = resp.status();
        debug!(sl(), "Request RESPONSE {:?} {:?}", &status, resp);
        if status.is_success() {
            return Ok(resp);
        } else {
            let body = hyper::body::to_bytes(resp.into_body()).await?;
            if body.is_empty() {
                debug!(sl(), "Request FAILED WITH STATUS: {:?}", status);
                None
            } else {
                let body = String::from_utf8_lossy(&body).into_owned();
                debug!(
                    sl(),
                    "Request FAILED WITH STATUS: {:?} and BODY: {:?}", status, body
                );
                Some(body)
            };
        }

        Err(anyhow::anyhow!(
            "After {} attempts, it
                            still doesn't work.",
            REQUEST_RETRY
        ))
    }
    pub(crate) fn cleanup_resource(&self) {
        if self.jailed {
            self.umount_jail_resource(FC_KERNEL).ok();
            self.umount_jail_resource(FC_ROOT_FS).ok();

            for i in 1..DISK_POOL_SIZE {
                self.umount_jail_resource(&[DRIVE_PREFIX, &i.to_string()].concat())
                    .ok();
            }

            self.umount_jail_resource("").ok();
        }
        std::fs::remove_dir_all(self.vm_path.as_str())
            .map_err(|err| {
                error!(
                    sl(),
                    "failed to remove dir all for {} with error: {:?}", &self.vm_path, &err
                );
                err
            })
            .ok();
    }

    pub(crate) fn umount_jail_resource(&self, jailed_path: &str) -> Result<()> {
        let path = match jailed_path {
            // Handle final case to umount the bind-mounted `/run/kata/firecracker/{id}/root` dir
            "" => [self.vm_path.clone(), ROOT.to_string()].join("/"),
            // Handle generic case to umount the bind-mounted
            // `/run/kata/firecracker/{id}/root/asset` file/dir
            _ => [
                self.vm_path.clone(),
                ROOT.to_string(),
                jailed_path.to_string(),
            ]
            .join("/"),
        };
        nix::mount::umount2(path.as_str(), nix::mount::MntFlags::MNT_DETACH)
            .with_context(|| format!("umount path {}", &path))
    }
}
