// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-1 offline fragment signer / key generator (developer tooling).
//!
//! Reuses [`PolicyFragment::signing_bytes`] so the signature format is guaranteed to match
//! what the guest verifies. Not a production signing tool — for tests, demos, and local
//! development of the signed-policy-fragment feature.
//!
//! Usage:
//!   # generate an Ed25519 keypair (hex); put the public key in fragment-issuers.toml
//!   cargo run --example sign-fragment -- gen-key
//!
//!   # sign a fragment; prints the detached signature (hex)
//!   cargo run --example sign-fragment -- sign \
//!       --issuer issuerA --svn 1 --receipt r1 \
//!       --includes exec \
//!       --module /path/to/fragment.rego \
//!       --key <privkey-hex>
//!
//! The signer prints the signature hex; feed it, the module file, and the other fields to
//! `kata-agent-ctl`'s `LoadPolicyFragment` command.

use ed25519_dalek::{Signer, SigningKey};
use kata_security_reference_monitor::PolicyFragment;
use std::collections::HashMap;

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err("hex string has odd length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// Parse `--flag value` pairs from args. A `--flag` with no following value (end of args
/// or immediately followed by another `--flag`) is recorded as a boolean (value "true").
fn parse_flags(args: &[String]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        if let Some(flag) = args[i].strip_prefix("--") {
            let next_is_value = i + 1 < args.len() && !args[i + 1].starts_with("--");
            if next_is_value {
                m.insert(flag.to_string(), args[i + 1].clone());
                i += 2;
            } else {
                m.insert(flag.to_string(), "true".to_string());
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    m
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 {
        eprintln!("usage: sign-fragment <gen-key|sign> [--flags]");
        std::process::exit(2);
    }

    match argv[1].as_str() {
        "gen-key" => {
            // 32 random bytes from the OS as the Ed25519 secret scalar seed.
            use std::io::Read;
            let mut seed = [0u8; 32];
            std::fs::File::open("/dev/urandom")
                .expect("open /dev/urandom")
                .read_exact(&mut seed)
                .expect("read 32 random bytes");
            let sk = SigningKey::from_bytes(&seed);
            let pk = sk.verifying_key().to_bytes();
            println!("private_key_hex={}", hex_encode(&seed));
            println!("public_key_hex={}", hex_encode(&pk));
        }
        "sign" => {
            let f = parse_flags(&argv[2..]);
            let issuer = f.get("issuer").cloned().unwrap_or_default();
            let svn: u64 = f.get("svn").and_then(|s| s.parse().ok()).unwrap_or(0);
            let receipt = f.get("receipt").cloned();
            let includes: Vec<String> = f
                .get("includes")
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let module = f.get("module").map(|p| {
                String::from_utf8(std::fs::read(p).expect("read module file")).expect("module utf8")
            });
            let key_hex = match f.get("key") {
                Some(k) => k.clone(),
                None => {
                    eprintln!("--key <privkey-hex> is required");
                    std::process::exit(2);
                }
            };
            let seed_vec = hex_decode(&key_hex).expect("decode key hex");
            if seed_vec.len() != 32 {
                eprintln!("key must be 32 bytes ({} hex chars)", 64);
                std::process::exit(2);
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&seed_vec);
            let sk = SigningKey::from_bytes(&seed);

            let fragment = PolicyFragment {
                issuer,
                feed: f.get("feed").cloned().unwrap_or_default(),
                svn,
                grants: vec![],
                policy_module: module,
                includes,
                requires: f
                    .get("requires")
                    .map(|s| {
                        s.split(',')
                            .map(|x| x.trim().to_string())
                            .filter(|x| !x.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                receipt,
                receipt_ledger: f.get("ledger").cloned(),
                signature: vec![],
            };
            let sig = sk.sign(&fragment.signing_bytes());
            println!("signature_hex={}", hex_encode(&sig.to_bytes()));

            // FR-1f: optionally also emit a transparency receipt = a signature over the
            // same statement by a transparency ledger key (--receipt-key <hex>). Tag the
            // originating ledger with --ledger <id> so the trust list can scope/verify it.
            if let Some(rk_hex) = f.get("receipt-key") {
                let rk_vec = hex_decode(rk_hex).expect("decode receipt key hex");
                if rk_vec.len() == 32 {
                    let mut rk = [0u8; 32];
                    rk.copy_from_slice(&rk_vec);
                    let ask = SigningKey::from_bytes(&rk);
                    let rsig = ask.sign(&fragment.signing_bytes());
                    println!("receipt_hex={}", hex_encode(&rsig.to_bytes()));
                    if let Some(ledger) = f.get("ledger") {
                        println!("receipt_ledger={}", ledger);
                    }
                }
            }

            // FR-1h: optionally emit a COSE_Sign1 (CBOR) envelope carrying the statement as
            // payload, signed by the issuer key (EdDSA) — for the COSE load path.
            if f.contains_key("cose") {
                use coset::{iana, CborSerializable, CoseSign1Builder, HeaderBuilder};
                let protected = HeaderBuilder::new().algorithm(iana::Algorithm::EdDSA).build();
                let sign1 = CoseSign1Builder::new()
                    .protected(protected)
                    .payload(fragment.signing_bytes())
                    .create_signature(b"", |tbs| sk.sign(tbs).to_bytes().to_vec())
                    .build();
                println!("cose_sign1_hex={}", hex_encode(&sign1.to_vec().unwrap()));
            }
        }
        other => {
            eprintln!("unknown subcommand: {other}");
            std::process::exit(2);
        }
    }
}
