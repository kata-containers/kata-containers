// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use cgroups::manager::is_systemd_cgroup;
use cgroups::{CgroupPid, FreezerState, FsManager, Manager, SystemdManager};
use hypervisor::{Hypervisor, VcpuThreadIds};
use kata_types::cpu::CpuSet;
use nix::errno::Errno;
use nix::sched::{sched_setaffinity, CpuSet as NixCpuSet};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use oci_spec::runtime::{LinuxCpu, LinuxCpuBuilder, LinuxResources, LinuxResourcesBuilder};
use tokio::time::sleep;

use crate::cgroups::utils::get_tgid_from_pid;
use crate::cgroups::CgroupConfig;
use crate::ResourceUpdateOp;

pub type CgroupManager = Box<dyn Manager>;

const KILL_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DESTROY_RETRY_INTERVAL: Duration = Duration::from_millis(50);
const DESTROY_RETRY_TIMEOUT: Duration = Duration::from_secs(2);

fn procs_dir(cgroup: &dyn Manager) -> Option<PathBuf> {
    // The unified hierarchy ignores the subsystem, while every v1 hierarchy
    // lists the same processes.
    [
        None,
        Some("freezer"),
        Some("pids"),
        Some("memory"),
        Some("cpu"),
    ]
    .iter()
    .find_map(|subsystem| cgroup.cgroup_path(*subsystem).ok())
    .map(PathBuf::from)
}

fn read_procs(dir: &Path) -> Result<Vec<i32>> {
    let procs = match fs::read_to_string(dir.join("cgroup.procs")) {
        Ok(procs) => procs,
        // destroy() moves stragglers out before removing the cgroup, but
        // cleanup only reaches it once the VMM exit is confirmed.
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(err) => return Err(err).with_context(|| format!("read {}", dir.display())),
    };
    procs
        .lines()
        .map(|pid| pid.parse().context("parse cgroup.procs"))
        .collect()
}

async fn empty_cgroup(cgroup: &dyn Manager, deadline: tokio::time::Instant) -> Result<()> {
    let dir = procs_dir(cgroup).context("no cgroup.procs for the sandbox cgroup")?;
    loop {
        let procs = read_procs(&dir)?;
        if procs.is_empty() {
            return Ok(());
        }
        if procs.contains(&(process::id() as i32)) {
            return Err(anyhow!(
                "refusing to kill {}, the runtime runs in it",
                dir.display()
            ));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(anyhow!("{} still holds {:?}", dir.display(), procs));
        }
        kill_procs(cgroup, &dir)?;
        sleep(KILL_POLL_INTERVAL).await;
    }
}

fn kill_procs(cgroup: &dyn Manager, dir: &Path) -> Result<()> {
    let kill_file = dir.join("cgroup.kill");
    if kill_file.exists() {
        return fs::write(&kill_file, "1")
            .with_context(|| format!("write {}", kill_file.display()));
    }

    // Like runc without cgroup.kill: a frozen process can neither fork nor
    // exit, so its PID cannot be reused before SIGKILL reaches it.
    let frozen = match cgroup.freeze(FreezerState::Frozen) {
        Ok(()) => true,
        Err(err) => {
            warn!(
                sl!(),
                "cannot freeze {}, killing unfrozen: {err}",
                dir.display()
            );
            false
        }
    };
    let result = read_procs(dir).and_then(|procs| {
        for pid in procs {
            match kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(()) | Err(Errno::ESRCH) => {}
                Err(err) => return Err(err).with_context(|| format!("kill {pid}")),
            }
        }
        Ok(())
    });
    if frozen {
        cgroup
            .freeze(FreezerState::Thawed)
            .with_context(|| format!("thaw {}", dir.display()))?;
    }
    result
}

/// Retry destroy() briefly while `is_retryable` says the failure is
/// transient (e.g. EBUSY, see `CgroupsResourceInner::is_busy_error`), instead
/// of giving up on the first transient race over the cgroup directory.
///
/// The classifier is a parameter, rather than calling
/// `CgroupsResourceInner::is_busy_error` directly, so the retry loop can be
/// driven deterministically by a test double: `cgroups::manager::Error`'s
/// EBUSY-carrying variant can only be constructed from inside the
/// `cgroups-rs` crate, so tests supply their own predicate instead.
async fn destroy_with_retry(
    cgroup: &mut CgroupManager,
    what: &str,
    is_retryable: impl Fn(&cgroups::manager::Error) -> bool,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + DESTROY_RETRY_TIMEOUT;
    loop {
        match cgroup.destroy() {
            Ok(()) => return Ok(()),
            Err(err) if is_retryable(&err) && tokio::time::Instant::now() < deadline => {
                warn!(sl!(), "destroy {what} busy, retrying: {err}");
                sleep(DESTROY_RETRY_INTERVAL).await;
            }
            Err(err) => return Err(err).with_context(|| format!("destroy {what}")),
        }
    }
}

