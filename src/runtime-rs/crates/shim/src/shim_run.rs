// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
    future::Future,
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
        run_service(
            &socket_path,
            || {
                try_core_sched(
                    std::env::var("SCHED_CORE").ok().as_deref(),
                    core_sched::core_sched_create,
                )
            },
            self.serve(),
        )
        .await
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

// Activation and serving share one cleanup path, including when activation
// fails before the service has taken ownership of the inherited listener.
async fn run_service(
    socket_path: &Path,
    activate: impl FnOnce() -> Result<()>,
    serve: impl Future<Output = Result<()>>,
) -> Result<()> {
    let socket_id = file_id(socket_path);
    let result = async {
        activate().context("enable core scheduling")?;
        serve.await
    }
    .await;
    remove_socket_if_unchanged(socket_path, socket_id);
    result
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

fn try_core_sched(
    value: Option<&str>,
    create: impl FnOnce(usize) -> Result<(), nix::errno::Errno>,
) -> Result<()> {
    if value.is_some_and(|v| !v.is_empty()) {
        create(core_sched::PROCESS_GROUP)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, os::unix::net::UnixListener};

    use nix::errno::Errno;

    use super::*;

    #[test]
    fn test_core_sched_activation() {
        for (value, requested) in [
            (None, false),
            (Some(""), false),
            (Some("1"), true),
            (Some("0"), true),
        ] {
            for outcome in [Ok(()), Err(Errno::EINVAL)] {
                let called = Cell::new(false);
                let result = try_core_sched(value, |scope| {
                    called.set(true);
                    assert_eq!(scope, core_sched::PROCESS_GROUP);
                    outcome
                });
                assert_eq!(called.get(), requested);
                if requested && outcome.is_err() {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<Errno>(),
                        Some(&Errno::EINVAL)
                    );
                } else {
                    result.unwrap();
                }
            }
        }
    }

    #[tokio::test]
    async fn test_core_sched_failure_cleans_socket_and_allows_retry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("startup.sock");
        let _listener = UnixListener::bind(&path).unwrap();
        let served = Cell::new(false);
        let result = run_service(
            &path,
            || try_core_sched(Some("1"), |_| Err(Errno::EINVAL)),
            async {
                served.set(true);
                Ok(())
            },
        )
        .await;

        let err = result.unwrap_err();
        assert_eq!(err.downcast_ref::<Errno>(), Some(&Errno::EINVAL));
        assert!(format!("{err:#}").contains("enable core scheduling"));
        assert!(!served.get());
        assert!(!path.exists());

        // The next start can bind the same socket and serve after activation.
        let _retry = UnixListener::bind(&path).unwrap();
        run_service(&path, || try_core_sched(Some("1"), |_| Ok(())), async {
            served.set(true);
            Ok(())
        })
        .await
        .unwrap();
        assert!(served.get());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn test_core_sched_failure_keeps_replaced_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("replaced.sock");
        let _listener = UnixListener::bind(&path).unwrap();
        let mut replacement = None;
        let err = run_service(
            &path,
            || {
                fs::remove_file(&path).unwrap();
                replacement = Some(UnixListener::bind(&path).unwrap());
                try_core_sched(Some("1"), |_| Err(Errno::EINVAL))
            },
            async { panic!("must not serve after failed activation") },
        )
        .await
        .unwrap_err();

        assert_eq!(err.downcast_ref::<Errno>(), Some(&Errno::EINVAL));
        assert!(path.exists());
        assert!(replacement.is_some());
    }

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
