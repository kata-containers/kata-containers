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

#[cfg(test)]
pub(crate) mod test {
    use crate::errors::{Result, SigstoreError};

    use async_trait::async_trait;
    use oci_distribution::{
        client::ImageData, manifest::OciManifest, secrets::RegistryAuth, Reference,
    };

    #[derive(Default)]
    pub struct MockOciClient {
        pub fetch_manifest_digest_response: Option<anyhow::Result<String>>,
        pub pull_response: Option<anyhow::Result<ImageData>>,
        pub pull_manifest_response: Option<anyhow::Result<(OciManifest, String)>>,
    }

    #[async_trait]
    impl crate::registry::ClientCapabilities for MockOciClient {
        async fn fetch_manifest_digest(
            &mut self,
            image: &Reference,
            _auth: &RegistryAuth,
        ) -> Result<String> {
            let mock_response = self
                .fetch_manifest_digest_response
                .as_ref()
                .ok_or_else(|| SigstoreError::RegistryFetchManifestError {
                    image: image.whole(),
                    error: String::from("No fetch_manifest_digest_response provided!"),
                })?;

            match mock_response {
                Ok(r) => Ok(r.clone()),
                Err(e) => Err(SigstoreError::RegistryFetchManifestError {
                    image: image.whole(),
                    error: e.to_string(),
                }),
            }
        }

        async fn pull(
            &mut self,
            image: &Reference,
            _auth: &RegistryAuth,
            _accepted_media_types: Vec<&str>,
        ) -> Result<ImageData> {
            let mock_response =
                self.pull_response
                    .as_ref()
                    .ok_or_else(|| SigstoreError::RegistryPullError {
                        image: image.whole(),
                        error: String::from("No pull_response provided!"),
                    })?;

            match mock_response {
                Ok(r) => Ok(r.clone()),
                Err(e) => Err(SigstoreError::RegistryPullError {
                    image: image.whole(),
                    error: e.to_string(),
                }),
            }
        }

        async fn pull_manifest(
            &mut self,
            image: &Reference,
            _auth: &RegistryAuth,
        ) -> Result<(OciManifest, String)> {
            let mock_response = self.pull_manifest_response.as_ref().ok_or_else(|| {
                SigstoreError::RegistryPullError {
                    image: image.whole(),
                    error: String::from("No pull_manifest_response provided!"),
                }
            })?;

            match mock_response {
                Ok(r) => Ok(r.clone()),
                Err(e) => Err(SigstoreError::RegistryPullError {
                    image: image.whole(),
                    error: e.to_string(),
                }),
            }
        }
    }
}