pub(crate) struct CgroupsResourceInner {
    /// Container resources, key is container id, and value is resources.
    resources: HashMap<String, LinuxResources>,
    sandbox_cgroup: CgroupManager,
    overhead_cgroup: Option<CgroupManager>,
    /// User-facing config knob: whether vCPU pinning is allowed at all.
    /// Comes from TOML `enable_vcpus_pinning` or the per-pod annotation.
    enable_vcpus_pinning: bool,
    /// Runtime state: whether pinning is currently active. Pinning is only
    /// turned on when `enable_vcpus_pinning` is true *and* the vCPU count
    /// matches the sandbox cpuset size. Tracked so we know when to reset
    /// threads back to the full cpuset after a mismatch.
    is_vcpus_pinning_on: bool,
    sandbox_cgroup_deleted: bool,
    overhead_cgroup_deleted: bool,
}

impl CgroupsResourceInner {
    /// Walk `err`'s source chain for an `io::Error` satisfying `matches`.
    ///
    /// Factored out of `is_busy_error`/`is_already_exists_error` so the
    /// chain-walk itself can be unit tested directly: `cgroups::manager::Error`
    /// values that actually carry an `io::Error` cause can only be built from
    /// inside the `cgroups-rs` crate, so tests exercise this with a synthetic
    /// error chain instead of going through `cgroups::manager::Error`.
    fn io_error_in_chain_matches(
        err: &(dyn std::error::Error + 'static),
        matches: impl Fn(&std::io::Error) -> bool,
    ) -> bool {
        let mut source = err.source();

        while let Some(inner_err) = source {
            if let Some(io_err) = inner_err.downcast_ref::<std::io::Error>() {
                if matches(io_err) {
                    return true;
                }
            }

            source = inner_err.source();
        }

        false
    }

    fn is_already_exists_error(err: &cgroups::manager::Error) -> bool {
        Self::io_error_in_chain_matches(err, |io_err| {
            io_err.kind() == std::io::ErrorKind::AlreadyExists
        })
    }

    /// destroy() moves stragglers out of the cgroup and then rmdir(2)s it. A
    /// process that briefly has the directory open right as the sandbox
    /// exits (e.g. containerd inspecting it on the TaskExit path) can make
    /// that rmdir fail with EBUSY even though the cgroup is otherwise empty.
    fn is_busy_error(err: &cgroups::manager::Error) -> bool {
        Self::io_error_in_chain_matches(err, |io_err| io_err.raw_os_error() == Some(libc::EBUSY))
    }

    fn add_proc_with_existing_retry(
        cgroup: &mut CgroupManager,
        pid: CgroupPid,
        context: &str,
    ) -> Result<()> {
        match cgroup.add_proc(pid) {
            Ok(_) => Ok(()),
            Err(err) if Self::is_already_exists_error(&err) => cgroup
                .add_proc(pid)
                .with_context(|| format!("{context} (retry after pre-existing cgroup)")),
            Err(err) => Err(err).context(context.to_string()),
        }
    }

    /// Synchronously write the runtime's own pid into the sandbox cgroup's
    /// `cgroup.procs` (cgroup v2 only; a no-op otherwise).
    ///
    /// `add_proc` already places the runtime, but on the systemd driver that
    /// is an asynchronous call: the runtime may fork virtiofsd and the
    /// VMM before systemd has moved it, so those children inherit its original
    /// cgroup (e.g. `system.slice/containerd.service`). Under cgroup v2
    /// first-touch accounting the guest RAM is then charged there, not to the
    /// pod. A direct write is synchronous, so children forked afterwards
    /// inherit the sandbox cgroup.
    fn place_runtime_in_sandbox_cgroup_v2_sync(cgroup: &CgroupManager) -> Result<()> {
        if !cgroup.v2() {
            return Ok(());
        }
        let dir = cgroup
            .cgroup_path(None)
            .context("resolve sandbox cgroup path for runtime placement")?;
        let procs_path = format!("{}/cgroup.procs", dir.trim_end_matches('/'));
        let pid = process::id();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&procs_path)
            .with_context(|| format!("open sandbox cgroup.procs {procs_path}"))?;
        std::io::Write::write_all(&mut file, format!("{pid}\n").as_bytes())
            .with_context(|| format!("move runtime pid {pid} into sandbox cgroup {procs_path}"))?;
        info!(
            sl!(),
            "synchronously placed runtime (pid {}) into sandbox cgroup: {}", pid, procs_path
        );
        Ok(())
    }

