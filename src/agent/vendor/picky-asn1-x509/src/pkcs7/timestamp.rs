use crate::pkcs7::content_info::EncapsulatedContentInfo;
use picky_asn1::wrapper::ObjectIdentifierAsn1;
use serde::{Deserialize, Serialize};

/// ``` not_rust
/// [Time Stamping Authenticode Signatures](https://docs.microsoft.com/en-us/windows/win32/seccrypto/time-stamping-authenticode-signatures)
/// TimeStampRequest ::= SEQUENCE {
///     countersignatureType OBJECT IDENTIFIER,
///     attributes Attributes OPTIONAL,
///     content  ContentInfo
/// }
/// ```

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TimestampRequest {
    pub countersignature_type: ObjectIdentifierAsn1,
    // MSDN: No attributes are currently included in the time stamp request.
    // attributes
    pub content: EncapsulatedContentInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_timestamp_request() {
        let decoded = base64::decode(
            "MIIBIwYKKwYBBAGCNwMCATCCARMGCSqGSIb3DQEHAaCCAQQEggEAQrCUqwV+9+Fl\
                  bgfJUwua28rmolLOZf/d3alzHUu6P1iV/9crZeu9ShrFdmQ4ZVWTpcR7bcVGPVsW\
                  QdUgx2n1mJCPied8YPjzC0wfJPvzZzOz9X919EAFRUi4VPs5qEsHJV57YP5mJ2UC\
                  XqXKSR9HhRO/06TSGz7hkFh+vpsKYVvIZpDXNJPRUilgEDQXjHCdMZyOzPb9wO8k\
                  cFHbUYZT9lVp9p8Wg+P56RdOANgtS5GKfku2BTsgbwxh5k8GzMnsiaf++O8LgMaE\
                  zsvEwbfbi+Egxi+7An0T7EttZcn6vbS28vtGKXOg6uzaiBN2u2KYq7f3KTq32sgD\
                  QgPEuhsAhQ==",
        )
        .unwrap();

        let timestamp_request: TimestampRequest = picky_asn1_der::from_bytes(&decoded).unwrap();
        check_serde!(timestamp_request: TimestampRequest in decoded);
    }
}
