// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use containerd_shim_protos::api;
use protobuf::Message;
use std::{fs, path::Path};

use crate::{shim::ShimExecutor, Error};

impl ShimExecutor {
    pub async fn delete(&mut self) -> Result<()> {
        self.args.validate(true).context("validate")?;
        let rsp = self.do_cleanup().await.context("do cleanup")?;
        rsp.write_to_writer(&mut std::io::stdout())
            .context(Error::FileWrite(format!("write {:?} to stdout", rsp)))?;
        Ok(())
    }

    async fn do_cleanup(&self) -> Result<api::DeleteResponse> {
        let mut rsp = api::DeleteResponse::new();
        rsp.set_exit_status(128 + libc::SIGKILL as u32);
        let mut exited_time = protobuf::well_known_types::Timestamp::new();
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(Error::SystemTime)?
            .as_secs() as i64;
        exited_time.set_seconds(seconds);
        rsp.set_exited_at(exited_time);

        let address = self
            .socket_address(&self.args.id)
            .context("socket address")?;
        let trim_path = address.strip_prefix("unix://").context("trim path")?;
        let file_path = Path::new("/").join(trim_path);
        let file_path = file_path.as_path();
        if std::fs::metadata(&file_path).is_ok() {
            info!(sl!(), "remote socket path: {:?}", &file_path);
            fs::remove_file(file_path).ok();
        }
        service::ServiceManager::cleanup(&self.args.id)
            .await
            .context("cleanup")?;
        Ok(rsp)
    }
}
