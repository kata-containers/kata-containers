// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use endpoint::Endpoint;
use hypervisor::Hypervisor;
use scopeguard::defer;
use serde::{self, Deserialize};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use tokio::{fs, sync::RwLock};

use crate::network::endpoint::{self, TapEndpoint};

use super::{
    network_entity::NetworkEntity, network_info::DirectlyAttachableNetworkInfo, EndpointState,
    Network, NetworkInfo,
};

#[derive(Debug)]
pub struct DirectlyAttachableNetworkConfig {
    pub dan_conf_path: PathBuf,
}

pub struct DirectlyAttachableNetwork {
    inner: Arc<RwLock<DirectlyAttachableNetworkInner>>,
}

impl DirectlyAttachableNetwork {
    pub async fn new(config: &DirectlyAttachableNetworkConfig) -> Result<Self> {
        Ok(DirectlyAttachableNetwork {
            inner: Arc::new(RwLock::new(
                DirectlyAttachableNetworkInner::new(config).await?,
            )),
        })
    }
}

#[async_trait]
impl Network for DirectlyAttachableNetwork {
    async fn setup(&self, h: &dyn Hypervisor) -> Result<()> {
        let inner = self.inner.read().await;
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
}

pub struct DirectlyAttachableNetworkInner {
    entity_list: Vec<NetworkEntity>,
}

impl DirectlyAttachableNetworkInner {
    async fn new(config: &DirectlyAttachableNetworkConfig) -> Result<Self> {
        let mut idx = 0;
        let mut ret = DirectlyAttachableNetworkInner {
            entity_list: vec![],
        };
        let json_str = fs::read_to_string(&config.dan_conf_path).await?;
        let mut devices: Vec<DirectlyAttachableNetworkDevice> = serde_json::from_str(&json_str)?;
        info!(sl!(), "Dan devices are loaded = {:?}", devices);

        let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        for device in devices.iter_mut() {
            let entity: NetworkEntity;
            match device.r#type.as_str() {
                "tap" => {
                    let endpoint: Arc<dyn Endpoint> = Arc::new(
                        TapEndpoint::new(&handle, idx, &device.name)
                            .await
                            .context("Failed to create tap endpoint")?,
                    );

                    let network_info: Arc<dyn NetworkInfo> = match device.network_info.as_mut() {
                        Some(info) => {
                            info.set_name(endpoint.name().await.as_str());
                            info.set_hard_addr(endpoint.hardware_addr().await.as_str());
                            Arc::new(info.clone())
                        }
                        None => {
                            return Err(anyhow!("A network info is required to the tap devices"))
                        }
                    };

                    entity = NetworkEntity::new(endpoint, network_info);
                }
                _ => {
                    return Err(anyhow!(
                        "Unsupported network device, type = {}",
                        device.r#type
                    ))
                }
            }
            ret.entity_list.push(entity);
            idx += 1;
        }

        Ok(ret)
    }
}

/// DirectlyAttachableNetworkDevice represents a device encoded in the format of
/// JSON, and is set by the CNI plugins.
#[derive(Clone, Debug, Deserialize)]
pub struct DirectlyAttachableNetworkDevice {
    // Device name on the host
    name: String,
    // Device type
    r#type: String,
    // Device extra config set
    #[allow(dead_code)]
    dev_conf: Option<BTreeMap<String, String>>,
    // Network info
    network_info: Option<DirectlyAttachableNetworkInfo>,
}
