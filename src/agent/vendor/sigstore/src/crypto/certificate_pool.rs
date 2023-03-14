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

use crate::{
    errors::{Result, SigstoreError},
    registry::Certificate,
};

// The untrusted intermediate CA certificate, used for chain building
// TODO: Remove once this is bundled in TUF metadata.
const FULCIO_INTERMEDIATE_V1: &str = "-----BEGIN CERTIFICATE-----
MIICGjCCAaGgAwIBAgIUALnViVfnU0brJasmRkHrn/UnfaQwCgYIKoZIzj0EAwMw
KjEVMBMGA1UEChMMc2lnc3RvcmUuZGV2MREwDwYDVQQDEwhzaWdzdG9yZTAeFw0y
MjA0MTMyMDA2MTVaFw0zMTEwMDUxMzU2NThaMDcxFTATBgNVBAoTDHNpZ3N0b3Jl
LmRldjEeMBwGA1UEAxMVc2lnc3RvcmUtaW50ZXJtZWRpYXRlMHYwEAYHKoZIzj0C
AQYFK4EEACIDYgAE8RVS/ysH+NOvuDZyPIZtilgUF9NlarYpAd9HP1vBBH1U5CV7
7LSS7s0ZiH4nE7Hv7ptS6LvvR/STk798LVgMzLlJ4HeIfF3tHSaexLcYpSASr1kS
0N/RgBJz/9jWCiXno3sweTAOBgNVHQ8BAf8EBAMCAQYwEwYDVR0lBAwwCgYIKwYB
BQUHAwMwEgYDVR0TAQH/BAgwBgEB/wIBADAdBgNVHQ4EFgQU39Ppz1YkEZb5qNjp
KFWixi4YZD8wHwYDVR0jBBgwFoAUWMAeX5FFpWapesyQoZMi0CrFxfowCgYIKoZI
zj0EAwMDZwAwZAIwPCsQK4DYiZYDPIaDi5HFKnfxXx6ASSVmERfsynYBiX2X6SJR
nZU84/9DZdnFvvxmAjBOt6QpBlc4J/0DxvkTCqpclvziL6BCCPnjdlIB3Pu3BxsP
mygUY7Ii2zbdCdliiow=
-----END CERTIFICATE-----";

/// A collection of trusted root certificates
#[derive(Default, Debug)]
pub(crate) struct CertificatePool {
    trusted_roots: Vec<picky::x509::Cert>,
    intermediates: Vec<picky::x509::Cert>,
}

impl CertificatePool {
    /// Build a `CertificatePool` instance using the provided list of [`Certificate`]
    pub(crate) fn from_certificates(certs: &[Certificate]) -> Result<Self> {
        let mut trusted_roots = vec![];
        let mut intermediates = vec![];

        for c in certs {
            let pc = match c.encoding {
                crate::registry::CertificateEncoding::Pem => {
                    let pem_str = String::from_utf8(c.data.clone()).map_err(|_| {
                        SigstoreError::UnexpectedError("certificate is not PEM encoded".to_string())
                    })?;
                    picky::x509::Cert::from_pem_str(&pem_str)
                }
                crate::registry::CertificateEncoding::Der => picky::x509::Cert::from_der(&c.data),
            }?;

            match pc.ty() {
                picky::x509::certificate::CertType::Root => {
                    trusted_roots.push(pc);
                }
                picky::x509::certificate::CertType::Intermediate => {
                    intermediates.push(pc);
                }
                _ => {
                    return Err(SigstoreError::CertificatePoolError(
                        "Cannot add a certificate that is no root or intermediate".to_string(),
                    ));
                }
            }
        }

        // TODO: Remove once FULCIO_INTERMEDIATE_V1 is bundled in TUF metadata.
        if intermediates.is_empty() {
            intermediates.push(picky::x509::Cert::from_pem_str(FULCIO_INTERMEDIATE_V1)?);
        }

        Ok(CertificatePool {
            trusted_roots,
            intermediates,
        })
    }

    /// Ensures the given certificate has been issued by one of the trusted root certificates
    /// An `Err` is returned when the verification fails.
    ///
    /// **Note well:** certificates issued by Fulciuo are, by design, valid only
    /// for a really limited amount of time.
    /// Because of that the validity checks performed by this method are more
    /// relaxed. The validity checks are done inside of
    /// [`crate::crypto::verify_validity`] and [`crate::crypto::verify_expiration`].
    pub(crate) fn verify(&self, cert_pem: &[u8]) -> Result<()> {
        let cert_pem_str = String::from_utf8(cert_pem.to_vec()).map_err(|_| {
            SigstoreError::UnexpectedError("Cannot convert cert back to string".to_string())
        })?;
        let cert = picky::x509::Cert::from_pem_str(&cert_pem_str)?;

        let verified = self
            .create_chains_for_all_certificates()
            .iter()
            .any(|chain| {
                cert.verifier()
                    .chain(chain.iter().copied())
                    .exact_date(&cert.valid_not_before())
                    .verify()
                    .is_ok()
            });

        if verified {
            Ok(())
        } else {
            Err(SigstoreError::CertificateValidityError(
                "Not issued by a trusted root".to_string(),
            ))
        }
    }

    fn create_chains_for_all_certificates(&self) -> Vec<Vec<&picky::x509::Cert>> {
        let mut chains: Vec<Vec<&picky::x509::Cert>> = vec![];
        self.trusted_roots.iter().for_each(|trusted_root| {
            chains.push([trusted_root].to_vec());
        });
        self.intermediates.iter().for_each(|intermediate| {
            for root in self.trusted_roots.iter() {
                if root.is_parent_of(intermediate).is_ok() {
                    chains.push([intermediate, root].to_vec());
                }
            }
        });

        chains
    }
}
