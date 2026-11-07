// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use super::endpoint::endpoint_persist::EndpointState;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use kata_sys_util::netns;
use netns_rs::get_from_path;
use scopeguard::defer;
use tokio::sync::{Mutex, RwLock};

use super::{
    endpoint::{
        Endpoint, IPVlanEndpoint, MacVlanEndpoint, PhysicalEndpoint, VethEndpoint, VlanEndpoint,
    },
    network_entity::NetworkEntity,
    network_info::network_info_from_link::{handle_addresses, NetworkInfoFromLink},
    utils::link,
    Network,
};
use crate::network::{AttachProgress, NetworkInfo};

#[derive(Clone, Debug)]
pub struct NetworkWithNetNsConfig {
    pub network_model: String,
    pub netns_path: String,
    pub queues: usize,
    pub network_created: bool,
}

struct NetworkWithNetnsInner {
    netns_path: String,
    entity_list: Vec<NetworkEntity>,
    network_created: bool,
}

impl NetworkWithNetnsInner {
    async fn new(
        config: &NetworkWithNetNsConfig,
        d: Arc<RwLock<DeviceManager>>,
    ) -> Result<(Self, Option<anyhow::Error>)> {
        let (entity_list, construction_error) = if config.netns_path.is_empty() {
            warn!(sl!(), "Skip to scan network for empty netns");
            (vec![], None)
        } else if config.network_model.as_str() == "none" {
            warn!(
                sl!(),
                "Skip to scan network from netns due to the none network model"
            );
            (vec![], None)
        } else {
            // get endpoint
            get_entity_from_netns(config, d)
                .await
                .context("get entity from netns")?
        };
        Ok((
            Self {
                netns_path: config.netns_path.to_string(),
                entity_list,
                network_created: config.network_created,
            },
            construction_error,
        ))
    }
}

pub(crate) struct NetworkWithNetns {
    inner: Arc<RwLock<NetworkWithNetnsInner>>,
    /// The endpoints setup() got to attaching; see `AttachProgress`.
    attach_progress: AttachProgress,
    cleanup: Mutex<NetworkCleanup>,
    /// Why construction stopped early, when it did. The endpoints built
    /// before the failure are kept, so what they created reaches the saved
    /// state and the cleanup; setup() reports the failure instead of
    /// attaching anything, every time: a partially constructed network can
    /// never be attached whole, a resumed setup included.
    construction_error: Option<String>,
}

#[derive(Default)]
struct NetworkCleanup {
    /// Once a removal has begun, nothing may attach any more: setup() runs
    /// on a thread the caller may have been cancelled away from, and an
    /// attach after (or interleaved with) the removal would leave devices
    /// attached with their records gone.
    removal_started: bool,
    detached: HashSet<usize>,
    netns_unmounted: bool,
    path_removed: bool,
}

impl NetworkWithNetns {
    pub(crate) async fn new(
        config: &NetworkWithNetNsConfig,
        d: Arc<RwLock<DeviceManager>>,
    ) -> Result<Self> {
        let (inner, construction_error) = NetworkWithNetnsInner::new(config, d).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
            attach_progress: AttachProgress::default(),
            cleanup: Mutex::new(NetworkCleanup::default()),
            construction_error: construction_error.map(|e| format!("{e:#}")),
        })
    }
}

#[async_trait]
impl Network for NetworkWithNetns {
    async fn setup(&self) -> Result<()> {
        // A failed construction has nothing to attach: it kept the endpoints
        // it built so the caller could record them, and this is where their
        // builder's error surfaces.
        if let Some(msg) = self.construction_error.as_deref() {
            return Err(anyhow!("network construction: {msg}"));
        }
        // Held for the whole attach, so the removal of a sandbox whose
        // attach_network() caller was cancelled waits for this thread
        // instead of racing it, and an attach can never follow a removal.
        let cleanup = self.cleanup.lock().await;
        if cleanup.removal_started {
            return Err(anyhow!("the network is being removed, nothing may attach"));
        }
        let inner = self.inner.read().await;
        let _netns_guard = netns::NetnsGuard::new(&inner.netns_path).context("net netns guard")?;
        for (index, e) in inner.entity_list.iter().enumerate() {
            // A retried setup resumes after the endpoints it finished:
            // replaying an attach would hotplug its device a second time.
            if self.attach_progress.is_finished(index) {
                continue;
            }
            self.attach_progress.begin(index);
            if let Some(device_path) = e.endpoint.attach().await.context("attach")? {
                e.network_info
                    .set_device_path(device_path)
                    .await
                    .context("set device path")?;
            }
            self.attach_progress.finish(index);
        }
        Ok(())
    }

