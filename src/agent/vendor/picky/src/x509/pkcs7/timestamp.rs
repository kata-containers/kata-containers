use crate::hash::HashAlgorithm;
use crate::x509::certificate::CertError;
use crate::x509::pkcs7::{Pkcs7, Pkcs7Error};
use crate::x509::utils::{from_der, to_der};
use picky_asn1::wrapper::ExplicitContextTag0;

use picky_asn1_x509::oids;
use picky_asn1_x509::pkcs7::content_info::{ContentValue, EncapsulatedContentInfo};
use picky_asn1_x509::pkcs7::signed_data::SignedData;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TimestampError {
    #[error(transparent)]
    Asn1DerError(#[from] CertError),
    #[error(transparent)]
    Pkcs7Error(#[from] Pkcs7Error),
    #[error("Timestamp token is empty")]
    TimestampTokenEmpty,
    #[cfg(feature = "http_timestamp")]
    #[error("Failed to decode base64 response")]
    Base64DecodeError,
    #[cfg(feature = "http_timestamp")]
    #[error("Remote Authenticode TSA server responded with `{0}` status code")]
    BadResponse(reqwest::StatusCode),
    #[cfg(feature = "http_timestamp")]
    #[error("Remote Authenticode TSA server response error: {0}")]
    RemoteServerResponseError(reqwest::Error),
    #[cfg(feature = "http_timestamp")]
    #[error("Badly formatted URL")]
    BadUrl,
}

pub trait Timestamper: Sized {
    fn timestamp(&self, digest: Vec<u8>, hash_algo: HashAlgorithm) -> Result<Pkcs7, TimestampError>; // hash_algo is used in RFC3161
    fn modify_signed_data(&self, token: Pkcs7, signed_data: &mut SignedData);
}

#[cfg(feature = "http_timestamp")]
pub mod http_timestamp {
    use super::*;
    use picky_asn1_x509::pkcs7::signer_info::{UnsignedAttribute, UnsignedAttributeValue};
    use reqwest::blocking::Client;
    use reqwest::header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE};
    use reqwest::{Method, StatusCode, Url};

    #[derive(Clone, Debug, PartialEq)]
    pub struct AuthenticodeTimestamper {
        url: Url,
    }

    impl AuthenticodeTimestamper {
        pub fn new<U: AsRef<str>>(url: U) -> Result<AuthenticodeTimestamper, TimestampError> {
            let url = Url::parse(url.as_ref()).map_err(|_| TimestampError::BadUrl)?;
            Ok(Self { url })
        }
    }

    impl Timestamper for AuthenticodeTimestamper {
        fn timestamp(&self, digest: Vec<u8>, _: HashAlgorithm) -> Result<Pkcs7, TimestampError> {
            let timestamp_request = TimestampRequest::new(digest);

            let client = Client::new();
            let content = base64::encode(timestamp_request.to_der()?);

            let request = client
                .request(Method::POST, self.url.clone())
                .header(CACHE_CONTROL, "no-cache")
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(CONTENT_LENGTH, content.len())
                .body(content)
                .build()
                .expect("RequestBuilder should not panic");

            let response = client
                .execute(request)
                .map_err(TimestampError::RemoteServerResponseError)?;

            if response.status() != StatusCode::OK {
                return Err(TimestampError::BadResponse(response.status()));
            }

            let mut body = response
                .bytes()
                .map_err(TimestampError::RemoteServerResponseError)?
                .to_vec();

            body.retain(|&x| x != b'\n' && x != b'\r' && x != b'\0'); // Removing CRLF entries

            let der = base64::decode(body).map_err(|_| TimestampError::Base64DecodeError)?;
            let token = Pkcs7::from_der(&der).map_err(TimestampError::Pkcs7Error)?;

            Ok(token)
        }

        fn modify_signed_data(&self, token: Pkcs7, signed_data: &mut SignedData) {
            let SignedData {
                certificates,
                signers_infos,
                ..
            } = token.0.signed_data.0;

            let singer_info = signers_infos
                .0
                .first()
                .expect("Exactly one SignedInfo should be present");

            let unsigned_attribute = UnsignedAttribute {
                ty: oids::counter_sign().into(),
                value: UnsignedAttributeValue::CounterSign(vec![singer_info.clone()].into()),
            };

            let signer_info = signed_data
                .signers_infos
                .0
                 .0
                .first_mut()
                .expect("Exactly one SignedInfo should be present");

            signer_info.unsigned_attrs.0 .0.push(unsigned_attribute);

            signed_data.certificates.0 .0.extend(certificates.0 .0);
        }
    }
}

const ELEMENT_NAME: &str = "TimestampRequest";

#[derive(Clone, Debug, PartialEq)]
pub struct TimestampRequest(picky_asn1_x509::timestamp::TimestampRequest);

impl TimestampRequest {
    pub fn new(digest: Vec<u8>) -> TimestampRequest {
        Self(picky_asn1_x509::timestamp::TimestampRequest {
            countersignature_type: oids::timestamp_request().into(),
            content: EncapsulatedContentInfo {
                content_type: oids::pkcs7().into(),
                content: Some(ContentValue::Data(digest.into()).into()),
            },
        })
    }

    pub fn content(&self) -> &EncapsulatedContentInfo {
        &self.0.content
    }

    pub fn into_content(self) -> EncapsulatedContentInfo {
        self.0.content
    }

    pub fn digest(&self) -> &[u8] {
        if let ExplicitContextTag0(ContentValue::Data(data)) = self.0.content.content.as_ref().unwrap() {
            &data.0
        } else {
            unreachable!()
        }
    }

    pub fn from_der<V: ?Sized + AsRef<[u8]>>(data: &V) -> Result<Self, CertError> {
        from_der(data, ELEMENT_NAME).map(Self)
    }

    pub fn to_der(&self) -> Result<Vec<u8>, CertError> {
        to_der(&self.0, ELEMENT_NAME)
    }
}

impl From<TimestampRequest> for picky_asn1_x509::timestamp::TimestampRequest {
    fn from(tr: TimestampRequest) -> picky_asn1_x509::timestamp::TimestampRequest {
        tr.0
    }
}

impl From<picky_asn1_x509::timestamp::TimestampRequest> for TimestampRequest {
    fn from(tr: picky_asn1_x509::timestamp::TimestampRequest) -> TimestampRequest {
        Self(tr)
    }
}
