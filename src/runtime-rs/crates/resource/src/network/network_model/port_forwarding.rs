// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::net::Ipv4Addr;

use anyhow::{anyhow, Result};
use tokio::process::Command;

const IPV4_LOOPBACK: &str = "127.0.0.1/32";
const KATA_OUTPUT_CHAIN: &str = "KATA_PORTFW_OUTPUT";
const KATA_POSTROUTING_CHAIN: &str = "KATA_PORTFW_POSTRT";
const XTABLES_LOCK_WAIT_SECONDS: &str = "5";
pub(crate) const TAP_IPV4_ADDR: Ipv4Addr = Ipv4Addr::new(169, 254, 0, 1);

pub(crate) async fn configure_port_forwarding(
    tap_name: &str,
    pod_ipv4: Option<Ipv4Addr>,
    tap_ipv4: Ipv4Addr,
) {
    if let Some(pod_ip) = pod_ipv4 {
        if let Err(err) = configure_ipv4(tap_name, pod_ip, tap_ipv4).await {
            warn!(
                sl!(),
                "failed to configure IPv4 port-forward NAT; continuing without it: {}", err
            );
        }
    }
}

async fn configure_ipv4(tap_name: &str, pod_ip: Ipv4Addr, tap_ip: Ipv4Addr) -> Result<()> {
    let iptables = detect_iptables_binary().await?;
    let pod_cidr = format!("{pod_ip}/32");
    let tap_ip = tap_ip.to_string();
    let pod_ip = pod_ip.to_string();

    run_iptables(iptables, &["-t", "nat", "-N", KATA_POSTROUTING_CHAIN]).await?;
    run_iptables(iptables, &["-t", "nat", "-N", KATA_OUTPUT_CHAIN]).await?;

    // Make replies from the guest routable back to the host-side pod
    // namespace. Install this before DNAT so a partial failure cannot leave
    // destination rewriting active without a valid reply path.
    run_iptables(
        iptables,
        &[
            "-t",
            "nat",
            "-A",
            KATA_POSTROUTING_CHAIN,
            "-s",
            IPV4_LOOPBACK,
            "-d",
            &pod_cidr,
            "-o",
            tap_name,
            "-p",
            "tcp",
            "-j",
            "SNAT",
            "--to-source",
            &tap_ip,
        ],
    )
    .await?;
    run_iptables(
        iptables,
        &[
            "-t",
            "nat",
            "-A",
            "POSTROUTING",
            "-j",
            KATA_POSTROUTING_CHAIN,
        ],
    )
    .await?;

    // containerd port-forward connects to localhost inside the pod network
    // namespace. Preserve the requested port while redirecting that
    // connection to the workload inside the guest.
    run_iptables(
        iptables,
        &[
            "-t",
            "nat",
            "-A",
            KATA_OUTPUT_CHAIN,
            "-s",
            IPV4_LOOPBACK,
            "-d",
            IPV4_LOOPBACK,
            "-p",
            "tcp",
            "-j",
            "DNAT",
            "--to-destination",
            &pod_ip,
        ],
    )
    .await?;
    run_iptables(
        iptables,
        &["-t", "nat", "-A", "OUTPUT", "-j", KATA_OUTPUT_CHAIN],
    )
    .await
}

async fn run_iptables(binary: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(binary)
        .args(["-w", XTABLES_LOCK_WAIT_SECONDS])
        .args(args)
        .output()
        .await
        .map_err(|err| anyhow!("failed to execute {binary}: {err}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "{} {} failed: {}",
            binary,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

async fn detect_iptables_binary() -> Result<&'static str> {
    let [legacy, nft, plain] = ["iptables-legacy", "iptables-nft", "iptables"];

    let legacy_output = probe_binary(legacy).await;
    if legacy_output.as_deref().is_some_and(has_rules) {
        return Ok(legacy);
    }

    let nft_output = probe_binary(nft).await;
    if nft_output.as_deref().is_some_and(has_rules) {
        return Ok(nft);
    }

    if probe_binary(plain).await.is_some() {
        return Ok(plain);
    }

    if nft_output.is_some() {
        return Ok(nft);
    }

    if legacy_output.is_some() {
        return Ok(legacy);
    }

    Err(anyhow!(
        "no usable IPv4 iptables binary found (tried {}, {}, {})",
        legacy,
        nft,
        plain
    ))
}

fn has_rules(output: &str) -> bool {
    output.lines().any(|line| line.starts_with("-A "))
}

async fn probe_binary(binary: &'static str) -> Option<String> {
    let save_binary = format!("{binary}-save");
    match Command::new(save_binary).output().await {
        Ok(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        _ => None,
    }
}