    /// Create cgroup managers according to the cgroup configuration.
    ///
    /// # Returns
    ///
    /// - `Ok((CgroupManager, Option<CgroupManager>))`: A tuple containing
    ///   the sandbox cgroup manager and an optional overhead cgroup
    ///   manager.
    fn new_cgroup_managers(
        config: &CgroupConfig,
    ) -> Result<(CgroupManager, Option<CgroupManager>)> {
        let use_systemd = is_systemd_cgroup(&config.path);
        let sandbox_cgroup = if use_systemd {
            let mut manager = SystemdManager::new(&config.path).context("new systemd manager")?;
            // Set SIGTERM timeout to 5mins, so that the runtime has up to
            // 5mins to do graceful shutdown. Exceeding this timeout, the
            // systemd will forcibly kill the runtime by sending SIGKILL.
            manager.set_term_timeout(300).context("set term timeout")?;
            Box::new(manager) as Box<dyn Manager>
        } else {
            let manager = FsManager::new(&config.path).context("new fs manager")?;
            Box::new(manager) as Box<dyn Manager>
        };

        let overhead_cgroup = if config.sandbox_cgroup_only {
            None
        } else if use_systemd {
            let mut manager = SystemdManager::new(&config.overhead_path)
                .context("new systemd manager for overhead")?;
            manager
                .set_term_timeout(300)
                .context("set term timeout for overhead")?;
            Some(Box::new(manager) as Box<dyn Manager>)
        } else {
            let manager =
                FsManager::new(&config.overhead_path).context("new fs manager for overhead")?;
            Some(Box::new(manager) as Box<dyn Manager>)
        };

        Ok((sandbox_cgroup, overhead_cgroup))
    }

    /// Create a new `CgroupsResourceInner` instance.
    pub(crate) fn new(config: &CgroupConfig) -> Result<Self> {
        let (mut sandbox_cgroup, mut overhead_cgroup) =
            Self::new_cgroup_managers(config).context("create new cgroups")?;

        // The runtime is prioritized to be added to the overhead cgroup.
        let pid = CgroupPid::from(process::id() as u64);
        if let Some(overhead_cgroup) = overhead_cgroup.as_mut() {
            Self::add_proc_with_existing_retry(
                overhead_cgroup,
                pid,
                "add runtime to overhead cgroup",
            )?;
        } else {
            Self::add_proc_with_existing_retry(
                &mut sandbox_cgroup,
                pid,
                "add runtime to sandbox cgroup",
            )?;
            // The systemd add_proc above is asynchronous; make sure we have
            // really joined the sandbox cgroup before forking any child.
            Self::place_runtime_in_sandbox_cgroup_v2_sync(&sandbox_cgroup)
                .context("synchronously place runtime in sandbox cgroup")?;
        }

        Ok(Self {
            resources: HashMap::new(),
            sandbox_cgroup,
            overhead_cgroup,
            enable_vcpus_pinning: config.enable_vcpus_pinning,
            is_vcpus_pinning_on: false,
            sandbox_cgroup_deleted: false,
            overhead_cgroup_deleted: false,
        })
    }

    pub(crate) fn restore(config: &CgroupConfig) -> Result<Self> {
        let (sandbox_cgroup, overhead_cgroup) =
            Self::new_cgroup_managers(config).context("restore cgroups")?;

        Ok(Self {
            resources: HashMap::new(),
            sandbox_cgroup,
            overhead_cgroup,
            enable_vcpus_pinning: config.enable_vcpus_pinning,
            is_vcpus_pinning_on: false,
            sandbox_cgroup_deleted: false,
            overhead_cgroup_deleted: false,
        })
    }
}

impl CgroupsResourceInner {
    /// Add cpuset resources of all containers to the sandbox cgroup.
    fn collect_resources(&self) -> Result<LinuxResources> {
        let mut cpu_cpus = HashSet::new();
        let mut cpu_mems = HashSet::new();

        for res in self.resources.values() {
            if let Some(cpu) = res.cpu() {
                if let Some(cpus) = cpu.cpus() {
                    cpu_cpus.insert(cpus.to_string());
                }
                if let Some(mems) = cpu.mems() {
                    cpu_mems.insert(mems.to_string());
                }
            }
        }

        let mut resources_builder = LinuxResourcesBuilder::default();

        let mut cpu_builder = LinuxCpuBuilder::default();
        if !cpu_cpus.is_empty() {
            cpu_builder = cpu_builder.cpus(cpu_cpus.into_iter().collect::<Vec<_>>().join(","));
        }
        if !cpu_mems.is_empty() {
            cpu_builder = cpu_builder.mems(cpu_mems.into_iter().collect::<Vec<_>>().join(","));
        }
        let cpu = cpu_builder.build().context("build linux cpu")?;
        if cpu != LinuxCpu::default() {
            resources_builder = resources_builder.cpu(cpu);
        }

        let resources = resources_builder.build().context("build linux resources")?;

        Ok(resources)
    }

