//
// Copyright 2022 The Sigstore Authors.
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

use tracing::info;

use super::client::Client;
use crate::crypto::{
    certificate_pool::CertificatePool, CosignVerificationKey, SignatureDigestAlgorithm,
};
use crate::errors::Result;
use crate::registry::{Certificate, ClientConfig};

/// A builder that generates Client objects.
///
/// ## Rekor integration
///
/// Rekor integration can be enabled by specifying Rekor's public key.
/// This can be provided via the [`ClientBuilder::with_rekor_pub_key`] method.
///
/// > Note well: the [`tuf`](crate::tuf) module provides helper structs and methods
/// > to obtain this data from the official TUF repository of the Sigstore project.
///
/// ## Fulcio integration
///
/// Fulcio integration can be enabled by specifying Fulcio's certificate.
/// This can be provided via the [`ClientBuilder::with_fulcio_cert`] method.
///
/// > Note well: the [`tuf`](crate::tuf) module provides helper structs and methods
/// > to obtain this data from the official TUF repository of the Sigstore project.
///
/// ## Registry caching
///
/// The [`cosign::Client`](crate::cosign::Client) interacts with remote container registries to obtain
/// the data needed to perform Sigstore verification.
///
/// By default, the client will always reach out to the remote registry. However,
/// it's possible to enable an in-memory cache. This behaviour can be enabled via
/// the [`ClientBuilder::enable_registry_caching`] method.
///
/// Each cached entry will automatically expire after 60 seconds.
#[derive(Default)]
pub struct ClientBuilder {
    oci_client_config: ClientConfig,
    rekor_pub_key: Option<String>,
    fulcio_certs: Vec<Certificate>,
    enable_registry_caching: bool,
}

impl ClientBuilder {
    /// Enable caching of data returned from remote OCI registries
    pub fn enable_registry_caching(mut self) -> Self {
        self.enable_registry_caching = true;
        self
    }

    /// Specify the public key used by Rekor.
    ///
    /// The public key can be obtained by using the helper methods under the
    /// [`tuf`](crate::tuf) module.
    ///
    /// `key` is a PEM encoded public key
    ///
    /// When provided, this enables Rekor's integration.
    pub fn with_rekor_pub_key(mut self, key: &str) -> Self {
        self.rekor_pub_key = Some(key.to_string());
        self
    }

    /// Specify the certificate used by Fulcio. This method can be invoked
    /// multiple times to add all the certificates that Fulcio used over the
    /// time.
    ///
    /// `cert` is a PEM encoded certificate
    ///
    /// The certificates can be obtained by using the helper methods under the
    /// [`tuf`](crate::tuf) module.
    ///
    /// When provided, this enables Fulcio's integration.
    pub fn with_fulcio_cert(mut self, cert: &[u8]) -> Self {
        let certificate = Certificate {
            encoding: crate::registry::CertificateEncoding::Pem,
            data: cert.to_owned(),
        };
        self.fulcio_certs.push(certificate);
        self
    }

    /// Specify the certificates used by Fulcio.
    ///
    /// The certificates can be obtained by using the helper methods under the
    /// [`tuf`](crate::tuf) module.
    ///
    /// When provided, this enables Fulcio's integration.
    pub fn with_fulcio_certs(mut self, certs: &[crate::registry::Certificate]) -> Self {
        self.fulcio_certs = certs.to_vec();
        self
    }

    /// Optional - the configuration to be used by the OCI client.
    ///
    /// This can be used when dealing with registries that are not using
    /// TLS termination, or are using self-signed certificates.
    pub fn with_oci_client_config(mut self, config: ClientConfig) -> Self {
        self.oci_client_config = config;
        self
    }

    pub fn build(self) -> Result<Client> {
        let rekor_pub_key = match self.rekor_pub_key {
            None => {
                info!("Rekor public key not provided. Rekor integration disabled");
                None
            }
            Some(data) => Some(CosignVerificationKey::from_pem(
                data.as_bytes(),
                SignatureDigestAlgorithm::default(),
            )?),
        };

        let fulcio_cert_pool = if self.fulcio_certs.is_empty() {
            info!("No Fulcio cert has been provided. Fulcio integration disabled");
            None
        } else {
            let cert_pool = CertificatePool::from_certificates(&self.fulcio_certs)?;
            Some(cert_pool)
        };

        let oci_client =
            oci_distribution::client::Client::new(self.oci_client_config.clone().into());

        let registry_client: Box<dyn crate::registry::ClientCapabilities> =
            if self.enable_registry_caching {
                Box::new(crate::registry::OciCachingClient {
                    registry_client: oci_client,
                })
            } else {
                Box::new(crate::registry::OciClient {
                    registry_client: oci_client,
                })
            };

        Ok(Client {
            registry_client,
            rekor_pub_key,
            fulcio_cert_pool,
        })
    }
}