    async fn interfaces(&self) -> Result<Vec<agent::Interface>> {
        let inner = self.inner.read().await;
        let mut interfaces = vec![];
        for e in &inner.entity_list {
            let mut iface = e.network_info.interface().await.context("interface")?;
            // For cold-plugged physical (VFIO) endpoints, fill device_path
            // with the guest PCI path so the agent can do PCI-path-based MAC
            // reconciliation for IB/RoCE VFs. device_path is empty by default
            // because network_info_from_link builds the Interface before
            // attach() runs.
            if iface.device_path.is_empty() {
                if let Some(pci_path) = e.endpoint.guest_pci_path().await {
                    iface.device_path = pci_path;
                }
            }
            interfaces.push(iface);
        }
        Ok(interfaces)
    }

    async fn routes(&self) -> Result<Vec<agent::Route>> {
        let inner = self.inner.read().await;
        let mut routes = vec![];
        for e in &inner.entity_list {
            let mut list = e.network_info.routes().await.context("routes")?;
            routes.append(&mut list);
        }
        Ok(routes)
    }

    async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>> {
        let inner = self.inner.read().await;
        let mut neighs = vec![];
        for e in &inner.entity_list {
            let mut list = e.network_info.neighs().await.context("neighs")?;
            neighs.append(&mut list);
        }
        Ok(neighs)
    }

    async fn save(&self) -> Option<Vec<EndpointState>> {
        let inner = self.inner.read().await;
        let mut endpoint = vec![];
        for e in inner.entity_list.iter() {
            if let Some(state) = e.endpoint.save().await {
                endpoint.push(state);
            }
        }
        Some(endpoint)
    }

    async fn netns_path(&self) -> Option<String> {
        let inner = self.inner.read().await;
        Some(inner.netns_path.clone())
    }