    async fn move_vcpus_to_sandbox_cgroup(&mut self, hv_pids: &VcpuThreadIds) -> Result<usize> {
        let mut pids = hv_pids.vcpus.values();

        // Use threaded mode only in cgroup v1 + cgroupfs
        if !self.sandbox_cgroup.systemd() && !self.sandbox_cgroup.v2() {
            for pid in pids {
                let pid = CgroupPid::from(*pid as u64);
                self.sandbox_cgroup
                    .add_thread(pid)
                    .with_context(|| format!("add vcpu pid {}", pid.pid))?
            }
        } else {
            // No vCPU, exits early
            let vcpu = match pids.next() {
                Some(pid) => *pid,
                None => return Ok(0),
            };

            let tgid = get_tgid_from_pid(vcpu as i32).context("get tgid from vCPU thread")? as u64;
            self.sandbox_cgroup
                .add_proc(CgroupPid::from(tgid))
                .with_context(|| format!("add vcpu tgid {tgid}"))?;
        }

        Ok(hv_pids.vcpus.len())
    }

    async fn update_sandbox_cgroups(&mut self, hypervisor: &dyn Hypervisor) -> Result<bool> {
        let needs_thread_ids = self.overhead_cgroup.is_some() || self.enable_vcpus_pinning;

        let thread_ids = if needs_thread_ids {
            let mut tids = hypervisor
                .get_thread_ids()
                .await
                .context("get vCPU thread IDs")?;

            // QEMU may not have spawned all vCPU threads yet. Retry with
            // exponential backoff until we see the expected count.
            let expected = hypervisor
                .hypervisor_config()
                .await
                .cpu_info
                .default_vcpus
                .ceil() as usize;
            if expected > 0 && tids.vcpus.len() < expected {
                const MAX_ATTEMPTS: u32 = 10;
                let mut backoff = Duration::from_millis(50);
                for attempt in 2..=MAX_ATTEMPTS {
                    if tids.vcpus.len() >= expected {
                        break;
                    }
                    info!(
                        sl!(),
                        "waiting for all vCPU threads: have {}, want {}, attempt {}",
                        tids.vcpus.len(),
                        expected,
                        attempt
                    );
                    sleep(backoff).await;
                    backoff *= 2;
                    tids = hypervisor
                        .get_thread_ids()
                        .await
                        .context("get vCPU thread IDs (retry)")?;
                }
                if tids.vcpus.len() < expected {
                    warn!(
                        sl!(),
                        "not all vCPU threads available after retries: have {}, want {}; pinning available ones",
                        tids.vcpus.len(),
                        expected
                    );
                }
            }

            Some(tids)
        } else {
            None
        };

        // The runtime is under overhead cgroup if available. The
        // hypervisor as a child of the runtime is under the overhead
        // cgroup by default. We should move VMM process/vCPU threads to
        // the sandbox cgroup to prevent them from consuming excessive
        // resources.
        if self.overhead_cgroup.is_some() {
            let vcpu_num = self
                .move_vcpus_to_sandbox_cgroup(thread_ids.as_ref().unwrap())
                .await
                .context("move vcpus to sandbox cgroup")?;
            // The cgroup managers will not create cgroups if no processes
            // are added to it. `vcpu_num == 0` reflects that the
            // hypervisor hasn't been started yet. We skip resource
            // setting, as the sandbox cgroup might not be created yet.
            if vcpu_num == 0 {
                return Ok(false);
            }
        }

        let sandbox_resources = self.collect_resources().context("collect resources")?;
        self.sandbox_cgroup.set(&sandbox_resources).context("set")?;

        if let Some(thread_ids) = thread_ids {
            self.check_vcpus_pinning(thread_ids)
                .context("check vCPUs pinning")?;
        }

        Ok(true)
    }

    fn collect_sandbox_cpuset(&self) -> CpuSet {
        let mut cpuset = CpuSet::new();
        for res in self.resources.values() {
            let Some(cpus_str) = res.cpu().as_ref().and_then(|c| c.cpus().as_deref()) else {
                continue;
            };
            match CpuSet::from_str(cpus_str) {
                Ok(parsed) => cpuset.extend(&parsed),
                Err(e) => warn!(
                    sl!(),
                    "vCPU pinning: failed to parse cpuset \"{}\": {}", cpus_str, e
                ),
            }
        }
        cpuset
    }

