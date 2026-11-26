// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

impl ContainerInner {
    pub(crate) async fn discard_unstarted_container(
        &mut self,
        cid: &str,
        device_manager: &RwLock<DeviceManager>,
    ) -> Result<()> {
        if !self.volumes.is_empty() || self.rootfs.len() != 1 {
            return Err(anyhow!(
                "restored container {cid} has unexpected resource inventory: {} volumes, {} rootfs objects",
                self.volumes.len(),
                self.rootfs.len()
            ));
        }

        let remove_request = agent::RemoveContainerRequest {
            container_id: cid.to_string(),
        };

        if let Err(err) = self
            .agent
            .remove_container(remove_request)
            .await
            .map_err(convert_agent_error)
        {
            if !matches!(
                err.downcast_ref::<Error>(),
                Some(Error::ProcessAlreadyTerminated)
            ) && !is_no_such_process_error(&err)
            {
                return Err(err).context("remove unstarted restored container from guest");
            }
        }

        self.init_process.stop().await;
        self.rootfs[0]
            .cleanup(device_manager)
            .await
            .context("remove restored rootfs device")?;
        self.rootfs.clear();

        Ok(())
    }
}
