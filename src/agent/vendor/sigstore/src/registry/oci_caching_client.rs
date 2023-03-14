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
use cached::proc_macro::cached;
use olpc_cjson::CanonicalFormatter;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tracing::{debug, error};

/// Internal client for an OCI Registry. This performs actual
/// calls against the remote registry and caches the results
/// for 60 seconds.
///
/// For testing purposes, use instead the client inside of the
/// `mock_client` module.
pub(crate) struct OciCachingClient {
    pub registry_client: oci_distribution::Client,
}

#[cached(
    time = 60,
    result = true,
    sync_writes = true,
    key = "String",
    convert = r#"{ format!("{}", image) }"#,
    with_cached_flag = true
)]
async fn fetch_manifest_digest_cached(
    client: &mut oci_distribution::Client,
    image: &oci_distribution::Reference,
    auth: &oci_distribution::secrets::RegistryAuth,
) -> Result<cached::Return<String>> {
    client
        .fetch_manifest_digest(image, auth)
        .await
        .map_err(|e| SigstoreError::RegistryFetchManifestError {
            image: image.whole(),
            error: e.to_string(),
        })
        .map(cached::Return::new)
}

/// Internal struct, used to calculate a unique hash of the pull
/// settings. This is required to cache pull results.
#[derive(Serialize, Debug)]
struct PullSettings<'a> {
    image: String,
    auth: super::config::Auth,
    pub accepted_media_types: Vec<&'a str>,
}

impl<'a> PullSettings<'a> {
    fn new(
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
        accepted_media_types: Vec<&'a str>,
    ) -> PullSettings<'a> {
        let image_str = image.whole();
        let auth_sigstore: super::config::Auth = From::from(auth);

        PullSettings {
            image: image_str,
            auth: auth_sigstore,
            accepted_media_types,
        }
    }

    #[allow(clippy::unwrap_used)]
    pub fn image(&self) -> oci_distribution::Reference {
        // we can use `unwrap` here, because this will never fail
        let reference: oci_distribution::Reference = self.image.parse().unwrap();
        reference
    }

    pub fn auth(&self) -> oci_distribution::secrets::RegistryAuth {
        let internal_auth: &super::config::Auth = &self.auth;
        let a: oci_distribution::secrets::RegistryAuth = internal_auth.into();
        a
    }

    // This function returns a hash of the PullSettings struct.
    // The has is computed by doing a canonical JSON representation of
    // the struct.
    //
    // This method cannot error, because its value is used by the `cached`
    // macro, which doesn't allow error handling.
    // Because of that the method will return the '0' value when something goes
    // wrong during the serialization operation. This is very unlikely to happen
    pub fn hash(&self) -> String {
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
        if let Err(e) = self.serialize(&mut ser) {
            error!(err=?e, settings=?self, "Cannot perform canonical serialization");
            return "0".to_string();
        }

        let mut hasher = Sha256::new();
        hasher.update(&buf);
        let result = hasher.finalize();
        result
            .iter()
            .map(|v| format!("{:x}", v))
            .collect::<Vec<String>>()
            .join("")
    }
}

// Pulls an OCI artifact.
// Details about this cache:
//   * the cache is time bound: cached values are purged after 60 seconds
//   * only successful results are cached
#[cached(
    time = 60,
    result = true,
    sync_writes = true,
    key = "String",
    convert = r#"{ settings.hash() }"#,
    with_cached_flag = true
)]
async fn pull_cached(
    client: &mut oci_distribution::Client,
    settings: PullSettings<'_>,
) -> Result<cached::Return<oci_distribution::client::ImageData>> {
    let auth = settings.auth();
    let image = settings.image();

    client
        .pull(&image, &auth, settings.accepted_media_types)
        .await
        .map_err(|e| SigstoreError::RegistryPullError {
            image: image.whole(),
            error: e.to_string(),
        })
        .map(cached::Return::new)
}

/// Internal struct, used to calculate a unique hash of the pull manifest
/// settings. This is required to cache pull manifest results.
#[derive(Serialize, Debug)]
struct PullManifestSettings {
    image: String,
    auth: super::config::Auth,
}

