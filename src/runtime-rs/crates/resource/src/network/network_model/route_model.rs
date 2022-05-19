// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use super::{NetworkModel, NetworkModelType};
use crate::network::NetworkPair;

#[derive(Debug)]
pub(crate) struct RouteModel {}

impl RouteModel {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl NetworkModel for RouteModel {
    fn model_type(&self) -> NetworkModelType {
        NetworkModelType::Route
    }

    async fn add(&self, pair: &NetworkPair) -> Result<()> {
        let tap_name = &pair.tap.tap_iface.name;
        let virt_name = &pair.virt_iface.name;
        let virt_iface_addr = pair.virt_iface.addrs[0].addr.to_string();

        let commands_args = vec![
            vec![
                "rule", "add", "pref", "10", "from", "all", "lookup", "local",
            ],
            vec!["rule", "del", "pref", "0", "from", "all"],
            vec!["rule", "add", "pref", "5", "iif", virt_name, "table", "10"],
            vec![
                "route", "replace", "default", "dev", tap_name, "table", "10",
            ],
            vec![
                "neigh",
                "replace",
                &virt_iface_addr,
                "lladdr",
                &pair.virt_iface.hard_addr,
                "dev",
                tap_name,
            ],
        ];

        for ca in commands_args {
            let output = Command::new("/sbin/ip")
                .args(&ca)
                .output()
                .await
                .with_context(|| format!("run command ip args {:?}", &ca))?;
            if !output.status.success() {
                return Err(anyhow!(
                    "run command ip args {:?} error {}",
                    &ca,
                    String::from_utf8(output.stderr)?
                ));
            }
        }

        // TODO: support ipv6
        // change sysctl for tap0_kata
        // echo 1 > /proc/sys/net/ipv4/conf/tap0_kata/accept_local
        let accept_local_path = format!("/proc/sys/net/ipv4/conf/{}/accept_local", &tap_name);
        std::fs::write(&accept_local_path, "1".to_string())
            .with_context(|| format!("Failed to echo 1 > {}", &accept_local_path))?;

        // echo 1 > /proc/sys/net/ipv4/conf/eth0/proxy_arp
        // This enabled ARP reply on peer eth0 to prevent without any reply on VPC
        let proxy_arp_path = format!("/proc/sys/net/ipv4/conf/{}/proxy_arp", &virt_name);
        std::fs::write(&proxy_arp_path, "1".to_string())
            .with_context(|| format!("Failed to echo 1 > {}", &proxy_arp_path))?;

        Ok(())
    }

    async fn del(&self, _pair: &NetworkPair) -> Result<()> {
        todo!()
    }
}
