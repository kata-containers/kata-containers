//
// Copyright 2021 The Sigstore Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use olpc_cjson::CanonicalFormatter;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;

use crate::crypto::{CosignVerificationKey, Signature};
use crate::errors::{Result, SigstoreError};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Bundle {
    pub signed_entry_timestamp: String,
    pub payload: Payload,
}

impl Bundle {
    /// Create a new verified `Bundle`
    ///
    /// **Note well:** The bundle will be returned only if it can be verified
    /// using the supplied `rekor_pub_key` public key.
    pub(crate) fn new_verified(raw: &str, rekor_pub_key: &CosignVerificationKey) -> Result<Self> {
        let bundle: Bundle = serde_json::from_str(raw).map_err(|e| {
            SigstoreError::UnexpectedError(format!("Cannot parse bundle |{}|: {:?}", raw, e))
        })?;

        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
        bundle.payload.serialize(&mut ser).map_err(|e| {
            SigstoreError::UnexpectedError(format!(
                "Cannot create canonical JSON representation of bundle: {:?}",
                e
            ))
        })?;

        rekor_pub_key.verify_signature(
            Signature::Base64Encoded(bundle.signed_entry_timestamp.as_bytes()),
            &buf,
        )?;
        Ok(bundle)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
    pub body: String,
    pub integrated_time: i64,
    pub log_index: i64,
    #[serde(rename = "logID")]
    pub log_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::cosign::tests::get_rekor_public_key;
    use crate::crypto::SignatureDigestAlgorithm;

    fn build_correct_bundle() -> String {
        let bundle_json = json!({
          "SignedEntryTimestamp": "MEUCIDx9M+yRpD0O47/Mzm8NAPCbtqy4uiTkLWWexW0bo4jZAiEA1wwueIW8XzJWNkut5y9snYj7UOfbMmUXp7fH3CzJmWg=",
          "Payload": {
            "body": "eyJhcGlWZXJzaW9uIjoiMC4wLjEiLCJraW5kIjoicmVrb3JkIiwic3BlYyI6eyJkYXRhIjp7Imhhc2giOnsiYWxnb3JpdGhtIjoic2hhMjU2IiwidmFsdWUiOiIzYWY0NDE0ZDIwYzllMWNiNzZjY2M3MmFhZThiMjQyMTY2ZGFiZTZhZjUzMWE0YTc5MGRiOGUyZjBlNWVlN2M5In19LCJzaWduYXR1cmUiOnsiY29udGVudCI6Ik1FWUNJUURXV3hQUWEzWEZVc1BieVRZK24rYlp1LzZQd2hnNVd3eVlEUXRFZlFobzl3SWhBUGtLVzdldWI4YjdCWCtZYmJSYWM4VHd3SXJLNUt4dmR0UTZOdW9EK2l2VyIsImZvcm1hdCI6Ing1MDkiLCJwdWJsaWNLZXkiOnsiY29udGVudCI6IkxTMHRMUzFDUlVkSlRpQlFWVUpNU1VNZ1MwVlpMUzB0TFMwS1RVWnJkMFYzV1VoTGIxcEplbW93UTBGUldVbExiMXBKZW1vd1JFRlJZMFJSWjBGRlRFdG9SRGRHTlU5TGVUYzNXalU0TWxrMmFEQjFNVW96UjA1Qkt3cHJkbFZ6YURSbFMzQmtNV3gzYTBSQmVtWkdSSE0zZVZoRlJYaHpSV3RRVUhWcFVVcENaV3hFVkRZNGJqZFFSRWxYUWk5UlJWazNiWEpCUFQwS0xTMHRMUzFGVGtRZ1VGVkNURWxESUV0RldTMHRMUzB0Q2c9PSJ9fX19",
            "integratedTime": 1634714179,
            "logIndex": 783606,
            "logID": "c0d23d6ad406973f9559f3ba2d1ca01f84147d8ffc5b8445c224f98b9591801d"
          }
        });
        serde_json::to_string(&bundle_json).unwrap()
    }

    #[test]
    fn bundle_new_verified_success() {
        let rekor_pub_key = get_rekor_public_key();

        let bundle_json = build_correct_bundle();
        let bundle = Bundle::new_verified(&bundle_json, &rekor_pub_key);

        assert!(bundle.is_ok());
    }

    #[test]
    fn bundle_new_verified_failure() {
        let public_key = r#"-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAENptdY/l3nB0yqkXLBWkZWQwo6+cu
OSWS1X9vPavpiQOoTTGC0xX57OojUadxF1cdQmrsiReWg2Wn4FneJfa8xw==
-----END PUBLIC KEY-----"#;
        let not_rekor_pub_key = CosignVerificationKey::from_pem(
            public_key.as_bytes(),
            SignatureDigestAlgorithm::default(),
        )
        .expect("Cannot create CosignVerificationKey");

        let bundle_json = build_correct_bundle();
        let bundle = Bundle::new_verified(&bundle_json, &not_rekor_pub_key);

        assert!(bundle.is_err());
    }
}
