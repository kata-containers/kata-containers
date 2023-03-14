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

use super::ClientCapabilities;
use crate::errors::{Result, SigstoreError};

use async_trait::async_trait;

/// Internal client for an OCI Registry. This performs actual
/// calls against the remote registry.OciClient
///
/// For testing purposes, use instead the client inside of the
/// `mock_client` module.
pub(crate) struct OciClient {
    pub registry_client: oci_distribution::Client,
}

#[async_trait]
impl ClientCapabilities for OciClient {
    async fn fetch_manifest_digest(
        &mut self,
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
    ) -> Result<String> {
        self.registry_client
            .fetch_manifest_digest(image, auth)
            .await
            .map_err(|e| SigstoreError::RegistryFetchManifestError {
                image: image.whole(),
                error: e.to_string(),
            })
    }

    async fn pull(
        &mut self,
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
        accepted_media_types: Vec<&str>,
    ) -> Result<oci_distribution::client::ImageData> {
        self.registry_client
            .pull(image, auth, accepted_media_types)
            .await
            .map_err(|e| SigstoreError::RegistryPullError {
                image: image.whole(),
                error: e.to_string(),
            })
    }

    async fn pull_manifest(
        &mut self,
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
    ) -> Result<(oci_distribution::manifest::OciManifest, String)> {
        self.registry_client
            .pull_manifest(image, auth)
            .await
            .map_err(|e| SigstoreError::RegistryPullManifestError {
                image: image.whole(),
                error: e.to_string(),
            })
    }
}