    fn set_thread_affinity(tid: u32, cpus: &[u32]) -> Result<()> {
        let nix_cpuset = build_nix_cpuset(cpus)?;
        sched_setaffinity(Pid::from_raw(tid as i32), &nix_cpuset).map_err(|e| {
            anyhow!(
                "sched_setaffinity failed for thread {} to cpus {:?}: {}",
                tid,
                cpus,
                e
            )
        })
    }

    fn check_vcpus_pinning(&mut self, thread_ids: VcpuThreadIds) -> Result<()> {
        if !self.enable_vcpus_pinning {
            return Ok(());
        }

        let cpuset = self.collect_sandbox_cpuset();
        let cpuset_slice: Vec<u32> = cpuset.iter().copied().collect();

        let num_vcpus = thread_ids.vcpus.len();
        let num_cpus = cpuset_slice.len();

        if num_vcpus == 0 || num_cpus == 0 || num_vcpus != num_cpus {
            if num_vcpus == 0 {
                info!(sl!(), "vCPU pinning: no vCPU threads found, skipping");
            } else if num_cpus == 0 {
                info!(sl!(), "vCPU pinning: no cpuset configured, skipping");
            } else {
                info!(
                    sl!(),
                    "vCPU pinning: vCPU count ({}) != cpuset size ({}), pinning not possible",
                    num_vcpus,
                    num_cpus
                );
            }
            if self.is_vcpus_pinning_on && num_vcpus > 0 {
                info!(sl!(), "vCPU pinning: resetting previous pinning");
                self.reset_vcpus_pinning(&thread_ids.vcpus, &cpuset_slice)?;
                self.is_vcpus_pinning_on = false;
            }
            return Ok(());
        }

        // Pin vCPU i to cpuset_slice[i] (both sorted by index)
        let mut sorted_vcpus: Vec<(u32, u32)> = thread_ids.vcpus.into_iter().collect();
        sorted_vcpus.sort_by_key(|(idx, _)| *idx);

        for (i, (_vcpu_idx, tid)) in sorted_vcpus.iter().enumerate() {
            if let Err(e) = Self::set_thread_affinity(*tid, &cpuset_slice[i..i + 1]) {
                // On failure, reset all pinning and propagate the error
                let all_vcpus: HashMap<u32, u32> = sorted_vcpus.iter().copied().collect();
                let _ = self.reset_vcpus_pinning(&all_vcpus, &cpuset_slice);
                return Err(e).context(format!(
                    "failed to pin vCPU thread {} to CPU {}",
                    tid, cpuset_slice[i]
                ));
            }
        }

        self.is_vcpus_pinning_on = true;
        info!(
            sl!(),
            "vCPU pinning: pinned {} vCPU threads to cpuset {:?}", num_vcpus, cpuset_slice
        );
        Ok(())
    }

    fn reset_vcpus_pinning(&self, vcpus: &HashMap<u32, u32>, cpuset_slice: &[u32]) -> Result<()> {
        if cpuset_slice.is_empty() {
            return Ok(());
        }
        for tid in vcpus.values() {
            Self::set_thread_affinity(*tid, cpuset_slice)
                .with_context(|| format!("failed to reset vCPU thread {} affinity", tid))?;
        }
        Ok(())
    }
}

impl CgroupsResourceInner {
    /// A restored shim holds no child handles. The runtime joins these
    /// cgroups before it forks the VMM and its daemons, and an in-process VMM
    /// runs in the runtime itself, so an empty cgroup is the kernel's word
    /// that the VMM is gone. Must not run in a shim that lives in them.
    pub(crate) async fn kill_all_processes(&self, wait: Duration) -> Result<()> {
        let cgroups: Vec<&dyn Manager> = std::iter::once(self.sandbox_cgroup.as_ref())
            .chain(self.overhead_cgroup.as_deref())
            .collect();
        let deadline = tokio::time::Instant::now() + wait;
        for cgroup in cgroups {
            empty_cgroup(cgroup, deadline).await?;
        }
        Ok(())
    }

