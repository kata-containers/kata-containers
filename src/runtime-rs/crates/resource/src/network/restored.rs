// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{device::driver, Hypervisor};
use kata_sys_util::netns::{is_pinned_netns, is_proc_pid_netns, NetnsGuard};
use scopeguard::defer;
use std::path::Path;

use super::network_pair::{delete_link_by_name, kata_link_names};
use super::{rebind_saved_physical_endpoints, EndpointState, Network};

/// The network of a sandbox restored after its shim went away.
///
/// A live network holds the netns and the endpoints it attached. A restored
/// one only has what state.json recorded about them, and what is left to do
/// with that is give passed-through devices back to their host drivers (the
/// VM is gone, so there is nothing to detach them from) and remove the links
/// Kata made in the namespace, when the namespace is the caller's and so
/// outlives the sandbox. Holding this as the resource manager's network
/// keeps one cleanup path for both, and the endpoints stay in the state
/// until that cleanup succeeded.
pub(crate) struct RestoredNetwork {
    endpoints: Vec<EndpointState>,
    netns_path: Option<String>,
}

impl RestoredNetwork {
    pub(crate) fn new(endpoints: Vec<EndpointState>, netns_path: Option<String>) -> Self {
        Self {
            endpoints,
            netns_path,
        }
    }
}

/// Remove every link Kata made in the namespace, told apart by the name
/// suffix every one of them is created with: attached or not, recorded or
/// not, bridge and TAP alike. A namespace that is already gone took them
/// with it. Shared by the restored network and the cleanup of a sandbox
/// that died before it had one; a mid-construction death leaves TAPs that
/// only the namespace the bootstrap state recorded can reach.
pub async fn remove_kata_links(netns_path: &str) -> Result<()> {
    // A /proc/<pid>/ns/net reference died with the VMM it named: by the
    // time recovery runs, the pid either resolves nothing or, reused,
    // names an unrelated process — whose namespace must not be swept.
    if is_proc_pid_netns(netns_path) {
        info!(
            sl!(),
            "the recorded netns reference {netns_path} died with its process; not swept"
        );
        return Ok(());
    }
    // Whoever owns the namespace may be tearing it down as recovery runs:
    // once unmounted, and until the file is removed, the path is a plain
    // file that cannot be entered.
    if !is_pinned_netns(Path::new(netns_path)) {
        return Ok(());
    }
    let Some(_netns_guard) = enter_cleanup_netns(netns_path)? else {
        return Ok(());
    };
    let (connection, handle, _) = rtnetlink::new_connection().context("new netlink connection")?;
    let thread_handler = tokio::spawn(connection);
    defer!({
        thread_handler.abort();
    });
    for name in kata_link_names(&handle).await? {
        delete_link_by_name(&handle, &name)
            .await
            .with_context(|| format!("remove the kata link {name}"))?;
    }
    Ok(())
}

fn enter_cleanup_netns(netns_path: &str) -> Result<Option<NetnsGuard>> {
    match NetnsGuard::new(netns_path) {
        Ok(guard) => Ok(Some(guard)),
        // The owner can unmount or remove the namespace after the initial
        // check. Its links are gone too; never fall back to the shim's netns.
        Err(_) if !is_pinned_netns(Path::new(netns_path)) => Ok(None),
        Err(err) => Err(err).context("net netns guard"),
    }
}

#[async_trait]
impl Network for RestoredNetwork {
    async fn setup(&self) -> Result<()> {
        Err(anyhow!("a restored network cannot be set up again"))
    }

    async fn interfaces(&self) -> Result<Vec<agent::Interface>> {
        Err(anyhow!("a restored network has no live interfaces"))
    }

    async fn routes(&self) -> Result<Vec<agent::Route>> {
        Err(anyhow!("a restored network has no live routes"))
    }

    async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>> {
        Err(anyhow!("a restored network has no live neighbors"))
    }

    async fn save(&self) -> Option<Vec<EndpointState>> {
        Some(self.endpoints.clone())
    }

    async fn remove(&self, _h: &dyn Hypervisor) -> Result<()> {
        // The TAPs and the passed-through NICs are independent: a failure on
        // one side must not keep the other from being tried, only keep the
        // state around for a retry.
        let taps = match self.netns_path.as_deref() {
            Some(netns_path) => remove_kata_links(netns_path)
                .await
                .context("remove the sandbox's links from its namespace"),
            None => Ok(()),
        };
        if let Err(err) = &taps {
            // A rebind failure below would otherwise be the only error told.
            warn!(sl!(), "failed to remove the sandbox's links: {err:#}");
        }
        rebind_saved_physical_endpoints(&self.endpoints, driver::bind_device_to_host)?;
        taps
    }

    async fn netns_path(&self) -> Option<String> {
        self.netns_path.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::endpoint::endpoint_persist::PhysicalEndpointState;
    use hypervisor::qemu::Qemu;

    #[tokio::test]
    async fn a_proc_netns_reference_is_never_entered() {
        // /proc/1/ns/net exists, but it is not the recorded sandbox's
        // namespace: the pid outlived nothing of the sandbox, and after
        // reuse it can be anyone. The sweep must leave it alone.
        let network = RestoredNetwork::new(vec![], Some("/proc/1/ns/net".into()));
        network.remove(&Qemu::new()).await.unwrap();
    }

    #[tokio::test]
    async fn an_unmounted_netns_file_is_not_entered() {
        // What the owner of the namespace leaves between unmounting it and
        // removing its file: a plain file where the namespace was.
        let leftover = tempfile::NamedTempFile::new().unwrap();
        let path = leftover.path().to_str().unwrap().to_string();
        let network = RestoredNetwork::new(vec![], Some(path));
        network.remove(&Qemu::new()).await.unwrap();
    }

    #[test]
    fn a_netns_unmounted_after_the_initial_check_is_not_entered() {
        test_utils::skip_if_not_root!();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("netns");
        kata_sys_util::netns::pin_netns(Path::new("/proc/self/ns/net"), &path).unwrap();
        assert!(is_pinned_netns(&path));

        nix::mount::umount2(&path, nix::mount::MntFlags::MNT_DETACH).unwrap();
        assert!(path.exists());
        assert!(enter_cleanup_netns(path.to_str().unwrap())
            .unwrap()
            .is_none());

        std::fs::remove_file(&path).unwrap();
        assert!(enter_cleanup_netns(path.to_str().unwrap())
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn a_restored_network_saves_what_it_was_restored_from() {
        let endpoints = vec![EndpointState {
            physical_endpoint: Some(PhysicalEndpointState {
                bdf: "0000:41:00.2".to_string(),
                driver: "mlx5_core".to_string(),
            }),
        }];
        let network = RestoredNetwork::new(endpoints.clone(), Some("/run/netns/cni-1".into()));

        let saved = network.save().await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(
            saved[0].physical_endpoint.as_ref().unwrap().bdf,
            endpoints[0].physical_endpoint.as_ref().unwrap().bdf
        );
        assert!(network.setup().await.is_err());
        assert!(network.endpoints().await.is_empty());
    }
}
