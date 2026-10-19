// Copyright 2021-2023 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::{Duration, Instant};
use std::vec;

use super::common::{
    CgroupHierarchy, Properties, NO_SUCH_UNIT_ERROR, SIGNAL_KILL, SLICE_SUFFIX, UNIT_MODE_REPLACE,
    WHO_ENUM_ALL,
};
use super::interface::system::{JobRemoved, ManagerProxy, ManagerProxyBlocking as SystemManager};
use anyhow::{anyhow, Context, Result};
use futures::future::{self, select, Either};
use futures::stream::{self, Stream, StreamExt};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Value};

const JOB_PATH_PREFIX: &str = "/org/freedesktop/systemd1/job/";
const JOB_RESULT_DONE: &str = "done";
// Same unit start timeout as runc.
const START_UNIT_TIMEOUT: Duration = Duration::from_secs(30);

pub trait SystemdInterface {
    fn start_unit(&self, pid: i32, parent: &str, cg_hierarchy: &CgroupHierarchy) -> Result<()>;
    fn set_properties(&self, properties: &Properties) -> Result<()>;
    fn kill_unit(&self) -> Result<()>;
    fn freeze_unit(&self) -> Result<()>;
    fn thaw_unit(&self) -> Result<()>;
    fn add_process(&self, pid: i32, subcgroup: &str) -> Result<()>;
    fn get_version(&self) -> Result<String>;
    fn unit_exists(&self) -> Result<bool>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DBusClient {
    unit_name: String,
}

impl DBusClient {
    pub fn new(unit_name: String) -> Self {
        Self { unit_name }
    }

    fn build_proxy(&self) -> Result<SystemManager<'static>> {
        let connection =
            zbus::blocking::Connection::system().context("Establishing a D-Bus connection")?;
        let proxy = SystemManager::new(&connection).context("Building a D-Bus proxy manager")?;

        Ok(proxy)
    }

    // Start the unit. Return its job and the JobRemoved signals.
    async fn start_job(
        &self,
        proxy: &ManagerProxy<'static>,
        properties: &Properties<'_>,
    ) -> Result<(OwnedObjectPath, impl Stream<Item = JobRemoved> + Unpin)> {
        // Match JobRemoved before the call, so that the signal of a fast job is not lost.
        let mut removed = proxy
            .receive_job_removed()
            .await
            .context("failed to match JobRemoved signals")?;

        // Keep the signals during the call, because a full signal queue stalls the reply.
        let mut early = vec![];
        let drain = (&mut removed).for_each(|signal| {
            early.push(signal);
            future::ready(())
        });
        let call = proxy.start_transient_unit(&self.unit_name, UNIT_MODE_REPLACE, properties, &[]);
        let job = match select(Box::pin(call), drain).await {
            Either::Left((job, _)) => job?,
            Either::Right(_) => return Err(anyhow!("JobRemoved signals ended")),
        };

        Ok((job, stream::iter(early).chain(removed)))
    }
}

// Run a systemd call with a time limit like runc, so that a stuck systemd cannot block the agent.
fn block_on_until<T>(deadline: Instant, future: impl Future<Output = Result<T>>) -> Result<T> {
    let timer = async_io::Timer::at(deadline);
    match async_io::block_on(select(Box::pin(future), timer)) {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err(anyhow!(
            "no answer from systemd within {:?}",
            START_UNIT_TIMEOUT
        )),
    }
}

// Return the result of the job. The start job of a scope moves the PIDs.
// dbus-broker 32 did not send JobRemoved to an arg2 match on the unit name,
// so match the job path here.
async fn wait_job(
    job: &ObjectPath<'_>,
    mut signals: impl Stream<Item = JobRemoved> + Unpin,
) -> Result<String> {
    while let Some(signal) = signals.next().await {
        let args = signal.args()?;
        if *args.job() == *job {
            return Ok(args.result().to_string());
        }
    }

    Err(anyhow!("JobRemoved signals ended"))
}

// Cancel the job, so that it cannot move the PIDs later.
async fn cancel_job(proxy: &ManagerProxy<'_>, job: &ObjectPath<'_>) -> Result<()> {
    let id = job
        .as_str()
        .strip_prefix(JOB_PATH_PREFIX)
        .context("invalid systemd job path")?
        .parse()?;

    Ok(proxy.cancel_job(id).await?)
}

impl SystemdInterface for DBusClient {
    fn start_unit(&self, pid: i32, parent: &str, cg_hierarchy: &CgroupHierarchy) -> Result<()> {
        // Use one connection, because systemd sends JobRemoved to the job requester.
        let proxy = ManagerProxy::from(self.build_proxy()?.into_inner().into_inner());

        // enable CPUAccounting & MemoryAccounting & (Block)IOAccounting by default
        let mut properties: Properties = vec![
            ("CPUAccounting", Value::Bool(true)),
            ("DefaultDependencies", Value::Bool(false)),
            ("MemoryAccounting", Value::Bool(true)),
            ("TasksAccounting", Value::Bool(true)),
            ("Description", Value::Str("kata-agent container".into())),
            ("PIDs", Value::Array(vec![pid as u32].into())),
        ];

        match *cg_hierarchy {
            CgroupHierarchy::Legacy => properties.push(("IOAccounting", Value::Bool(true))),
            CgroupHierarchy::Unified => properties.push(("BlockIOAccounting", Value::Bool(true))),
        }

        if self.unit_name.ends_with(SLICE_SUFFIX) {
            properties.push(("Wants", Value::Str(parent.into())));
        } else {
            properties.push(("Slice", Value::Str(parent.into())));
            properties.push(("Delegate", Value::Bool(true)));
        }

        // Share one deadline between the start and the wait.
        let deadline = Instant::now() + START_UNIT_TIMEOUT;
        let (job, signals) = block_on_until(deadline, self.start_job(&proxy, &properties))
            .context(format!("failed to start transient unit {}", self.unit_name))?;

        // Wait for the start job, so that the PID is in the unit when the child continues.
        let result = block_on_until(deadline, wait_job(&job, signals));
        if matches!(result.as_deref(), Ok(JOB_RESULT_DONE)) {
            return Ok(());
        }

        // A job that did not end can still move the PID after the error.
        let cleanup = Instant::now() + START_UNIT_TIMEOUT;
        if result.is_err() {
            let _ = block_on_until(cleanup, cancel_job(&proxy, &job));
        }

        // Clear the failed state like runc, so that the unit name can be used again.
        let _ = block_on_until(cleanup, async {
            Ok(proxy.reset_failed_unit(&self.unit_name).await?)
        });

        let result = result.context(format!("failed to wait for unit {} job", self.unit_name))?;
        Err(anyhow!(
            "failed to start transient unit {}: job result {}",
            self.unit_name,
            result
        ))
    }

