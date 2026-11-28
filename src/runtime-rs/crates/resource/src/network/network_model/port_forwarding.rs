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
    // The NAT is set up once per pod netns: with several l3forwarding interfaces, the first one
    // to get here sets it up for its pod address and the others leave it alone. The jump from
    // OUTPUT is the last rule added below, so it is only there if a setup completed.
    if run_iptables(
        iptables,
        &["-t", "nat", "-C", "OUTPUT", "-j", KATA_OUTPUT_CHAIN],
    )
    .await
    .is_ok()
    {
        return Ok(());
    }
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
    // Note that "-w" is a no-op for iptables-nft.
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

// Simplified version of what Istio does in
// https://github.com/istio/istio/blob/1ef2d93c3b1ba12922d2e8e1384e4ec7317a973f/tools/istio-iptables/pkg/dependencies/implementation.go#L179-L210
// In short, if "iptables-legacy" is available and has rules, use it.
// If "iptables-nft" is available, use it.
// If "iptables" (which symlinks to iptables-nft or iptables) is available, use it.
// Finally, fail.
// This error should be caught and logged, but should not be fatal.
async fn detect_iptables_binary() -> Result<&'static str> {
    let [legacy, plain, nft] = ["iptables-legacy", "iptables", "iptables-nft"];

    let legacy_output = probe_binary(legacy).await;
    if legacy_output.as_deref().is_some_and(has_rules) {
        return Ok(legacy);
    }

    let nft_output = probe_binary(nft).await;
    if nft_output.is_some() {
        return Ok(nft);
    }

    if probe_binary(plain).await.is_some() {
        return Ok(plain);
    }

    Err(anyhow!(
        "no usable IPv4 iptables binary found (tried {}, {}, {})",
        legacy,
        nft,
        plain
    ))
}

// Check if the output of iptablesxxx-save has any rules.
// Istio does the same heuristic.
// https://github.com/istio/istio/blob/1ef2d93c3b1ba12922d2e8e1384e4ec7317a973f/tools/istio-iptables/pkg/dependencies/implementation_linux.go#L140-L144
fn has_rules(output: &str) -> bool {
    output.matches('\n').count() >= 3
}

// We assume that iptablesxxx-save runs iff iptablesxxx is usable.
async fn probe_binary(binary: &'static str) -> Option<String> {
    let save_binary = format!("{binary}-save");
    match Command::new(save_binary).output().await {
        Ok(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::skip_if_not_root;

    /// Runs `body` with the calling thread in a throwaway network namespace, whose iptables
    /// rules are its own.
    async fn in_netns<F: std::future::Future<Output = ()>>(tag: &str, body: F) {
        let name = format!("kata-portfw-{}-{}", tag, std::process::id());
        let netns = scopeguard::guard(
            netns_rs::NetNs::new(&name).expect("create netns"),
            |netns| {
                let _ = netns.remove();
            },
        );
        let _guard = kata_sys_util::netns::NetnsGuard::new(netns.path().to_str().unwrap())
            .expect("enter netns");
        body.await;
    }

    /// The iptables binary the NAT setup would use in the current network namespace, or `None`
    /// if there is none, in which case the test has nothing to check: port forwarding is skipped
    /// the same way. The choice depends on the rules in the namespace, so call it from inside
    /// the test's namespace.
    async fn iptables_backend() -> Option<&'static str> {
        let backend = detect_iptables_binary().await.ok();
        if backend.is_none() {
            eprintln!("skipping: no usable iptables binary");
        }
        backend
    }

    async fn nat_rules(iptables: &str) -> String {
        let output = Command::new(iptables)
            .args(["-t", "nat", "-S"])
            .output()
            .await
            .expect("list nat rules");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Requires root: creating a netns needs CAP_SYS_ADMIN.
    #[actix_rt::test]
    async fn nat_is_set_up_once_per_netns() {
        skip_if_not_root!();
        in_netns("once", async {
            let Some(iptables) = iptables_backend().await else {
                return;
            };
            configure_ipv4("tap0_kata", Ipv4Addr::new(10, 0, 1, 5), TAP_IPV4_ADDR)
                .await
                .expect("set up the NAT for the first interface");
            configure_ipv4("tap1_kata", Ipv4Addr::new(10, 0, 2, 5), TAP_IPV4_ADDR)
                .await
                .expect("a second interface leaves the NAT alone");

            let rules = nat_rules(iptables).await;
            assert_eq!(
                rules.matches("-A OUTPUT -j KATA_PORTFW_OUTPUT").count(),
                1,
                "{}",
                rules
            );
            assert!(!rules.contains("tap1_kata"), "{}", rules);
        })
        .await;
    }

    /// A setup that stopped after creating its first chain is not taken as done.
    ///
    /// Requires root: creating a netns needs CAP_SYS_ADMIN.
    #[actix_rt::test]
    async fn partial_nat_setup_is_not_taken_as_done() {
        skip_if_not_root!();
        in_netns("partial", async {
            let Some(iptables) = iptables_backend().await else {
                return;
            };
            run_iptables(iptables, &["-t", "nat", "-N", KATA_POSTROUTING_CHAIN])
                .await
                .expect("create the first chain");
            assert!(
                configure_ipv4("tap0_kata", Ipv4Addr::new(10, 0, 1, 5), TAP_IPV4_ADDR)
                    .await
                    .is_err(),
                "a partial setup was taken as done"
            );
        })
        .await;
    }
}
