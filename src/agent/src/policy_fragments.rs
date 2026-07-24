// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

//! BL-8 — boot-time pull, verify, and inject of declared signed policy fragments.
//!
//! The measured base policy (set from init-data before this runs) may declare
//! `data.agent_policy.policy_fragments[]` entries — each naming an `issuer`
//! (`did:x509`), a `feed` (OCI reference), and a `minimum_svn`. For every such entry this
//! module:
//!   1. pulls the COSE_Sign1(rego) fragment artifact from the feed's OCI registry,
//!   2. reconstructs the `PolicyFragment` from the (untrusted) COSE payload,
//!   3. verifies it through the **coco-parity SRM `FragmentStore`** — the *same*
//!      verify → apply → commit sequence the runtime ttRPC push path
//!      (`rpc::load_policy_fragment`) uses, so FR-1d (did:x509), FR-1f (transparency
//!      receipts), FR-1i (rollback floor), and FR-1j (append-only ordering) all apply to
//!      OCI-delivered fragments too — and injects the verified Rego module.
//!
//! Everything is fail-closed: if any declared fragment cannot be fetched, verified, or
//! injected, the whole operation returns `Err` and the boot path aborts the VM rather than
//! serving requests under a partially-composed policy. Both delivery paths (boot pull and
//! runtime push) share the single global `FRAGMENTS` store, so FR-1j ordering and FR-1i
//! rollback state remain one monotonic chain.

use anyhow::{anyhow, bail, Context, Result};
use oci_client::client::{ClientConfig, ClientProtocol};
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};
use slog::info;

use crate::{AGENT_POLICY, FRAGMENTS};

/// OCI layer media type carrying the COSE_Sign1(rego) fragment envelope.
const COSE_LAYER_MEDIA_TYPE: &str = "application/cose-x509+rego";
/// Expected OCI artifactType for a kata policy fragment.
const FRAGMENT_ARTIFACT_TYPE: &str = "application/x-ms-ccepolicy-frag";

macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

/// Fetch, verify, and inject every fragment the measured base policy declares.
///
/// Returns the number of fragments injected (0 when none are declared). Any failure is
/// fatal and propagated so the boot path can fail closed. Declarations are processed in
/// base-policy order so the single FR-1j ordering chain advances deterministically.
pub async fn load_declared_fragments() -> Result<usize> {
    let specs = {
        let mut policy = AGENT_POLICY.lock().await;
        policy
            .fragment_specs()
            .context("reading policy_fragments from the base policy")?
    };

    if specs.is_empty() {
        info!(sl!(), "policy-fragments: base policy declares no fragments");
        return Ok(0);
    }

    info!(
        sl!(),
        "policy-fragments: base policy declares {} fragment(s)",
        specs.len()
    );

    let mut injected = 0usize;
    for spec in &specs {
        let cose = fetch_fragment(&spec.feed)
            .await
            .with_context(|| format!("fetching fragment for feed {}", spec.feed))?;
        verify_and_inject(&spec.issuer, &spec.feed, spec.minimum_svn, &cose)
            .await
            .with_context(|| format!("verifying/injecting fragment for feed {}", spec.feed))?;
        injected += 1;
    }

    info!(
        sl!(),
        "policy-fragments: injected {}/{} verified fragment(s)", injected, specs.len()
    );
    Ok(injected)
}

