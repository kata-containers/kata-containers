// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
    os::unix::{fs::MetadataExt, io::RawFd},
    path::Path,
};

use anyhow::{Context, Result};
use kata_sys_util::spec::get_bundle_path;

use crate::{
    core_sched, logger,
    shim::{socket_path, ShimExecutor, ENV_KATA_RUNTIME_BIND_FD},
    Error,
};

impl ShimExecutor {
    pub async fn run(&mut self) -> Result<()> {
        crate::panic_hook::set_panic_hook();
        let sid = self.args.id.clone();
        let bundle_path = get_bundle_path().context("get bundle")?;
        let path = bundle_path.join("log");
        let _logger_guard =
            logger::set_logger(path.to_str().unwrap(), &sid, self.args.debug).context("set logger");
        // Regist shim logger for later use.
        logging::register_subsystem_logger("runtimes", "shim");

        if try_core_sched().is_err() {
            warn!(
                sl!(),
                "Failed to enable core sched since prctl() returns non-zero value."
            );
        }

        self.do_run()
            .await
            .map_err(|err| {
                error!(sl!(), "failed run shim {:?}", err);
                err
            })
            .context("run shim")?;

        Ok(())
    }

    async fn do_run(&mut self) -> Result<()> {
        info!(sl!(), "start to run");
        self.args.validate(false).context("validate")?;

        let address = self.socket_address(&self.args.id)?;
        let socket_path = socket_path(&address)?;
        let socket_id = file_id(&socket_path);

        let result = self.serve().await;
        remove_socket_if_unchanged(&socket_path, socket_id);
        result
    }

    async fn serve(&mut self) -> Result<()> {
        let server_fd = get_server_fd().context("get server fd")?;
        let service_manager = service::ServiceManager::new(
            &self.args.id,
            &self.args.publish_binary,
            &self.args.address,
            &self.args.namespace,
            server_fd,
        )
        .await
        .context("new shim server")?;
        service_manager.run().await.context("run")?;

        Ok(())
    }
}

fn file_id(path: &Path) -> Option<(u64, u64)> {
    fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

// Once this shim stops listening, a new `shim start` of the same sandbox may
// already have replaced the socket file, which must then be left alone.
fn remove_socket_if_unchanged(path: &Path, id: Option<(u64, u64)>) {
    if id.is_some() && file_id(path) == id {
        if let Err(e) = fs::remove_file(path) {
            warn!(sl!(), "failed to remove socket {:?}: {}", path, e);
        }
    }
}

fn get_server_fd() -> Result<RawFd> {
    let env_fd = std::env::var(ENV_KATA_RUNTIME_BIND_FD).map_err(Error::EnvVar)?;
    let fd = env_fd
        .parse::<RawFd>()
        .map_err(|_| Error::ServerFd(env_fd))?;
    Ok(fd)
}

// TODO: currently we log a warning on fail (i.e. kernel version < 5.14), maybe just exit
// TODO: more test on higher version of kernel
fn try_core_sched() -> Result<()> {
    if let Ok(v) = std::env::var("SCHED_CORE") {
        if !v.is_empty() {
            core_sched::core_sched_create(core_sched::PROCESS_GROUP)?
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;

    use super::*;

    #[test]
    fn test_remove_own_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("own.sock");
        let _listener = UnixListener::bind(&path).unwrap();
        let id = file_id(&path);

        remove_socket_if_unchanged(&path, id);
        assert!(!path.exists());
    }

    #[test]
    fn test_keep_replaced_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("replaced.sock");
        let _old = UnixListener::bind(&path).unwrap();
        let id = file_id(&path);

        fs::remove_file(&path).unwrap();
        let _new = UnixListener::bind(&path).unwrap();

        remove_socket_if_unchanged(&path, id);
        assert!(path.exists());
    }
}
