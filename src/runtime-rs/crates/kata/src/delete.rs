// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use containerd_shim_protos::shim::shim::DeleteResponse;
use protobuf::Message;
use virtcontainers::Sandbox;

use crate::{to_timestamp, Error, Result, ShimExecutor};

impl ShimExecutor {
    // Implement delete subcommnad
    // This functions should be the most outside one for `delete` command action
    // So it does not return errors but directly write errors to stderr
    pub fn delete(&mut self) {
        if let Err(e) = self.args.validate(true) {
            eprintln!("delete shim err: {}", e);
            return;
        }

        match self.do_cleanup() {
            Ok(rsp) => {
                rsp.write_to_writer(&mut std::io::stdout())
                    .expect("failed to write stdout");
            }
            Err(e) => eprintln!("failed to delete: {}", e),
        }
    }

    fn do_cleanup(&self) -> Result<DeleteResponse> {
        let exited_time = to_timestamp(std::time::SystemTime::now())?;
        let rsp = DeleteResponse {
            exit_status: 128 + libc::SIGKILL as u32,
            exited_at: Some(exited_time).into(),
            ..Default::default()
        };

        let id = &self.args.id;
        Sandbox::cleanup_container(id).map_err(Error::Sandbox)?;

        Ok(rsp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShimArgs;
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_shim_delete() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_path = dir.path();
        std::env::set_current_dir(bundle_path).unwrap();

        let id: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        let namespace: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        let args = ShimArgs {
            id,
            namespace,
            address: "containerd_socket".into(),
            publish_binary: "containerd".into(),
            bundle: bundle_path.to_str().unwrap().into(),
            debug: false,
        };

        let executor = ShimExecutor::new(args);

        let resp = executor.do_cleanup().unwrap();
        //assert_eq!(resp.pid > 0);
        assert_eq!(resp.exit_status, 128 + libc::SIGKILL as u32);
        assert!(resp.exited_at.as_ref().unwrap().seconds > 0);
    }
}