/// Verify an OCI-pulled COSE fragment through the SRM and inject it, mirroring
/// `rpc::load_policy_fragment` (verify → apply → commit, atomic and fail-closed).
async fn verify_and_inject(
    decl_issuer: &str,
    decl_feed: &str,
    minimum_svn: u64,
    cose_sign1: &[u8],
) -> Result<()> {
    // Reconstruct the fragment from the (untrusted) COSE payload. The SRM re-verifies the
    // COSE signature against exactly these reconstructed fields, so a forged payload cannot
    // pass; parsing here only tells us which fields the envelope claims to bind.
    let fragment = kata_security_reference_monitor::PolicyFragment::from_cose_envelope(cose_sign1)
        .ok_or_else(|| anyhow!("fragment COSE envelope is not a kata-policy-fragment/v3 statement"))?;

    // The pulled artifact must match what the measured base policy declared. This is a
    // defence-in-depth cross-check on top of the SRM's own issuer/feed/SVN gates.
    if fragment.issuer != decl_issuer {
        bail!(
            "pulled fragment issuer {:?} does not match declared issuer {:?}",
            fragment.issuer,
            decl_issuer
        );
    }
    if fragment.feed != decl_feed {
        bail!(
            "pulled fragment feed {:?} does not match declared feed {:?}",
            fragment.feed,
            decl_feed
        );
    }
    if fragment.svn < minimum_svn {
        bail!(
            "pulled fragment svn {} is below declared minimum_svn {}",
            fragment.svn,
            minimum_svn
        );
    }

    // Verify through the SRM. Routing is identical to the runtime push path: an
    // x5chain-bearing envelope (or a store requiring x509) is always verified as did:x509;
    // there is no permissive fallback.
    let verified = {
        let store = FRAGMENTS.lock().await;
        let r = if store.require_x509()
            || (store.has_did_x509_anchors()
                && kata_security_reference_monitor::did_x509::cose_has_x5chain(cose_sign1))
        {
            store.verify_cose_x509(&fragment, cose_sign1)
        } else {
            store.verify_cose(&fragment, cose_sign1)
        };
        r.map_err(|e| anyhow!("SRM rejected boot-pulled fragment: {e}"))?
    };

    if let Some(module) = &verified.policy_module {
        AGENT_POLICY
            .lock()
            .await
            .apply_fragment_module(
                &format!("fragment:{}:{}", verified.issuer, verified.svn),
                module,
                &verified.includes,
            )
            .map_err(|e| anyhow!("applying boot-pulled fragment module: {e}"))?;
    }

    // FR-1i: persist the SVN high-water marks after commit so a restart cannot reopen a
    // rollback window.
    {
        let mut store = FRAGMENTS.lock().await;
        store.commit(&verified);
        crate::persist_fragment_svn_state(&store.export_svn_state());
    }
    Ok(())
}

/// Pull the raw COSE_Sign1 bytes for a fragment feed from its OCI registry.
///
/// `feed` is an OCI reference (e.g. `contoso.azurecr.io/frag/infra:1`). The manifest is
/// resolved, the COSE layer selected by media type, and its blob downloaded. No
/// verification happens here — the returned bytes are untrusted until SRM-verified.
async fn fetch_fragment(feed: &str) -> Result<Vec<u8>> {
    let reference: Reference = feed
        .parse()
        .with_context(|| format!("invalid OCI reference for feed: {feed}"))?;

    // Registries are HTTPS by default; only fall back to plain HTTP for an explicit
    // localhost/loopback dev registry.
    let protocol = if is_plain_http_registry(&reference) {
        ClientProtocol::Http
    } else {
        ClientProtocol::Https
    };
    let client = Client::new(ClientConfig {
        protocol,
        ..Default::default()
    });

    // Fragments are public artifacts pinned by digest/tag; anonymous pull.
    let auth = RegistryAuth::Anonymous;

    let (manifest, _digest) = client
        .pull_image_manifest(&reference, &auth)
        .await
        .with_context(|| format!("failed to pull manifest for {reference}"))?;

    if let Some(at) = &manifest.artifact_type {
        if at != FRAGMENT_ARTIFACT_TYPE {
            info!(
                sl!(),
                "policy-fragments: unexpected artifactType {at} (want {FRAGMENT_ARTIFACT_TYPE}) for {reference} — continuing"
            );
        }
    }

    let layer = manifest
        .layers
        .iter()
        .find(|l| l.media_type == COSE_LAYER_MEDIA_TYPE)
        .ok_or_else(|| {
            anyhow!(
                "no {COSE_LAYER_MEDIA_TYPE} layer in manifest for {reference} (have: {})",
                manifest
                    .layers
                    .iter()
                    .map(|l| l.media_type.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

    let mut buf: Vec<u8> = Vec::with_capacity(layer.size.max(0) as usize);
    client
        .pull_blob(&reference, layer, &mut buf)
        .await
        .with_context(|| format!("failed to download fragment layer {}", layer.digest))?;

    if buf.is_empty() {
        bail!("downloaded fragment layer is empty for {reference}");
    }
    Ok(buf)
}

/// Only treat an explicit localhost/loopback registry as plain-HTTP; all other registries
/// must use TLS.
fn is_plain_http_registry(reference: &Reference) -> bool {
    let registry = reference.registry();
    registry.starts_with("localhost")
        || registry.starts_with("127.0.0.1")
        || registry.starts_with("[::1]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_only_for_localhost() {
        let local: Reference = "localhost:5001/frag/infra:1".parse().unwrap();
        assert!(is_plain_http_registry(&local));

        let loopback: Reference = "127.0.0.1:5001/frag/infra:1".parse().unwrap();
        assert!(is_plain_http_registry(&loopback));

        let remote: Reference = "contoso.azurecr.io/frag/infra:1".parse().unwrap();
        assert!(!is_plain_http_registry(&remote));
    }
}