    pub(crate) async fn delete(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        if !self.sandbox_cgroup_deleted {
            match destroy_with_retry(
                &mut self.sandbox_cgroup,
                "sandbox cgroup",
                Self::is_busy_error,
            )
            .await
            {
                Ok(()) => self.sandbox_cgroup_deleted = true,
                Err(e) => errors.push(e),
            }
        }

        if !self.overhead_cgroup_deleted {
            if let Some(overhead_cgroup) = self.overhead_cgroup.as_mut() {
                match destroy_with_retry(overhead_cgroup, "overhead cgroup", Self::is_busy_error)
                    .await
                {
                    Ok(()) => self.overhead_cgroup_deleted = true,
                    Err(e) => errors.push(e),
                }
            } else {
                self.overhead_cgroup_deleted = true;
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            for error in &errors {
                warn!(sl!(), "cgroup cleanup step failed: {error:#}");
            }
            Err(anyhow!("{} cgroup cleanup step(s) failed", errors.len()))
        }
    }

    pub(crate) async fn update(
        &mut self,
        cid: &str,
        resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<()> {
        let old = match op {
            ResourceUpdateOp::Add | ResourceUpdateOp::Update => {
                let resources = resources.ok_or_else(|| {
                    anyhow::anyhow!("resources should not be empty for Add or Update operation")
                })?;
                let new = new_cpuset_resources(resources).context("new cpuset resources")?;
                let old = self.resources.insert(cid.to_string(), new.clone());
                // If the new resources are the same as the old ones, we
                // can skip the update.
                if let Some(old) = old.as_ref() {
                    if old == &new {
                        return Ok(());
                    }
                }
                old
            }
            ResourceUpdateOp::Del => self.resources.remove(cid),
        };

        let ret = self
            .update_sandbox_cgroups(hypervisor)
            .await
            .context("update sandbox cgroups");

        // Rollback if the update fails
        if ret.is_err() {
            match op {
                ResourceUpdateOp::Add => {
                    self.resources.remove(cid);
                }
                ResourceUpdateOp::Update | ResourceUpdateOp::Del => {
                    if let Some(old) = old {
                        self.resources.insert(cid.to_string(), old);
                    }
                }
            }
        }

        ret.map(|_| ())
    }

    pub(crate) async fn setup_after_start_vm(&mut self, hypervisor: &dyn Hypervisor) -> Result<()> {
        let updated = self
            .update_sandbox_cgroups(hypervisor)
            .await
            .context("update sandbox cgroups after start vm")?;

        // There is an overhead cgroup and we are falling to move the vCPUs
        // to the sandbox cgroup, it results in those threads being under
        // the overhead cgroup, and allowing them to consume more resources
        // than we have allocated for the sandbox.
        if self.overhead_cgroup.is_some() && !updated {
            return Err(anyhow!("hypervisor cannot be moved to sandbox cgroup"));
        }

        Ok(())
    }
}

/// Build a `NixCpuSet` from a slice of CPU ids.
fn build_nix_cpuset(cpus: &[u32]) -> Result<NixCpuSet> {
    let mut nix_cpuset = NixCpuSet::new();
    for cpu_id in cpus {
        nix_cpuset
            .set(*cpu_id as usize)
            .map_err(|e| anyhow!("failed to set CPU {} in cpuset: {}", cpu_id, e))?;
    }
    Ok(nix_cpuset)
}

/// Copy cpu.cpus and cpu.mems from the given resources to new resources.
fn new_cpuset_resources(resources: &LinuxResources) -> Result<LinuxResources> {
    let cpu = resources.cpu();
    let cpus = cpu.as_ref().and_then(|c| c.cpus().clone());
    let mems = cpu.as_ref().and_then(|c| c.mems().clone());

    let mut builder = LinuxCpuBuilder::default();
    if let Some(cpus) = cpus {
        builder = builder.cpus(cpus);
    }
    if let Some(mems) = mems {
        builder = builder.mems(mems);
    }
    let linux_cpu = builder.build().context("build linux cpu")?;

    let builder = LinuxResourcesBuilder::default().cpu(linux_cpu);
    let resources = builder.build().context("build linux resources")?;

    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn make_resources_with_cpus(cpus: &str) -> LinuxResources {
        let cpu = LinuxCpuBuilder::default()
            .cpus(cpus.to_string())
            .build()
            .unwrap();
        LinuxResourcesBuilder::default().cpu(cpu).build().unwrap()
    }

    fn make_inner_for_test(enable_pinning: bool) -> CgroupsResourceInner {
        CgroupsResourceInner {
            resources: HashMap::new(),
            sandbox_cgroup: Box::new(FsManager::new("test_sandbox_cgroup_pinning").unwrap()),
            overhead_cgroup: None,
            enable_vcpus_pinning: enable_pinning,
            is_vcpus_pinning_on: false,
            sandbox_cgroup_deleted: false,
            overhead_cgroup_deleted: false,
        }
    }

    #[rstest]
    #[case::empty(vec![], vec![])]
    #[case::single_range(vec![("c1", "0-3")], vec![0, 1, 2, 3])]
    #[case::single_list(vec![("c1", "0,2,4")], vec![0, 2, 4])]
    #[case::multi_container_disjoint(
        vec![("c1", "0,1"), ("c2", "2,3")],
        vec![0, 1, 2, 3]
    )]
    #[case::multi_container_overlapping(
        vec![("c1", "0-2"), ("c2", "1-3")],
        vec![0, 1, 2, 3]
    )]
    #[case::three_containers(
        vec![("c1", "0"), ("c2", "4-6"), ("c3", "2")],
        vec![0, 2, 4, 5, 6]
    )]
    fn test_collect_sandbox_cpuset(
        #[case] containers: Vec<(&str, &str)>,
        #[case] expected: Vec<u32>,
    ) {
        let mut inner = make_inner_for_test(true);
        for (cid, cpus) in containers {
            inner
                .resources
                .insert(cid.to_string(), make_resources_with_cpus(cpus));
        }
        let cpuset = inner.collect_sandbox_cpuset();
        let cpus: Vec<u32> = cpuset.iter().copied().collect();
        assert_eq!(cpus, expected);
    }

    #[test]
    fn test_collect_sandbox_cpuset_no_cpu_field() {
        let mut inner = make_inner_for_test(true);
        let resources = LinuxResourcesBuilder::default().build().unwrap();
        inner.resources.insert("c1".to_string(), resources);
        let cpuset = inner.collect_sandbox_cpuset();
        assert!(cpuset.is_empty());
    }

    #[rstest]
    #[case::specific_cpus(&[0, 2, 4], &[0, 2, 4], &[1, 3])]
    #[case::contiguous(&[0, 1, 2, 3], &[0, 1, 2, 3], &[4, 5])]
    #[case::single(&[7], &[7], &[0, 6])]
    fn test_build_nix_cpuset(
        #[case] input: &[u32],
        #[case] expected_set: &[u32],
        #[case] expected_unset: &[u32],
    ) {
        let cpuset = build_nix_cpuset(input).unwrap();
        for cpu in expected_set {
            assert!(
                cpuset.is_set(*cpu as usize).unwrap(),
                "CPU {} should be set",
                cpu
            );
        }
        for cpu in expected_unset {
            assert!(
                !cpuset.is_set(*cpu as usize).unwrap(),
                "CPU {} should not be set",
                cpu
            );
        }
    }

    #[test]
    fn test_build_nix_cpuset_empty() {
        let cpuset = build_nix_cpuset(&[]).unwrap();
        assert!(!cpuset.is_set(0).unwrap());
    }

    #[rstest]
    #[case::disabled(false, false)]
    #[case::enabled(true, false)]
    fn test_pinning_initial_state(#[case] enable: bool, #[case] expected_on: bool) {
        let inner = make_inner_for_test(enable);
        assert_eq!(inner.enable_vcpus_pinning, enable);
        assert_eq!(inner.is_vcpus_pinning_on, expected_on);
    }

    /// A sibling of the test's own cgroup, which root can always create and
    /// an unprivileged user can under a `systemd-run --user --scope`.
    fn sibling_cgroup(name: &str) -> Option<String> {
        if !Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
            return None;
        }
        let own = fs::read_to_string("/proc/self/cgroup").ok()?;
        let own = own.lines().find_map(|line| line.strip_prefix("0::"))?;
        let parent = Path::new(own).parent()?;
        let relative = parent.join(name);
        let relative = relative.to_str()?.trim_start_matches('/').to_string();
        fs::create_dir(Path::new("/sys/fs/cgroup").join(&relative)).ok()?;
        Some(relative)
    }

    #[tokio::test]
    async fn restored_shim_empties_the_sandbox_cgroup() {
        let name = format!("kata-kill-test-{}", process::id());
        let Some(relative) = sibling_cgroup(&name) else {
            eprintln!("skipping: cannot create a cgroup v2 sibling");
            return;
        };
        let dir = Path::new("/sys/fs/cgroup").join(&relative);
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        fs::write(dir.join("cgroup.procs"), child.id().to_string()).unwrap();
        let cgroup = FsManager::new(&relative).unwrap();

        let result = empty_cgroup(
            &cgroup,
            tokio::time::Instant::now() + Duration::from_secs(5),
        )
        .await;
        let status = child.wait().unwrap();
        fs::remove_dir(&dir).unwrap();

        result.unwrap();
        assert!(!status.success());
        assert!(read_procs(&dir).unwrap().is_empty());
    }

    #[tokio::test]
    async fn never_kills_the_cgroup_the_runtime_runs_in() {
        if !Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
            eprintln!("skipping: needs cgroup v2");
            return;
        }
        let own = fs::read_to_string("/proc/self/cgroup").unwrap();
        let own = own
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .unwrap();
        let cgroup = FsManager::new(own.trim_start_matches('/')).unwrap();

        let err = empty_cgroup(&cgroup, tokio::time::Instant::now())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("the runtime runs in it"));
    }

    /// A minimal error whose source chain carries a raw `io::Error`, standing
    /// in for the real `cgroups::manager::Error::Cgroupfs(...)` chain that
    /// `is_busy_error` walks in production (that variant's constructors are
    /// `pub(crate)` in `cgroups-rs`, so it cannot be built here).
    #[derive(Debug)]
    struct WrappedIoError(std::io::Error);

    impl std::fmt::Display for WrappedIoError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "wrapped: {}", self.0)
        }
    }

    impl std::error::Error for WrappedIoError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn test_io_error_in_chain_matches_finds_ebusy() {
        let err = WrappedIoError(std::io::Error::from_raw_os_error(libc::EBUSY));
        assert!(CgroupsResourceInner::io_error_in_chain_matches(&err, |e| {
            e.raw_os_error() == Some(libc::EBUSY)
        }));
    }

    #[test]
    fn test_io_error_in_chain_matches_rejects_other_errno() {
        let err = WrappedIoError(std::io::Error::from_raw_os_error(libc::EPERM));
        assert!(!CgroupsResourceInner::io_error_in_chain_matches(
            &err,
            |e| e.raw_os_error() == Some(libc::EBUSY)
        ));
    }

    #[test]
    fn test_io_error_in_chain_matches_no_source_chain() {
        // A bare cgroups::manager::Error variant has no io::Error in its
        // source chain at all; is_busy_error must not treat that as busy.
        assert!(!CgroupsResourceInner::is_busy_error(
            &cgroups::manager::Error::InvalidArgument
        ));
    }

    /// A `Manager` test double whose `destroy()` fails a fixed number of
    /// times before succeeding, so `destroy_with_retry`'s retry loop can be
    /// driven deterministically. `calls` is shared via `Arc` so the test can
    /// observe the attempt count after `destroy_with_retry` returns, even
    /// though it only ever sees the manager through `&mut CgroupManager`.
    struct FlakyManager {
        failures_left: u32,
        calls: std::sync::Arc<std::sync::atomic::AtomicU32>,
    }

    impl Manager for FlakyManager {
        fn add_proc(&mut self, _tgid: CgroupPid) -> cgroups::manager::Result<()> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn add_thread(&mut self, _pid: CgroupPid) -> cgroups::manager::Result<()> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn pids(&self) -> cgroups::manager::Result<Vec<CgroupPid>> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn freeze(&self, _state: FreezerState) -> cgroups::manager::Result<()> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn destroy(&mut self) -> cgroups::manager::Result<()> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.failures_left > 0 {
                self.failures_left -= 1;
                Err(cgroups::manager::Error::InvalidLinuxResource)
            } else {
                Ok(())
            }
        }
        fn set(&mut self, _resources: &LinuxResources) -> cgroups::manager::Result<()> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn cgroup_path(&self, _subsystem: Option<&str>) -> cgroups::manager::Result<String> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn enable_cpus_topdown(&self, _cpus: &str) -> cgroups::manager::Result<()> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn stats(&self) -> cgroups::CgroupStats {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn paths(&self) -> &HashMap<String, String> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn mounts(&self) -> &HashMap<String, String> {
            unimplemented!("not exercised by destroy_with_retry")
        }
        fn systemd(&self) -> bool {
            false
        }
        fn v2(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn destroy_with_retry_retries_busy_errors_then_succeeds() {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let mut manager: CgroupManager = Box::new(FlakyManager {
            failures_left: 2,
            calls: calls.clone(),
        });

        // `InvalidLinuxResource` stands in for a real EBUSY error (see
        // `WrappedIoError` above for why); this test injects its own "is
        // busy" predicate instead of `CgroupsResourceInner::is_busy_error`.
        let result = destroy_with_retry(&mut manager, "test cgroup", |err| {
            matches!(err, cgroups::manager::Error::InvalidLinuxResource)
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn destroy_with_retry_gives_up_immediately_on_non_retryable_error() {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let mut manager: CgroupManager = Box::new(FlakyManager {
            failures_left: 1,
            calls: calls.clone(),
        });

        // InvalidLinuxResource carries no io::Error source, so the real
        // is_busy_error correctly refuses to retry it.
        let result = destroy_with_retry(
            &mut manager,
            "test cgroup",
            CgroupsResourceInner::is_busy_error,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
