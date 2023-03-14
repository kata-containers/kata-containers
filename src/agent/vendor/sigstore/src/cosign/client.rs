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

use async_trait::async_trait;

use super::{
    constants::SIGSTORE_OCI_MEDIA_TYPE,
    signature_layers::{build_signature_layers, SignatureLayer},
    CosignCapabilities,
};
use crate::crypto::CosignVerificationKey;
use crate::registry::Auth;
use crate::{
    crypto::certificate_pool::CertificatePool,
    errors::{Result, SigstoreError},
};
use tracing::debug;

/// Cosign Client
///
/// Instances of `Client` can be built via [`sigstore::cosign::ClientBuilder`](crate::cosign::ClientBuilder).
pub struct Client {
    pub(crate) registry_client: Box<dyn crate::registry::ClientCapabilities>,
    pub(crate) rekor_pub_key: Option<CosignVerificationKey>,
    pub(crate) fulcio_cert_pool: Option<CertificatePool>,
}

#[async_trait]
impl CosignCapabilities for Client {
    async fn triangulate(&mut self, image: &str, auth: &Auth) -> Result<(String, String)> {
        let image_reference: oci_distribution::Reference =
            image
                .parse()
                .map_err(|_| SigstoreError::OciReferenceNotValidError {
                    reference: image.to_string(),
                })?;

        let manifest_digest = self
            .registry_client
            .fetch_manifest_digest(&image_reference, &auth.into())
            .await?;

        let sign = format!(
            "{}/{}:{}.sig",
            image_reference.registry(),
            image_reference.repository(),
            manifest_digest.replace(':', "-")
        );
        let reference = sign
            .parse()
            .map_err(|_| SigstoreError::OciReferenceNotValidError {
                reference: image.to_string(),
            })?;

        Ok((reference, manifest_digest))
    }

    async fn trusted_signature_layers(
        &mut self,
        auth: &Auth,
        source_image_digest: &str,
        cosign_image: &str,
    ) -> Result<Vec<SignatureLayer>> {
        let (manifest, layers) = self.fetch_manifest_and_layers(auth, cosign_image).await?;
        let image_manifest = match manifest {
            oci_distribution::manifest::OciManifest::Image(im) => im,
            oci_distribution::manifest::OciManifest::ImageIndex(_) => {
                return Err(SigstoreError::RegistryPullManifestError {
                    image: cosign_image.to_string(),
                    error: "Found a OciImageIndex instead of a OciImageManifest".to_string(),
                });
            }
        };

        let sl = build_signature_layers(
            &image_manifest,
            source_image_digest,
            &layers,
            self.rekor_pub_key.as_ref(),
            self.fulcio_cert_pool.as_ref(),
        )?;

        debug!(signature_layers=?sl, ?cosign_image, "trusted signature layers");
        Ok(sl)
    }
}

impl Client {
    /// Internal helper method used to fetch data from an OCI registry
    async fn fetch_manifest_and_layers(
        &mut self,
        auth: &Auth,
        cosign_image: &str,
    ) -> Result<(
        oci_distribution::manifest::OciManifest,
        Vec<oci_distribution::client::ImageLayer>,
    )> {
        let cosign_image_reference: oci_distribution::Reference =
            cosign_image
                .parse()
                .map_err(|_| SigstoreError::OciReferenceNotValidError {
                    reference: cosign_image.to_string(),
                })?;

        let oci_auth: oci_distribution::secrets::RegistryAuth = auth.into();

        let (manifest, _) = self
            .registry_client
            .pull_manifest(&cosign_image_reference, &oci_auth)
            .await?;
        let image_data = self
            .registry_client
            .pull(
                &cosign_image_reference,
                &oci_auth,
                vec![SIGSTORE_OCI_MEDIA_TYPE],
            )
            .await?;

        Ok((manifest, image_data.layers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosign::tests::{get_fulcio_cert_pool, REKOR_PUB_KEY};
    use crate::{crypto::SignatureDigestAlgorithm, mock_client::test::MockOciClient};

    fn build_test_client(mock_client: MockOciClient) -> Client {
        let rekor_pub_key = CosignVerificationKey::from_pem(
            REKOR_PUB_KEY.as_bytes(),
            SignatureDigestAlgorithm::default(),
        )
        .expect("Cannot create CosignVerificationKey");

        Client {
            registry_client: Box::new(mock_client),
            rekor_pub_key: Some(rekor_pub_key),
            fulcio_cert_pool: Some(get_fulcio_cert_pool()),
        }
    }

    #[tokio::test]
    async fn triangulate_sigstore_object() {
        let image = "docker.io/busybox:latest";
        let image_digest =
            String::from("sha256:f3cfc9d0dbf931d3db4685ec659b7ac68e2a578219da4aae65427886e649b06b");
        let expected_image = "docker.io/library/busybox:sha256-f3cfc9d0dbf931d3db4685ec659b7ac68e2a578219da4aae65427886e649b06b.sig".parse().unwrap();
        let mock_client = MockOciClient {
            fetch_manifest_digest_response: Some(Ok(image_digest.clone())),
            pull_response: None,
            pull_manifest_response: None,
        };
        let mut cosign_client = build_test_client(mock_client);

        let reference = cosign_client
            .triangulate(image, &crate::registry::Auth::Anonymous)
            .await;

        assert!(reference.is_ok());
        assert_eq!(reference.unwrap(), (expected_image, image_digest));
    }
}
