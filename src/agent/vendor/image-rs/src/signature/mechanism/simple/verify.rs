// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, bail, Result};
use openpgp::parse::Parse;
use openpgp::PacketPile;
use sequoia_openpgp as openpgp;

use crate::signature::payload::simple_signing::SigPayload;

const GPG_KEY_ID_BYTES_LENGTH: usize = 20;
const GPG_KEY_ID_SUFFIX_BYTES_LENGTH_IN_SIG: usize = 8;

// SigKeyIDs is a util helper struct, used to compare
// whether the keyID of the key which verified the signature
// is consistant with the keyID recorded in the signature itself.
#[derive(Default)]
struct SigKeyIDs {
    pub trusted_key_id: Vec<u8>,
    pub sig_info_key_id: Vec<u8>,
}

impl SigKeyIDs {
    pub fn validate(&self) -> Result<()> {
        if self.trusted_key_id.len() != GPG_KEY_ID_BYTES_LENGTH {
            bail!("Wrong GPG key ID length in pubkey ring");
        }
        if self.sig_info_key_id.len() != GPG_KEY_ID_SUFFIX_BYTES_LENGTH_IN_SIG {
            bail!("Wrong GPG key ID length in signature payload");
        }

        if self.sig_info_key_id
            == self.trusted_key_id
                [(GPG_KEY_ID_BYTES_LENGTH - GPG_KEY_ID_SUFFIX_BYTES_LENGTH_IN_SIG)..]
                .to_vec()
        {
            Ok(())
        } else {
            Err(
                anyhow!(
                    "Key ID not matched. trusted key id is: {:X?}, but key id in signature info is: {:X?}", 
                    &self.trusted_key_id,
                    &self.sig_info_key_id
                )
            )
        }
    }
}

