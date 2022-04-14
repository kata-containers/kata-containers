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
pub(crate) struct TcFilterModel {}

impl TcFilterModel {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl NetworkModel for TcFilterModel {
    fn model_type(&self) -> NetworkModelType {
        NetworkModelType::TcFilter
    }

    async fn add(&self, pair: &NetworkPair) -> Result<()> {
        let tap_name = &pair.tap.tap_iface.name;
        let virt_name = &pair.virt_iface.name;

        add_qdisc_ingress(tap_name)
            .await
            .context("add qdisc ingress for tap link")?;
        add_qdisc_ingress(virt_name)
            .await
            .context("add qdisc ingress")?;

        add_redirect_tcfilter(tap_name, virt_name)
            .await
            .context("add tc filter for tap")?;
        add_redirect_tcfilter(virt_name, tap_name)
            .await
            .context("add tc filter")?;
        Ok(())
    }

    async fn del(&self, pair: &NetworkPair) -> Result<()> {
        del_qdisc(&pair.virt_iface.name)
            .await
            .context("del qdisc")?;
        Ok(())
    }
}

// TODO: use netlink replace tc command
async fn add_qdisc_ingress(dev: &str) -> Result<()> {
    let output = Command::new("/sbin/tc")
        .args(&["qdisc", "add", "dev", dev, "handle", "ffff:", "ingress"])
        .output()
        .await
        .context("add tc")?;
    if !output.status.success() {
        return Err(anyhow!("{}", String::from_utf8(output.stderr)?));
    }
    Ok(())
}

async fn add_redirect_tcfilter(src: &str, dst: &str) -> Result<()> {
    let output = Command::new("/sbin/tc")
        .args(&[
            "filter", "add", "dev", src, "parent", "ffff:", "protocol", "all", "u32", "match",
            "u8", "0", "0", "action", "mirred", "egress", "redirect", "dev", dst,
        ])
        .output()
        .await
        .context("add redirect tcfilter")?;
    if !output.status.success() {
        return Err(anyhow!("{}", String::from_utf8(output.stderr)?));
    }
    Ok(())
}

async fn del_qdisc(dev: &str) -> Result<()> {
    let output = Command::new("/sbin/tc")
        .args(&["qdisc", "del", "dev", dev, "handle", "ffff:", "ingress"])
        .output()
        .await
        .context("del qdisc")?;
    if !output.status.success() {
        return Err(anyhow!("{}", String::from_utf8(output.stderr)?));
    }
    Ok(())
}
