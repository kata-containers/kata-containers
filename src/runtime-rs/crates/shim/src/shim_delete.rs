// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use containerd_shim_protos::shim::shim::DeleteResponse;
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

    fn do_cleanup(&self) -> Result<DeleteResponse> {
        let mut rsp = DeleteResponse::new();
        rsp.set_exit_status(128 + libc::SIGKILL as u32);
        let mut exited_time = protobuf::well_known_types::Timestamp::new();
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(Error::SystemTime)?
            .as_secs() as i64;
        exited_time.set_seconds(seconds);
        rsp.set_exited_at(exited_time);

        // TODO: implement cleanup
        Ok(rsp)
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use tests_utils::gen_id;

    use super::*;
    use crate::Args;

    #[test]
    #[serial]
    fn test_shim_delete() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_path = dir.path();
        std::env::set_current_dir(bundle_path).unwrap();

        let id = gen_id(16);
        let namespace = gen_id(16);
        let args = Args {
            id,
            namespace,
            address: "containerd_socket".into(),
            publish_binary: "containerd".into(),
            socket: "socket".into(),
            bundle: bundle_path.to_str().unwrap().into(),
            debug: false,
        };

        let executor = ShimExecutor::new(args);

        let resp = executor.do_cleanup().unwrap();
        assert_eq!(resp.exit_status, 128 + libc::SIGKILL as u32);
        assert!(resp.exited_at.as_ref().unwrap().seconds > 0);
    }
}
