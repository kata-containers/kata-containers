// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
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
use tokio::sync::RwLock;

use super::{
    endpoint::{
        Endpoint, IPVlanEndpoint, MacVlanEndpoint, PhysicalEndpoint, VethEndpoint, VlanEndpoint,
    },
    network_entity::NetworkEntity,
    network_info::network_info_from_link::{handle_addresses, NetworkInfoFromLink},
    utils::link,
    Network,
};
use crate::network::NetworkInfo;

#[derive(Debug)]
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
    async fn new(config: &NetworkWithNetNsConfig, d: Arc<RwLock<DeviceManager>>) -> Result<Self> {
        let entity_list = if config.netns_path.is_empty() {
            warn!(sl!(), "Skip to scan network for empty netns");
            vec![]
        } else if config.network_model.as_str() == "none" {
            warn!(
                sl!(),
                "Skip to scan network from netns due to the none network model"
            );
            vec![]
        } else {
            // get endpoint
            get_entity_from_netns(config, d)
                .await
                .context("get entity from netns")?
        };
        Ok(Self {
            netns_path: config.netns_path.to_string(),
            entity_list,
            network_created: config.network_created,
        })
    }
}

pub(crate) struct NetworkWithNetns {
    inner: Arc<RwLock<NetworkWithNetnsInner>>,
}

impl NetworkWithNetns {
    pub(crate) async fn new(
        config: &NetworkWithNetNsConfig,
        d: Arc<RwLock<DeviceManager>>,
    ) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(NetworkWithNetnsInner::new(config, d).await?)),
        })
    }
}

#[async_trait]
impl Network for NetworkWithNetns {
    async fn setup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let _netns_guard = netns::NetnsGuard::new(&inner.netns_path).context("net netns guard")?;
        for e in &inner.entity_list {
            e.endpoint.attach().await.context("attach")?;
        }
        Ok(())
    }

    async fn interfaces(&self) -> Result<Vec<agent::Interface>> {
        let inner = self.inner.read().await;
        let mut interfaces = vec![];
        for e in &inner.entity_list {
            interfaces.push(e.network_info.interface().await.context("interface")?);
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
        for e in &inner.entity_list {
            if let Some(state) = e.endpoint.save().await {
                endpoint.push(state);
            }
        }
        Some(endpoint)
    }

    async fn remove(&self, h: &dyn Hypervisor) -> Result<()> {
        let inner = self.inner.read().await;
        // The network namespace would have been deleted at this point
        // if it has not been created by virtcontainers.
        if !inner.network_created {
            return Ok(());
        }
        {
            let _netns_guard =
                netns::NetnsGuard::new(&inner.netns_path).context("net netns guard")?;
            for e in &inner.entity_list {
                e.endpoint.detach(h).await.context("detach")?;
            }
        }
        let netns = get_from_path(inner.netns_path.clone())?;
        netns.remove()?;
        fs::remove_dir_all(inner.netns_path.clone()).context("failed to remove netns path")?;
        Ok(())
    }
}

async fn get_entity_from_netns(
    config: &NetworkWithNetNsConfig,
    d: Arc<RwLock<DeviceManager>>,
) -> Result<Vec<NetworkEntity>> {
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
    while let Some(link) = links.try_next().await? {
        let link = link::get_link_from_message(link);
        let attrs = link.attrs();

        if (attrs.flags & libc::IFF_LOOPBACK as u32) != 0 {
            continue;
        }

        let ip_addresses = handle_addresses(&handle, attrs)
            .await
            .context("handle addresses")?;
        // Ignore unconfigured network interfaces. These are either base tunnel devices that are not namespaced
        // like gre0, gretap0, sit0, ipip0, tunl0 or incorrectly setup interfaces.
        if ip_addresses.is_empty() {
            continue;
        }

        let idx = idx.fetch_add(1, Ordering::Relaxed);
        let (endpoint, network_info) =
            create_endpoint(&handle, link.as_ref(), ip_addresses, idx, config, d.clone())
                .await
                .context("create endpoint")?;

        entity_list.push(NetworkEntity::new(endpoint, network_info));
    }

    Ok(entity_list)
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
        match link_type {
            "veth" => {
                let ret = VethEndpoint::new(
                    &d,
                    handle,
                    &attrs.name,
                    idx,
                    &config.network_model,
                    config.queues,
                )
                .await
                .context("veth endpoint")?;
                Arc::new(ret)
            }
            "vlan" => {
                let ret = VlanEndpoint::new(&d, handle, &attrs.name, idx, config.queues)
                    .await
                    .context("vlan endpoint")?;
                Arc::new(ret)
            }
            "ipvlan" => {
                let ret = IPVlanEndpoint::new(&d, handle, &attrs.name, idx, config.queues)
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
                    config.queues,
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