    async fn remove(&self, h: &dyn Hypervisor) -> Result<()> {
        let mut cleanup = self.cleanup.lock().await;
        cleanup.removal_started = true;
        let inner = self.inner.read().await;
        let mut errors = Vec::new();
        // An endpoint whose attach never started has nothing to detach, but
        // a pair-based one still made its TAP at construction, and the TAP
        // outlives the sandbox when the namespace belongs to the caller. A
        // physical endpoint's constructor changes nothing on the host.
        let mut unattempted = HashSet::new();
        for (index, e) in inner.entity_list.iter().enumerate() {
            if self.attach_progress.includes(index) {
                continue;
            }
            unattempted.insert(index);
            if e.endpoint.host_bdf().await.is_some() {
                cleanup.detached.insert(index);
            }
        }

        // Always detach endpoints regardless of whether kata created the netns.
        // Physical endpoints rebind their VF from vfio-pci back to the original
        // host driver here.  Skipping this when network_created=false would
        // permanently leave VFs bound to vfio-pci after pod deletion.
        // A passed-through device goes back to its host driver without the
        // netns, which may be gone by now.
        for (index, e) in inner.entity_list.iter().enumerate() {
            if cleanup.detached.contains(&index) {
                continue;
            }
            if e.endpoint.host_bdf().await.is_none() {
                continue;
            }
            match e.endpoint.detach(h).await {
                Ok(()) => {
                    cleanup.detached.insert(index);
                }
                Err(err) => errors.push(err),
            }
        }

        let mut needs_netns = false;
        for (index, e) in inner.entity_list.iter().enumerate() {
            if !cleanup.detached.contains(&index) && e.endpoint.host_bdf().await.is_none() {
                needs_netns = true;
                break;
            }
        }
        if needs_netns {
            // The others work on interfaces by name, which means the wrong
            // namespace would find the wrong interface.
            match netns::NetnsGuard::new(&inner.netns_path) {
                Ok(_netns_guard) => {
                    for (index, e) in inner.entity_list.iter().enumerate() {
                        if cleanup.detached.contains(&index) {
                            continue;
                        }
                        if e.endpoint.host_bdf().await.is_some() {
                            continue;
                        }
                        let removed = if unattempted.contains(&index) {
                            e.endpoint.undo_construction().await
                        } else {
                            e.endpoint.detach(h).await
                        };
                        match removed {
                            Ok(()) => {
                                cleanup.detached.insert(index);
                            }
                            Err(err) => errors.push(err),
                        }
                    }
                }
                Err(_)
                    if !inner.network_created
                        && matches!(
                            std::fs::metadata(&inner.netns_path),
                            Err(err) if err.kind() == std::io::ErrorKind::NotFound
                        ) =>
                {
                    // CNI already removed its namespace and the interfaces
                    // inside it. Never enter the shim's own namespace to
                    // detach those interfaces by name.
                    info!(sl!(), "CNI netns {} already removed", inner.netns_path);
                    for (index, e) in inner.entity_list.iter().enumerate() {
                        if e.endpoint.host_bdf().await.is_none() {
                            cleanup.detached.insert(index);
                        }
                    }
                }
                Err(err) => return Err(err).context("net netns guard"),
            }
        }

        if !errors.is_empty() {
            for error in &errors {
                warn!(sl!(), "failed to detach endpoint: {error:#}");
            }
            return Err(anyhow!(
                "{} network endpoint(s) failed to detach",
                errors.len()
            ));
        }

        // Only delete the network namespace if kata created it.
        // If the CNI created the netns, it will be cleaned up by the CNI.
        if !inner.network_created {
            return Ok(());
        }
        if !cleanup.netns_unmounted {
            let netns = get_from_path(inner.netns_path.clone())?;
            netns.remove()?;
            cleanup.netns_unmounted = true;
        }
        if !cleanup.path_removed {
            hypervisor::utils::remove_dir_all_if_exists(&inner.netns_path)
                .context("failed to remove netns path")?;
            cleanup.path_removed = true;
        }
        Ok(())
    }

    async fn endpoints(&self) -> Vec<std::sync::Arc<dyn crate::network::endpoint::Endpoint>> {
        let inner = self.inner.read().await;
        inner
            .entity_list
            .iter()
            .map(|e| e.endpoint.clone())
            .collect()
    }
}