    fn set_properties(&self, properties: &Properties) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .set_unit_properties(&self.unit_name, true, properties)
            .context(format!("failed to set unit {} properties", self.unit_name))?;

        Ok(())
    }

    fn kill_unit(&self) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .kill_unit(&self.unit_name, WHO_ENUM_ALL, SIGNAL_KILL)
            .or_else(|e| match e {
                zbus::Error::MethodError(error_name, _, _)
                    if error_name.as_str() == NO_SUCH_UNIT_ERROR =>
                {
                    Ok(())
                }
                _ => Err(e),
            })
            .context(format!("failed to kill unit {}", self.unit_name))?;

        Ok(())
    }

    fn freeze_unit(&self) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .freeze_unit(&self.unit_name)
            .context(format!("failed to freeze unit {}", self.unit_name))?;

        Ok(())
    }

    fn thaw_unit(&self) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .thaw_unit(&self.unit_name)
            .context(format!("failed to thaw unit {}", self.unit_name))?;

        Ok(())
    }

    fn get_version(&self) -> Result<String> {
        let proxy = self.build_proxy()?;

        let systemd_version = proxy
            .version()
            .context("failed to get systemd version".to_string())?;

        Ok(systemd_version)
    }

    fn unit_exists(&self) -> Result<bool> {
        let proxy = self.build_proxy()?;

        match proxy.get_unit(&self.unit_name) {
            Ok(_) => Ok(true),
            Err(zbus::Error::MethodError(error_name, _, _))
                if error_name.as_str() == NO_SUCH_UNIT_ERROR =>
            {
                Ok(false)
            }
            Err(e) => Err(anyhow!(format!(
                "failed to check if unit {} exists: {:?}",
                self.unit_name, e
            ))),
        }
    }

    fn add_process(&self, pid: i32, subcgroup: &str) -> Result<()> {
        let proxy = self.build_proxy()?;
        proxy
            .attach_processes_to_unit(&self.unit_name, subcgroup, &[pid as u32])
            .context(format!(
                "failed to add process into unit {}",
                self.unit_name
            ))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::systemd::common::DEFAULT_SLICE;
    use crate::cgroups_rs::hierarchies::is_cgroup2_unified_mode;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use test_utils::skip_if_not_root;

    // systemd creates this directory only when it runs as PID 1.
    const SYSTEMD_RUN_DIR: &str = "/run/systemd/system";
    const BLOCKER_SECONDS: &str = "5";
    const CHILD_SECONDS: &str = "30";

    #[test]
    fn test_start_unit_waits_for_job() {
        skip_if_not_root!();

        if !Path::new(SYSTEMD_RUN_DIR).exists() || !is_cgroup2_unified_mode() {
            println!(
                "INFO: skipping {} which needs systemd and cgroup v2",
                module_path!()
            );
            return;
        }

        // Use unique unit names, so that test runs on one host do not collide.
        let id = std::process::id();
        let scope = format!("kata-agent-test-{}.scope", id);
        let blocker = format!("kata-agent-test-blocker-{}.service", id);

        // Queue a slow job ordered before the scope.
        // The scope start job then waits after StartTransientUnit returns.
        let status = Command::new("systemd-run")
            .args(["--no-block", "--collect", "--property=Type=oneshot"])
            .arg(format!("--unit={}", blocker))
            .arg(format!("--property=Before={}", scope))
            .args(["sleep", BLOCKER_SECONDS])
            .status()
            .unwrap();
        assert!(status.success(), "systemd-run failed: {:?}", status);

        let mut child = Command::new("sleep").arg(CHILD_SECONDS).spawn().unwrap();
        let pid = child.id();

        // Remove the child and the units, also when an assert fails.
        defer!({
            let _ = child.kill();
            let _ = child.wait();
            let _ = Command::new("systemctl")
                .args(["stop", &scope, &blocker])
                .status();
            let _ = Command::new("systemctl")
                .args(["reset-failed", &scope, &blocker])
                .status();
        });

        let client = DBusClient::new(scope.clone());
        let ret = client.start_unit(pid as i32, DEFAULT_SLICE, &CgroupHierarchy::Unified);
        assert!(ret.is_ok(), "Should pass. Got: {:?}", ret);

        // The child unshares its cgroupns next, so the PID must be in the scope now.
        let cgroup = fs::read_to_string(format!("/proc/{}/cgroup", pid)).unwrap();
        assert!(
            cgroup.contains(&format!("/{}", scope)),
            "pid {} not in {}: {}",
            pid,
            scope,
            cgroup
        );
    }
}
