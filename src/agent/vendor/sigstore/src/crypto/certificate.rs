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

use x509_parser::{certificate::X509Certificate, prelude::ASN1Time};

use crate::errors::{Result, SigstoreError};

/// Ensure the given certificate can be trusted for verifying cosign
/// signatures.
///
/// The following checks are performed against the given certificate:
/// * The certificate has the right set of key usages
/// * The certificate cannot be used before the current time
pub(crate) fn is_trusted(certificate: &X509Certificate, integrated_time: i64) -> Result<()> {
    verify_key_usages(certificate)?;
    verify_has_san(certificate)?;
    verify_validity(certificate)?;
    verify_expiration(certificate, integrated_time)?;

    Ok(())
}

fn verify_key_usages(certificate: &X509Certificate) -> Result<()> {
    let key_usage = certificate
        .tbs_certificate
        .key_usage()?
        .ok_or(SigstoreError::CertificateWithoutDigitalSignatureKeyUsage)?;
    if !key_usage.value.digital_signature() {
        return Err(SigstoreError::CertificateWithoutDigitalSignatureKeyUsage);
    }

    let ext_key_usage = certificate
        .tbs_certificate
        .extended_key_usage()?
        .ok_or(SigstoreError::CertificateWithoutCodeSigningKeyUsage)?;
    if !ext_key_usage.value.code_signing {
        return Err(SigstoreError::CertificateWithoutCodeSigningKeyUsage);
    }

    Ok(())
}

fn verify_has_san(certificate: &X509Certificate) -> Result<()> {
    let _subject_alternative_name = certificate
        .tbs_certificate
        .subject_alternative_name()?
        .ok_or(SigstoreError::CertificateWithoutSubjectAlternativeName)?;
    Ok(())
}

fn verify_validity(certificate: &X509Certificate) -> Result<()> {
    // Comment taken from cosign verification code:
    // THIS IS IMPORTANT: WE DO NOT CHECK TIMES HERE
    // THE CERTIFICATE IS TREATED AS TRUSTED FOREVER
    // WE CHECK THAT THE SIGNATURES WERE CREATED DURING THIS WINDOW
    let validity = certificate.validity();
    let now = ASN1Time::now();
    if now < validity.not_before {
        Err(SigstoreError::CertificateValidityError(
            validity.not_before.to_string(),
        ))
    } else {
        Ok(())
    }
}

fn verify_expiration(certificate: &X509Certificate, integrated_time: i64) -> Result<()> {
    let it = ASN1Time::from_timestamp(integrated_time)?;
    let validity = certificate.validity();

    if it < validity.not_before {
        return Err(
            SigstoreError::CertificateExpiredBeforeSignaturesSubmittedToRekor {
                integrated_time: it.to_string(),
                not_before: validity.not_before.to_string(),
            },
        );
    }

    if it > validity.not_after {
        return Err(
            SigstoreError::CertificateIssuedAfterSignaturesSubmittedToRekor {
                integrated_time: it.to_string(),
                not_after: validity.not_after.to_string(),
            },
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::tests::*;

    use chrono::{Duration, Utc};

    #[test]
    fn verify_cert_key_usages_success() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(Some(&ca_data), CertGenerationOptions::default())?;
        let issued_cert_pem = issued_cert.cert.to_pem()?;
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        assert!(verify_key_usages(&cert).is_ok());

        Ok(())
    }

    #[test]
    fn verify_cert_key_usages_failure_because_no_digital_signature() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                digital_signature_key_usage: false,
                ..Default::default()
            },
        )?;
        let issued_cert_pem = issued_cert.cert.to_pem()?;
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        let err = verify_key_usages(&cert).expect_err("Was supposed to return an error");
        let found = match err {
            SigstoreError::CertificateWithoutDigitalSignatureKeyUsage => true,
            _ => false,
        };
        assert!(found, "Didn't get expected error, got {:?} instead", err);

        Ok(())
    }

    #[test]
    fn verify_cert_key_usages_failure_because_no_code_signing() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                code_signing_extended_key_usage: false,
                ..Default::default()
            },
        )?;
        let issued_cert_pem = issued_cert.cert.to_pem()?;
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        let err = verify_key_usages(&cert).expect_err("Was supposed to return an error");
        let found = match err {
            SigstoreError::CertificateWithoutCodeSigningKeyUsage => true,
            _ => false,
        };
        assert!(found, "Didn't get expected error, got {:?} instead", err);

        Ok(())
    }

    #[test]
    fn verify_cert_failure_because_no_san() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                subject_email: None,
                subject_url: None,
                ..Default::default()
            },
        )?;
        let issued_cert_pem = issued_cert.cert.to_pem()?;
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        let error = verify_has_san(&cert).expect_err("Didn't get an error");
        let found = match error {
            SigstoreError::CertificateWithoutSubjectAlternativeName => true,
            _ => false,
        };
        assert!(found, "Didn't get the expected error: {}", error);

        Ok(())
    }

    #[test]
    fn verify_cert_validity_success() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(Some(&ca_data), CertGenerationOptions::default())?;
        let issued_cert_pem = issued_cert.cert.to_pem()?;
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        assert!(verify_validity(&cert).is_ok());

        Ok(())
    }

    #[test]
    fn verify_cert_validity_failure() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                not_before: Utc::now().checked_add_signed(Duration::days(5)).unwrap(),
                not_after: Utc::now().checked_add_signed(Duration::days(6)).unwrap(),
                ..Default::default()
            },
        )?;
        let issued_cert_pem = issued_cert.cert.to_pem()?;
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        let err = verify_validity(&cert).expect_err("Was expecting an error");
        let found = match err {
            SigstoreError::CertificateValidityError(_) => true,
            _ => false,
        };
        assert!(found, "Didn't get expected error, got {:?} instead", err);

        Ok(())
    }

    #[test]
    fn verify_cert_expiration_success() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let integrated_time = Utc::now();

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                not_before: Utc::now().checked_sub_signed(Duration::days(1)).unwrap(),
                not_after: Utc::now().checked_add_signed(Duration::days(1)).unwrap(),
                ..Default::default()
            },
        )?;
        let issued_cert_pem = issued_cert.cert.to_pem()?;
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        assert!(verify_expiration(&cert, integrated_time.timestamp(),).is_ok());

        Ok(())
    }

    #[test]
    fn verify_cert_expiration_failure() -> anyhow::Result<()> {
        let ca_data = generate_certificate(None, CertGenerationOptions::default())?;

        let integrated_time = Utc::now().checked_add_signed(Duration::days(5)).unwrap();

        let issued_cert = generate_certificate(
            Some(&ca_data),
            CertGenerationOptions {
                not_before: Utc::now().checked_sub_signed(Duration::days(1)).unwrap(),
                not_after: Utc::now().checked_add_signed(Duration::days(1)).unwrap(),
                ..Default::default()
            },
        )?;
        let issued_cert_pem = issued_cert.cert.to_pem().unwrap();
        let (_, pem) = x509_parser::pem::parse_x509_pem(&issued_cert_pem)?;
        let (_, cert) = x509_parser::parse_x509_certificate(&pem.contents)?;

        let err = verify_expiration(&cert, integrated_time.timestamp())
            .expect_err("Was expecting an error");
        let found = match err {
            SigstoreError::CertificateIssuedAfterSignaturesSubmittedToRekor {
                integrated_time: _,
                not_after: _,
            } => true,
            _ => false,
        };
        assert!(found, "Didn't get expected error, got {:?} instead", err);

        Ok(())
    }
}