/// Lightweight probe: enter the netns and check whether any non-loopback
/// interface with at least one IP address exists.  Does NOT create endpoints
/// or attach anything to the hypervisor.
pub(crate) async fn netns_has_interfaces(netns_path: &str) -> Result<bool> {
    let _netns_guard = netns::NetnsGuard::new(netns_path).context("netns guard for scan")?;
    let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
    let thread_handler = tokio::spawn(connection);
    defer!({
        thread_handler.abort();
    });

    let mut links = handle.link().get().execute();
    while let Some(msg) = links.try_next().await? {
        let link = link::get_link_from_message(msg);
        let attrs = link.attrs();
        if (attrs.flags & libc::IFF_LOOPBACK as u32) != 0 {
            continue;
        }
        let addrs = handle_addresses(&handle, attrs)
            .await
            .context("handle addresses")?;
        if !addrs.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Build the endpoints of the namespace's interfaces. An endpoint that could
/// not be built does not take the already built ones with it: building a
/// pair-based endpoint creates its TAP, and discarding the endpoint object
/// would leave the TAP with no record naming it. The error comes back next
/// to whatever was built before it.
async fn get_entity_from_netns(
    config: &NetworkWithNetNsConfig,
    d: Arc<RwLock<DeviceManager>>,
) -> Result<(Vec<NetworkEntity>, Option<anyhow::Error>)> {
    info!(
        sl!(),
        "get network entity from config {:?} tid {:?}",
        config,
        nix::unistd::gettid()
    );
    let mut entity_list = vec![];
    let _netns_guard = netns::NetnsGuard::new(&config.netns_path)
        .context("net netns guard")
        .unwrap();
    let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
    let thread_handler = tokio::spawn(connection);
    defer!({
        thread_handler.abort();
    });

    let mut links = handle.link().get().execute();

    let idx = AtomicU32::new(0);
    let mut failure: Option<anyhow::Error> = None;
    loop {
        let link = match links.try_next().await {
            Ok(Some(link)) => link,
            Ok(None) => break,
            Err(e) => {
                failure = Some(anyhow!(e).context("list links"));
                break;
            }
        };
        let link = link::get_link_from_message(link);
        let attrs = link.attrs();

        if (attrs.flags & libc::IFF_LOOPBACK as u32) != 0 {
            continue;
        }

        let ip_addresses = match handle_addresses(&handle, attrs)
            .await
            .context("handle addresses")
        {
            Ok(addresses) => addresses,
            Err(e) => {
                failure = Some(e);
                break;
            }
        };
        // Ignore unconfigured network interfaces. These are either base tunnel devices that are not namespaced
        // like gre0, gretap0, sit0, ipip0, tunl0 or incorrectly setup interfaces.
        if ip_addresses.is_empty() {
            continue;
        }

        let idx = idx.fetch_add(1, Ordering::Relaxed);
        match create_endpoint(&handle, link.as_ref(), ip_addresses, idx, config, d.clone())
            .await
            .context("create endpoint")
        {
            Ok((endpoint, network_info)) => {
                entity_list.push(NetworkEntity::new(endpoint, network_info));
            }
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }

    // Currently, l3forwarding configures the host netns for a single pod interface.
    if failure.is_none()
        && config.network_model == crate::network::network_model::L3_FORWARDING_NET_MODEL_STR
        && entity_list.len() > 1
    {
        failure = Some(anyhow!(
            "l3forwarding supports only a single interface per network namespace, found {}",
            entity_list.len()
        ));
    }

    Ok((entity_list, failure))
}

async fn create_endpoint(
    handle: &rtnetlink::Handle,
    link: &dyn link::Link,
    addrs: Vec<agent::IPAddress>,
    idx: u32,
    config: &NetworkWithNetNsConfig,
    d: Arc<RwLock<DeviceManager>>,
) -> Result<(Arc<dyn Endpoint>, Arc<dyn NetworkInfo>)> {
    let _netns_guard = netns::NetnsGuard::new(&config.netns_path)
        .context("net netns guard")
        .unwrap();
    let attrs = link.attrs();
    let link_type = link.r#type();
    let endpoint: Arc<dyn Endpoint> = if is_physical_iface(&attrs.name)? {
        info!(
            sl!(),
            "physical network interface found: {} {:?}",
            &attrs.name,
            nix::unistd::gettid()
        );
        let t = PhysicalEndpoint::new(&attrs.name, &attrs.hardware_addr, d)
            .context("new physical endpoint")?;
        Arc::new(t)
    } else {
        info!(
            sl!(),
            "{} network interface found: {}", &link_type, &attrs.name
        );
        let queues = d.read().await.network_queue_pairs(config.queues).await;
        match link_type {
            // "device" is the generic netlink type for interfaces whose
            // drivers do not register a more specific kind. This includes
            // mlx5 Scalable Functions (SFs) and other non-PCI backed
            // netdevs that is_physical_iface() cannot classify via
            // ethtool BusInfo. Handle them the same way as veth
            // endpoints, using the configured network model.
            "veth" | "device" => {
                let ret =
                    VethEndpoint::new(&d, handle, &attrs.name, idx, &config.network_model, queues)
                        .await
                        .context(anyhow!("{link_type} endpoint"))?;
                Arc::new(ret)
            }
            "vlan" => {
                let ret = VlanEndpoint::new(&d, handle, &attrs.name, idx, queues)
                    .await
                    .context("vlan endpoint")?;
                Arc::new(ret)
            }
            "ipvlan" => {
                let ret = IPVlanEndpoint::new(&d, handle, &attrs.name, idx, queues)
                    .await
                    .context("ipvlan endpoint")?;
                Arc::new(ret)
            }
            "macvlan" => {
                let ret = MacVlanEndpoint::new(
                    &d,
                    handle,
                    &attrs.name,
                    idx,
                    &config.network_model,
                    queues,
                )
                .await
                .context("macvlan endpoint")?;
                Arc::new(ret)
            }
            _ => return Err(anyhow!("unsupported link type: {}", link_type)),
        }
    };

    let network_info = Arc::new(
        NetworkInfoFromLink::new(handle, link, addrs, &endpoint.hardware_addr().await)
            .await
            .context("network info from link")?,
    );

    info!(sl!(), "network info {:?}", network_info);

    Ok((endpoint, network_info))
}

fn is_physical_iface(name: &str) -> Result<bool> {
    if name == "lo" {
        return Ok(false);
    }
    let driver_info = link::get_driver_info(name)?;
    if driver_info.bus_info.split(':').count() != 3 {
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    use agent::{ARPNeighbor, Interface, Route};
    use hypervisor::qemu::Qemu;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Default)]
    struct TestEndpoint {
        calls: AtomicUsize,
        attaches: AtomicUsize,
        undone: AtomicUsize,
        fail_once: bool,
        attach_fails: bool,
        attach_fails_once: bool,
        /// A pair-based endpoint: no host BDF, and a TAP made at
        /// construction for undo_construction() to remove.
        pair_based: bool,
    }

    #[async_trait]
    impl Endpoint for TestEndpoint {
        async fn name(&self) -> String {
            "eth0".into()
        }
        async fn hardware_addr(&self) -> String {
            String::new()
        }
        async fn attach(&self) -> Result<Option<String>> {
            let call = self.attaches.fetch_add(1, Ordering::SeqCst);
            if self.attach_fails || (self.attach_fails_once && call == 0) {
                Err(anyhow!("no room for the device"))
            } else {
                Ok(None)
            }
        }
        async fn detach(&self, _: &dyn Hypervisor) -> Result<()> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_once && call == 0 {
                Err(anyhow!("device still busy"))
            } else {
                Ok(())
            }
        }
        async fn undo_construction(&self) -> Result<()> {
            self.undone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn save(&self) -> Option<EndpointState> {
            Some(EndpointState::default())
        }
        async fn host_bdf(&self) -> Option<String> {
            if self.pair_based {
                None
            } else {
                Some("0000:00:00.1".into())
            }
        }
    }

    #[derive(Debug)]
    struct TestNetworkInfo;

    #[async_trait]
    impl NetworkInfo for TestNetworkInfo {
        async fn interface(&self) -> Result<Interface> {
            Ok(Interface::default())
        }
        async fn routes(&self) -> Result<Vec<Route>> {
            Ok(vec![])
        }
        async fn neighs(&self) -> Result<Vec<ARPNeighbor>> {
            Ok(vec![])
        }
    }

    fn network(endpoints: &[Arc<TestEndpoint>]) -> NetworkWithNetns {
        NetworkWithNetns {
            inner: Arc::new(RwLock::new(NetworkWithNetnsInner {
                netns_path: String::new(),
                entity_list: endpoints
                    .iter()
                    .map(|e| NetworkEntity::new(e.clone(), Arc::new(TestNetworkInfo)))
                    .collect(),
                network_created: false,
            })),
            attach_progress: AttachProgress::default(),
            cleanup: Mutex::new(NetworkCleanup::default()),
            construction_error: None,
        }
    }

    #[tokio::test]
    async fn a_failed_construction_keeps_its_endpoints_and_attaches_nothing() {
        let built = Arc::new(TestEndpoint {
            pair_based: true,
            ..Default::default()
        });
        let mut network = network(std::slice::from_ref(&built));
        network.construction_error = Some("one endpoint could not be built".to_string());

        // The builder's error surfaces here, and no attach was tried; a
        // retried setup gets it again, so a resume cannot silently attach a
        // partial network.
        let err = network.setup().await.unwrap_err();
        assert!(err.to_string().contains("could not be built"));
        let err = network.setup().await.unwrap_err();
        assert!(err.to_string().contains("could not be built"));

        // What construction made is still there for the cleanup to undo.
        network.remove(&Qemu::new()).await.unwrap();
        assert_eq!(built.undone.load(Ordering::SeqCst), 1);
        assert_eq!(built.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn failed_detach_retries_without_repeating_successful_detach() {
        let done = Arc::new(TestEndpoint::default());
        let retry = Arc::new(TestEndpoint {
            fail_once: true,
            ..Default::default()
        });
        let network = network(&[done.clone(), retry.clone()]);
        network.setup().await.unwrap();
        let hypervisor = Qemu::new();

        assert!(network.remove(&hypervisor).await.is_err());
        network.remove(&hypervisor).await.unwrap();
        network.remove(&hypervisor).await.unwrap();
        assert_eq!(done.calls.load(Ordering::SeqCst), 1);
        assert_eq!(retry.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn no_attach_can_follow_a_removal() {
        let late = Arc::new(TestEndpoint::default());
        let network = network(std::slice::from_ref(&late));

        // The attach may run on a thread whose caller was cancelled, so it
        // can arrive after the sandbox's removal: it must refuse, or the
        // device would be attached with its record already gone.
        network.remove(&Qemu::new()).await.unwrap();
        let err = network.setup().await.unwrap_err();
        assert!(err.to_string().contains("being removed"));
        assert_eq!(late.attaches.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_retried_setup_resumes_after_the_attached_endpoints() {
        let done = Arc::new(TestEndpoint::default());
        let retry = Arc::new(TestEndpoint {
            attach_fails_once: true,
            ..Default::default()
        });
        let untouched = Arc::new(TestEndpoint::default());
        let network = network(&[done.clone(), retry.clone(), untouched.clone()]);

        assert!(network.setup().await.is_err());
        network.setup().await.unwrap();
        // The endpoint that attached is not attached again; the failed one
        // is retried and the rest follow.
        assert_eq!(done.attaches.load(Ordering::SeqCst), 1);
        assert_eq!(retry.attaches.load(Ordering::SeqCst), 2);
        assert_eq!(untouched.attaches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_setup_records_and_detaches_what_it_attached() {
        let attached = Arc::new(TestEndpoint::default());
        let failed = Arc::new(TestEndpoint {
            attach_fails: true,
            ..Default::default()
        });
        let untouched = Arc::new(TestEndpoint::default());
        let network = network(&[attached.clone(), failed.clone(), untouched.clone()]);

        assert!(network.setup().await.is_err());
        // The cleanup detaches the endpoints whose attach started and only
        // undoes the construction of the one whose attach never did.
        network.remove(&Qemu::new()).await.unwrap();
        assert_eq!(attached.calls.load(Ordering::SeqCst), 1);
        assert_eq!(failed.calls.load(Ordering::SeqCst), 1);
        assert_eq!(untouched.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_unattempted_pair_endpoint_loses_its_tap_and_keeps_its_driver() {
        let attached = Arc::new(TestEndpoint {
            pair_based: true,
            ..Default::default()
        });
        let failed = Arc::new(TestEndpoint {
            attach_fails: true,
            ..Default::default()
        });
        let unattempted = Arc::new(TestEndpoint {
            pair_based: true,
            ..Default::default()
        });
        let network = network(&[attached.clone(), failed.clone(), unattempted.clone()]);

        assert!(network.setup().await.is_err());
        network.remove(&Qemu::new()).await.unwrap();
        // The attached endpoints detach; the unattempted one only has its
        // construction undone, never a detach.
        assert_eq!(attached.calls.load(Ordering::SeqCst), 1);
        assert_eq!(attached.undone.load(Ordering::SeqCst), 0);
        assert_eq!(failed.calls.load(Ordering::SeqCst), 1);
        assert_eq!(unattempted.calls.load(Ordering::SeqCst), 0);
        assert_eq!(unattempted.undone.load(Ordering::SeqCst), 1);
        // A retry does not undo the construction twice.
        network.remove(&Qemu::new()).await.unwrap();
        assert_eq!(unattempted.undone.load(Ordering::SeqCst), 1);
    }
}