impl PullManifestSettings {
    fn new(
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
    ) -> PullManifestSettings {
        let image_str = image.whole();
        let auth_sigstore: super::config::Auth = From::from(auth);

        PullManifestSettings {
            image: image_str,
            auth: auth_sigstore,
        }
    }

    #[allow(clippy::unwrap_used)]
    pub fn image(&self) -> oci_distribution::Reference {
        // we can use `unwrap` here, because this will never fail
        let reference: oci_distribution::Reference = self.image.parse().unwrap();
        reference
    }

    pub fn auth(&self) -> oci_distribution::secrets::RegistryAuth {
        let internal_auth: &super::config::Auth = &self.auth;
        let a: oci_distribution::secrets::RegistryAuth = internal_auth.into();
        a
    }

    // This function returns a hash of the PullManifestSettings struct.
    // The has is computed by doing a canonical JSON representation of
    // the struct.
    //
    // This method cannot error, because its value is used by the `cached`
    // macro, which doesn't allow error handling.
    // Because of that the method will return the '0' value when something goes
    // wrong during the serialization operation. This is very unlikely to happen
    pub fn hash(&self) -> String {
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
        if let Err(e) = self.serialize(&mut ser) {
            error!(err=?e, settings=?self, "Cannot perform canonical serialization");
            return "0".to_string();
        }

        let mut hasher = Sha256::new();
        hasher.update(&buf);
        let result = hasher.finalize();
        result
            .iter()
            .map(|v| format!("{:x}", v))
            .collect::<Vec<String>>()
            .join("")
    }
}

// Pulls an OCI manifest.
// Details about this cache:
//   * the cache is time bound: cached values are purged after 60 seconds
//   * only successful results are cached
#[cached(
    time = 60,
    result = true,
    sync_writes = true,
    key = "String",
    convert = r#"{ settings.hash() }"#,
    with_cached_flag = true
)]
async fn pull_manifest_cached(
    client: &mut oci_distribution::Client,
    settings: PullManifestSettings,
) -> Result<cached::Return<(oci_distribution::manifest::OciManifest, String)>> {
    let image = settings.image();
    let auth = settings.auth();
    client
        .pull_manifest(&image, &auth)
        .await
        .map_err(|e| SigstoreError::RegistryPullManifestError {
            image: image.whole(),
            error: e.to_string(),
        })
        .map(cached::Return::new)
}

#[async_trait]
impl ClientCapabilities for OciCachingClient {
    async fn fetch_manifest_digest(
        &mut self,
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
    ) -> Result<String> {
        fetch_manifest_digest_cached(&mut self.registry_client, image, auth)
            .await
            .map(|digest| {
                if digest.was_cached {
                    debug!(?image, "Got image digest from cache");
                } else {
                    debug!(?image, "Got image digest by querying remote registry");
                }
                digest.value
            })
    }

    async fn pull(
        &mut self,
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
        accepted_media_types: Vec<&str>,
    ) -> Result<oci_distribution::client::ImageData> {
        let pull_settings = PullSettings::new(image, auth, accepted_media_types);

        pull_cached(&mut self.registry_client, pull_settings)
            .await
            .map(|data| {
                if data.was_cached {
                    debug!(?image, "Got image data from cache");
                } else {
                    debug!(?image, "Got image data by querying remote registry");
                }
                data.value
            })
    }

    async fn pull_manifest(
        &mut self,
        image: &oci_distribution::Reference,
        auth: &oci_distribution::secrets::RegistryAuth,
    ) -> Result<(oci_distribution::manifest::OciManifest, String)> {
        let pull_manifest_settings = PullManifestSettings::new(image, auth);

        pull_manifest_cached(&mut self.registry_client, pull_manifest_settings)
            .await
            .map(|data| {
                if data.was_cached {
                    debug!(?image, "Got image manifest from cache");
                } else {
                    debug!(?image, "Got image manifest by querying remote registry");
                }
                data.value
            })
    }
}
