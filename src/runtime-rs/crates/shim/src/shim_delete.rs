// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use containerd_shim_protos::api;
use protobuf::Message;

use crate::{shim::ShimExecutor, Error};

impl ShimExecutor {
    pub fn delete(&mut self) -> Result<()> {
        self.args.validate(true).context("validate")?;
        let rsp = self.do_cleanup().context("do cleanup")?;
        rsp.write_to_writer(&mut std::io::stdout())
            .context(Error::FileWrite(format!("write {:?} to stdout", rsp)))?;
        Ok(())
    }

    fn do_cleanup(&self) -> Result<api::DeleteResponse> {
        let mut rsp = api::DeleteResponse::new();
        rsp.set_exit_status(128 + libc::SIGKILL as u32);
        let mut exited_time = protobuf::well_known_types::Timestamp::new();
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(Error::SystemTime)?
            .as_secs() as i64;
        exited_time.set_seconds(seconds);
        rsp.set_exited_at(exited_time);

        service::ServiceManager::cleanup(&self.args.id).context("cleanup")?;
        Ok(rsp)
    }
}