// Verifies the input signature, and verifies its principal components match expected
// values, both as specified by rules, and returns the signature payload.
pub fn verify_sig_and_extract_payload(pubkey_ring: Vec<u8>, sig: Vec<u8>) -> Result<SigPayload> {
    // Parse the gpg pubkey ring.
    let keyring_packet = PacketPile::from_bytes(&pubkey_ring)?;
    let keyring_iter = keyring_packet.descendants();
    // Parse the signature cliam file into sequoia-opengpg PacketPile format.
    let mut sig_packet = PacketPile::from_bytes(&sig)?;

    let mut validate_key_id = SigKeyIDs::default();

    // Dump the keyID which recorded in the signature itself from the OnePassSig of the sig claim file.
    // OnePassSig: https://docs.rs/sequoia-openpgp/1.7.0/sequoia_openpgp/packet/enum.OnePassSig.html
    //
    // sig_packet is a sequoia-opengpg PacketPile, `path_ref()` and `path_ref_mut()`
    // returns a reference to the packet at the location described by
    // `pathspec`.
    //
    // `pathspec` is a slice of the form `[0, 1, 2]`.  Each element
    // is the index of packet in a container.  Thus, the previous
    // path specification means: return the third child of the second
    // child of the first top-level packet.  In other words, the
    // starred packet in the following tree:
    //
    // ```text
    //         PacketPile
    //        /     |     \
    //       0      1      2  ...
    //     /   \
    //    /     \
    //  0         1  ...
    //        /   |   \  ...
    //       0    1    2
    //                 *
    // ```
    //
    // According to sequoia-opengpg docs, the path ref of OnePassSig is [0, 0],
    // The path ref of Signature is [0, 2].
    if let Some(openpgp::Packet::OnePassSig(ref sig_info)) = sig_packet.path_ref(&[0, 0]) {
        validate_key_id.sig_info_key_id = sig_info.issuer().as_bytes().to_vec();
    }

    // Try to use each pubkey in the pubkey ring to verify the signature.
    // If the signature is verified,
    // verify that the keyID of the pubkey is consistent with the keyID recorded in the signature.
    for packet in keyring_iter {
        if let openpgp::Packet::PublicKey(pubkey) = packet {
            if let Some(openpgp::Packet::Signature(ref mut signature)) =
                sig_packet.path_ref_mut(&[0, 2])
            {
                if signature.verify(pubkey).is_ok() {
                    validate_key_id.trusted_key_id = pubkey.fingerprint().as_bytes().to_vec();
                    // If the cryptography verification passes, but the key IDs are inconsistent,
                    // the verification failure is returned directly.
                    validate_key_id.validate()?;
                }
            }
        }
    }

    if validate_key_id.trusted_key_id.is_empty() {
        bail!("signature verify failed! There is no pubkey can verify the signature!");
    }

    // Dump the signature payload.
    if let Some(openpgp::Packet::Literal(ref literal)) = sig_packet.path_ref(&[0, 1]) {
        let body_message = String::from_utf8(literal.body().to_vec())?;
        let sig_payload = serde_json::from_str::<SigPayload>(&body_message)?;
        Ok(sig_payload)
    } else {
        Err(anyhow!("Signature format error: no literal field in it!"))
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crate::signature::policy::ref_match::PolicyReqMatchType;

    use super::*;
    use oci_distribution::Reference;
    use serde_json::json;

    const SIG_PAYLOAD_JSON: &str = r#"{
        "critical": {
            "identity": {
                "docker-reference": "quay.io/ali_os_security/alpine:latest"
            },
              "image": {
                "docker-manifest-digest": "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a"
            },
              "type": "atomic container signature"
        },
        "optional": {
            "creator": "atomic 2.0.0",
            "timestamp": 1634533638
        }
    }"#;

    fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
        if s.len() % 2 == 0 {
            (0..s.len())
                .step_by(2)
                .map(|i| {
                    s.get(i..i + 2)
                        .and_then(|sub| u8::from_str_radix(sub, 16).ok())
                })
                .collect()
        } else {
            None
        }
    }

    #[test]
    fn test_validate_key_id() {
        let tests_unexpect = &[SigKeyIDs {
            trusted_key_id: hex_to_bytes("AEAF51ED16475A565335439B77E5C166C87B344B").unwrap(),
            sig_info_key_id: hex_to_bytes("7EFD4C926F9311E2").unwrap(),
        }];

        let tests_expect = &[SigKeyIDs {
            trusted_key_id: hex_to_bytes("2183156095E072685518F8A97EFD4C926F9311E2").unwrap(),
            sig_info_key_id: hex_to_bytes("7EFD4C926F9311E2").unwrap(),
        }];

        for case in tests_unexpect.iter() {
            assert!(case.validate().is_err());
        }

        for case in tests_expect.iter() {
            assert!(case.validate().is_ok());
        }
    }

    #[test]
    fn test_sigpayload_validate() {
        let sig_payload = serde_json::from_str::<SigPayload>(SIG_PAYLOAD_JSON).unwrap();
        let match_policy_json = r#"{
            "type": "matchExact"
        }"#;
        let match_reference_policy: PolicyReqMatchType =
            serde_json::from_str(match_policy_json).unwrap();

        #[derive(Debug)]
        struct TestData<'a> {
            digest: &'a str,
            reference: Reference,
        }

        let tests_expect = &[TestData {
            digest: "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a",
            reference: Reference::try_from("quay.io/ali_os_security/alpine:latest").unwrap(),
        }];

        let tests_unexpect = &[TestData {
            digest: "sha256:abcdeef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a",
            reference: Reference::try_from("quay.io/example_user/alpine:tag1").unwrap(),
        }];

        for case in tests_expect.iter() {
            assert!(sig_payload
                .validate_signed_docker_manifest_digest(case.digest)
                .is_ok());

            assert!(sig_payload
                .validate_signed_docker_reference(&case.reference, &match_reference_policy)
                .is_ok());
        }

        for case in tests_unexpect.iter() {
            assert!(sig_payload
                .validate_signed_docker_manifest_digest(case.digest)
                .is_err());

            assert!(sig_payload
                .validate_signed_docker_reference(&case.reference, &match_reference_policy)
                .is_err());
        }
    }

    #[test]
    fn test_verify_sig_and_extract_payload() {
        let sig_payload_parsed = json!({
            "critical": {
                "identity": {
                    "docker-reference": "quay.io/ali_os_security/alpine:latest"
                },
                  "image": {
                    "docker-manifest-digest": "sha256:69704ef328d05a9f806b6b8502915e6a0a4faa4d72018dc42343f511490daf8a"
                },
                  "type": "atomic container signature"
            },
            "optional": {
                "creator": "atomic 2.0.0",
                "timestamp": 1634533638
            }
        });

        let keyring_bytes_case_1 = ::std::fs::read("./test_data/signature/pubring.gpg").unwrap();
        let sig_bytes_case_1 =
            ::std::fs::read("./test_data/signature/signatures/signature-1").unwrap();

        let sig_payload_verified =
            verify_sig_and_extract_payload(keyring_bytes_case_1, sig_bytes_case_1).unwrap();

        let sig_payload_verified = serde_json::to_value(&sig_payload_verified).unwrap();

        assert_eq!(sig_payload_parsed, sig_payload_verified);
    }
}
