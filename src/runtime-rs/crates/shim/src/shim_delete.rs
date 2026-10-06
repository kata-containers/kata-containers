// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use containerd_shim_protos::api;
use kata_sys_util::spec::{get_bundle_path, get_container_type, load_oci_spec};
use kata_types::container::ContainerType;
use nix::{sys::signal::kill, sys::signal::SIGKILL, unistd::Pid};
use protobuf::Message;
use std::{fs, io::Write, path::Path};

use crate::{logger, shim::ShimExecutor, Error};

impl ShimExecutor {
    pub async fn delete(&mut self) -> Result<()> {
        let _logger_guard = match logger::set_logger("", &self.args.id, self.args.debug) {
            Ok(guard) => Some(guard),
            Err(err) => {
                eprintln!("failed to set the delete logger, errors go to stderr only: {err:#}");
                None
            }
        };
        logging::register_subsystem_logger("runtimes", "shim");

        self.do_delete()
            .await
            .inspect_err(|err| error!(sl!(), "failed to delete shim: {err:?}"))
    }

    async fn do_delete(&mut self) -> Result<()> {
        self.args.validate(true).context("validate")?;
        let rsp = self.do_cleanup().await.context("shim do cleanup")?;
        // Drop ignores buffered flush errors, so flush explicitly while errors can be reported.
        let mut stdout = std::io::stdout().lock();
        rsp.write_to_writer(&mut stdout)
            .context(Error::FileWrite(format!("write {rsp:?} to stdout")))?;
        stdout
            .flush()
            .context(Error::FileWrite("flush stdout".to_owned()))?;
        Ok(())
    }

    async fn do_cleanup(&self) -> Result<api::DeleteResponse> {
        let mut rsp = api::DeleteResponse::new();
        rsp.set_exit_status(128 + libc::SIGKILL as u32);
        let mut exited_time = protobuf::well_known_types::timestamp::Timestamp::new();
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(Error::SystemTime)?
            .as_secs() as i64;
        exited_time.seconds = seconds;
        rsp.set_exited_at(exited_time);

        let address = self
            .socket_address(&self.args.id)
            .context("socket address")?;
        let trim_path = address.strip_prefix("unix://").context("trim path")?;
        let file_path = Path::new("/").join(trim_path);
        let file_path = file_path.as_path();
        if std::fs::metadata(file_path).is_ok() {
            info!(sl!(), "remote socket path: {:?}", &file_path);
            fs::remove_file(file_path).ok();
        }

        if let Err(e) = service::ServiceManager::cleanup(&self.args.id).await {
            error!(
                sl!(),
                "failed to cleanup in service manager: {:?}. force shutdown shim process", e
            );

            let bundle_path = get_bundle_path().context("get bundle path")?;
            if let Ok(spec) = load_oci_spec() {
                if let Ok(ContainerType::PodSandbox) = get_container_type(&spec) {
                    // only force shutdown for sandbox container
                    if let Ok(shim_pid) = self.read_pid_file(&bundle_path) {
                        info!(sl!(), "force to shutdown shim process {}", shim_pid);
                        let pid = Pid::from_raw(shim_pid as i32);
                        if let Err(_e) = kill(pid, SIGKILL) {
                            // ignore kill errors
                        }
                    }
                }
            }
        }

        Ok(rsp)
    }
}
