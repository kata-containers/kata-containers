// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0
//

//! BL-9 — `genpolicy-fragmentgen`: package an already-signed policy-fragment COSE_Sign1
//! envelope as an OCI artifact, push it to a registry, and emit the `policy_fragments[]`
//! settings entry a measured base policy needs to declare it.
//!
//! This is the *distribution* half of the fragment pipeline. **Signing is unchanged**: use
//! the existing SRM signer
//! (`cargo run --example sign-fragment -p kata-security-reference-monitor -- sign … cose`
//! or `… --x509-key/--x509-chain`) to produce the `.cose` envelope, then this tool only
//! packages/pushes it. The tool never re-implements the COSE/x5chain crypto — it reuses the
//! guest's own `PolicyFragment::from_cose_envelope` parser to derive the
//! `issuer`/`feed`/`svn` the envelope commits to, guaranteeing the emitted settings entry
//! matches exactly what the guest will verify.
//!
//! OCI artifact contract (must match the guest boot-pull fetcher in
//! `src/agent/src/policy_fragments.rs`):
//!   - artifactType: `application/x-ms-ccepolicy-frag`
//!   - COSE layer mediaType: `application/cose-x509+rego`
//!   - empty config mediaType: `application/vnd.oci.empty.v1+json`

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use kata_security_reference_monitor::PolicyFragment;
use oci_client::client::{ClientConfig, ClientProtocol, Config, ImageLayer};
use oci_client::manifest::OciImageManifest;
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};

/// OCI artifactType for a kata policy fragment (matches the guest fetcher).
const FRAGMENT_ARTIFACT_TYPE: &str = "application/x-ms-ccepolicy-frag";
/// Media type of the COSE_Sign1 fragment layer (matches the guest fetcher).
const COSE_LAYER_MEDIA_TYPE: &str = "application/cose-x509+rego";
/// Empty-config media type for an OCI artifact manifest.
const EMPTY_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.empty.v1+json";

#[derive(Parser, Debug)]
#[command(
    name = "genpolicy-fragmentgen",
    about = "Package a signed policy-fragment COSE_Sign1 envelope as an OCI artifact and emit its base-policy settings entry",
    version
)]
struct Cli {
    /// Path to the signed COSE_Sign1 fragment envelope (produced by the SRM `sign-fragment`
    /// example). Its payload must be a `kata-policy-fragment/v3` statement.
    #[arg(long)]
    cose: PathBuf,

    /// Push the artifact to this OCI reference (e.g. `contoso.azurecr.io/frag/infra:1`).
    /// When omitted, the tool only emits the settings entry (offline).
    #[arg(long)]
    push: Option<String>,

    /// Allow plain-HTTP push to a localhost/loopback dev registry only.
    #[arg(long)]
    plain_http: bool,
}

fn read_hex_or_binary(bytes: Vec<u8>) -> Vec<u8> {
    // Accept either a raw binary .cose or a hex-encoded envelope (as printed by the signer's
    // `cose_sign1_hex=` line). Detect hex by an all-hex, even-length ASCII body.
    if let Ok(text) = std::str::from_utf8(&bytes) {
        let t = text.trim();
        if !t.is_empty()
            && t.len() % 2 == 0
            && t.bytes().all(|b| b.is_ascii_hexdigit())
        {
            if let Ok(decoded) = (0..t.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&t[i..i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
            {
                return decoded;
            }
        }
    }
    bytes
}

async fn push_artifact(reference: &str, plain_http: bool, cose: &[u8]) -> Result<()> {
    let reference: Reference = reference
        .parse()
        .with_context(|| format!("invalid OCI reference {reference:?}"))?;

    let registry = reference.registry();
    let is_local = registry.starts_with("localhost")
        || registry.starts_with("127.0.0.1")
        || registry.starts_with("[::1]");
    if plain_http && !is_local {
        bail!("--plain-http is only allowed for localhost/loopback registries (got {registry})");
    }
    let protocol = if plain_http && is_local {
        ClientProtocol::Http
    } else {
        ClientProtocol::Https
    };

    let client = Client::new(ClientConfig {
        protocol,
        ..Default::default()
    });

    let layer = ImageLayer::new(cose.to_vec(), COSE_LAYER_MEDIA_TYPE.to_string(), None);
    let config = Config::new(b"{}".to_vec(), EMPTY_CONFIG_MEDIA_TYPE.to_string(), None);

    let mut manifest = OciImageManifest::build(std::slice::from_ref(&layer), &config, None);
    manifest.artifact_type = Some(FRAGMENT_ARTIFACT_TYPE.to_string());
    let mut annotations = BTreeMap::new();
    annotations.insert(
        "org.opencontainers.image.title".to_string(),
        "kata-policy-fragment".to_string(),
    );
    manifest.annotations = Some(annotations);

    client
        .push(
            &reference,
            std::slice::from_ref(&layer),
            config,
            &RegistryAuth::Anonymous,
            Some(manifest),
        )
        .await
        .with_context(|| format!("push fragment artifact to {reference}"))?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let raw = std::fs::read(&cli.cose)
        .with_context(|| format!("read COSE envelope {:?}", cli.cose))?;
    let cose = read_hex_or_binary(raw);

    // Reuse the guest's own parser so the emitted settings entry is exactly what the guest
    // will verify — no divergent re-implementation.
    let fragment = PolicyFragment::from_cose_envelope(&cose)
        .ok_or_else(|| anyhow!("envelope payload is not a kata-policy-fragment/v3 statement"))?;

    println!("issuer: {}", fragment.issuer);
    println!("feed:   {}", fragment.feed);
    println!("svn:    {}", fragment.svn);
    println!();
    println!("genpolicy base-policy data.agent_policy.policy_fragments[] entry:");
    println!("  {{");
    println!("    \"issuer\": \"{}\",", fragment.issuer);
    println!("    \"feed\": \"{}\",", fragment.feed);
    println!("    \"minimum_svn\": {}", fragment.svn);
    println!("  }}");

    if let Some(reference) = &cli.push {
        push_artifact(reference, cli.plain_http, &cose).await?;
        println!();
        println!("Pushed {}-byte fragment artifact to {reference}", cose.len());
    }

    Ok(())
}
