// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use hypervisor::Hypervisor;
use scopeguard::defer;
use tokio::sync::RwLock;

use super::{
    endpoint::{Endpoint, PhysicalEndpoint, VethEndpoint},
    network_entity::NetworkEntity,
    network_info::network_info_from_link::NetworkInfoFromLink,
    utils::{link, netns},
    Network,
};
use crate::network::NetworkInfo;

#[derive(Debug)]
pub struct NetworkWithNetNsConfig {
    pub network_model: String,
    pub netns_path: String,
    pub queues: usize,
}

struct NetworkWithNetnsInner {
    netns_path: String,
    entity_list: Vec<NetworkEntity>,
}

impl NetworkWithNetnsInner {
    async fn new(config: &NetworkWithNetNsConfig) -> Result<Self> {
        let entity_list = if config.netns_path.is_empty() {
            warn!(sl!(), "skip to scan for empty netns");
            vec![]
        } else {
            // get endpoint
            get_entity_from_netns(config)
                .await
                .context("get entity from netns")?
        };
        Ok(Self {
            netns_path: config.netns_path.to_string(),
            entity_list,
        })
    }
}

pub(crate) struct NetworkWithNetns {
    inner: Arc<RwLock<NetworkWithNetnsInner>>,
}

impl NetworkWithNetns {
    pub(crate) async fn new(config: &NetworkWithNetNsConfig) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(NetworkWithNetnsInner::new(config).await?)),
        })
    }
}

#[async_trait]
impl Network for NetworkWithNetns {
    async fn setup(&self, h: &dyn Hypervisor) -> Result<()> {
        let inner = self.inner.read().await;
        let _netns_guard = netns::NetnsGuard::new(&inner.netns_path).context("net netns guard")?;
        for e in &inner.entity_list {
            e.endpoint.attach(h).await.context("attach")?;
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
}

async fn get_entity_from_netns(config: &NetworkWithNetNsConfig) -> Result<Vec<NetworkEntity>> {
    info!(
        sl!(),
        "get network entity for config {:?} tid {:?}",
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

        let idx = idx.fetch_add(1, Ordering::Relaxed);
        let (endpoint, network_info) = create_endpoint(&handle, link.as_ref(), idx, config)
            .await
            .context("create endpoint")?;

        entity_list.push(NetworkEntity::new(endpoint, network_info));
    }

    Ok(entity_list)
}

async fn create_endpoint(
    handle: &rtnetlink::Handle,
    link: &dyn link::Link,
    idx: u32,
    config: &NetworkWithNetNsConfig,
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
        let t = PhysicalEndpoint::new(&attrs.name, &attrs.hardware_addr)
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
            _ => return Err(anyhow!("unsupported link type: {}", link_type)),
        }
    };

    let network_info = Arc::new(
        NetworkInfoFromLink::new(handle, link, &endpoint.hardware_addr().await)
            .await
            .context("network info from link")?,
    );

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
