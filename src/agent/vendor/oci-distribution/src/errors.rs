//! Errors related to interacting with an OCI compliant remote store

use thiserror::Error;

/// Errors that can be raised while interacting with an OCI registry
#[derive(Error, Debug)]
pub enum OciDistributionError {
    /// Authentication error
    #[error("Authentication failure: {0}")]
    AuthenticationFailure(String),
    /// Generic error, might provide an explanation message
    #[error("Generic error: {0:?}")]
    GenericError(Option<String>),
    /// Transparent wrapper around `reqwest::header::ToStrError`
    #[error(transparent)]
    HeaderValueError(#[from] reqwest::header::ToStrError),
    /// IO Error
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    /// Platform resolver not specified
    #[error("Received Image Index/Manifest List, but platform_resolver was not defined on the client config. Consider setting platform_resolver")]
    ImageIndexParsingNoPlatformResolverError,
    /// Image manifest not found
    #[error("Image manifest not found: {0}")]
    ImageManifestNotFoundError(String),
    /// Registry returned a layer with an incompatible type
    #[error("Incompatible layer media type: {0}")]
    IncompatibleLayerMediaTypeError(String),
    #[error(transparent)]
    /// Transparent wrapper around `serde_json::error::Error`
    JsonError(#[from] serde_json::error::Error),
    /// Manifest: JSON unmarshalling error
    #[error("Failed to parse manifest as Versioned object: {0}")]
    ManifestParsingError(String),
    /// Cannot push a blob without data
    #[error("cannot push a blob without data")]
    PushNoDataError,
    /// Cannot push layer object without data
    #[error("cannot push a layer without data")]
    PushLayerNoDataError,
    /// No layers available to be pulled
    #[error("No layers to pull")]
    PullNoLayersError,
    /// OCI registry error
    #[error("Registry error: url {url}, envelope: {envelope}")]
    RegistryError {
        /// List of errors returned the by the OCI registry
        envelope: OciEnvelope,
        /// Request URL
        url: String,
    },
    /// Registry didn't return a Digest object
    #[error("Registry did not return a digest header")]
    RegistryNoDigestError,
    /// Registry didn't return a Location header
    #[error("Registry did not return a location header")]
    RegistryNoLocationError,
    /// Registry token: JSON deserialization error
    #[error("Failed to decode registry token: {0}")]
    RegistryTokenDecodeError(String),
    /// Transparent wrapper around `reqwest::Error`
    #[error(transparent)]
    RequestError(#[from] reqwest::Error),
    /// HTTP Server error
    #[error("Server error: url {url}, code: {code}, message: {message}")]
    ServerError {
        /// HTTP status code
        code: u16,
        /// Request URL
        url: String,
        /// Error message returned by the remote server
        message: String,
    },
    /// The [OCI distribution spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md)
    /// is not respected by the remote registry
    #[error("OCI distribution spec violation: {0}")]
    SpecViolationError(String),
    /// HTTP auth failed - user not authorized
    #[error("Not authorized: url {url}")]
    UnauthorizedError {
        /// request URL
        url: String,
    },
    /// Media type not supported
    #[error("Unsupported media type: {0}")]
    UnsupportedMediaTypeError(String),
    /// Schema version not supported
    #[error("Unsupported schema version: {0}")]
    UnsupportedSchemaVersionError(i32),
    /// Versioned object: JSON deserialization error
    #[error("Failed to parse manifest: {0}")]
    VersionedParsingError(String),
}

/// Helper type to declare `Result` objects that might return a `OciDistributionError`
pub type Result<T> = std::result::Result<T, OciDistributionError>;

/// The OCI specification defines a specific error format.
///
/// This struct represents that error format, which is formally described here:
/// <https://github.com/opencontainers/distribution-spec/blob/master/spec.md#errors-2>
#[derive(serde::Deserialize, Debug)]
pub struct OciError {
    /// The error code
    pub code: OciErrorCode,
    /// An optional message associated with the error
    #[serde(default)]
    pub message: String,
    /// Unstructured optional data associated with the error
    #[serde(default)]
    pub detail: serde_json::Value,
}

impl std::error::Error for OciError {
    fn description(&self) -> &str {
        self.message.as_str()
    }
}
impl std::fmt::Display for OciError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OCI API error: {}", self.message.as_str())
    }
}

/// A struct that holds a series of OCI errors
#[derive(serde::Deserialize, Debug)]
pub struct OciEnvelope {
    pub(crate) errors: Vec<OciError>,
}

impl std::fmt::Display for OciEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let errors: Vec<String> = self.errors.iter().map(|e| e.to_string()).collect();
        write!(f, "OCI API errors: [{}]", errors.join("\n"))
    }
}

/// OCI error codes
///
/// Outlined [here](https://github.com/opencontainers/distribution-spec/blob/master/spec.md#errors-2)
#[derive(serde::Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OciErrorCode {
    /// Blob unknown to registry
    ///
    /// This error MAY be returned when a blob is unknown to the registry in a specified
    /// repository. This can be returned with a standard get or if a manifest
    /// references an unknown layer during upload.
    BlobUnknown,
    /// Blob upload is invalid
    ///
    /// The blob upload encountered an error and can no longer proceed.
    BlobUploadInvalid,
    /// Blob upload is unknown to registry
    BlobUploadUnknown,
    /// Provided digest did not match uploaded content.
    DigestInvalid,
    /// Blob is unknown to registry
    ManifestBlobUnknown,
    /// Manifest is invalid
    ///
    /// During upload, manifests undergo several checks ensuring validity. If
    /// those checks fail, this error MAY be returned, unless a more specific
    /// error is included. The detail will contain information the failed
    /// validation.
    ManifestInvalid,
    /// Manifest unknown
    ///
    /// This error is returned when the manifest, identified by name and tag is unknown to the repository.
    ManifestUnknown,
    /// Manifest failed signature validation
    ///
    /// DEPRECATED: This error code has been removed from the OCI spec.
    ManifestUnverified,
    /// Invalid repository name
    NameInvalid,
    /// Repository name is not known
    NameUnknown,
    /// Provided length did not match content length
    SizeInvalid,
    /// Manifest tag did not match URI
    ///
    /// DEPRECATED: This error code has been removed from the OCI spec.
    TagInvalid,
    /// Authentication required.
    Unauthorized,
    /// Requested access to the resource is denied
    Denied,
    /// This operation is unsupported
    Unsupported,
    /// Too many requests from client
    Toomanyrequests,
}

#[cfg(test)]
mod test {
    use super::*;

    const EXAMPLE_ERROR: &str = r#"
      {"errors":[{"code":"UNAUTHORIZED","message":"authentication required","detail":[{"Type":"repository","Name":"hello-wasm","Action":"pull"}]}]}
      "#;
    #[test]
    fn test_deserialize() {
        let envelope: OciEnvelope =
            serde_json::from_str(EXAMPLE_ERROR).expect("parse example error");
        let e = &envelope.errors[0];
        assert_eq!(OciErrorCode::Unauthorized, e.code);
        assert_eq!("authentication required", e.message);
        assert_ne!(serde_json::value::Value::Null, e.detail);
    }

    const EXAMPLE_ERROR_TOOMANYREQUESTS: &str = r#"
      {"errors":[{"code":"TOOMANYREQUESTS","message":"pull request limit exceeded","detail":"You have reached your pull rate limit."}]}
      "#;
    #[test]
    fn test_deserialize_toomanyrequests() {
        let envelope: OciEnvelope =
            serde_json::from_str(EXAMPLE_ERROR_TOOMANYREQUESTS).expect("parse example error");
        let e = &envelope.errors[0];
        assert_eq!(OciErrorCode::Toomanyrequests, e.code);
        assert_eq!("pull request limit exceeded", e.message);
        assert_ne!(serde_json::value::Value::Null, e.detail);
    }

    const EXAMPLE_ERROR_MISSING_MESSAGE: &str = r#"
      {"errors":[{"code":"UNAUTHORIZED","detail":[{"Type":"repository","Name":"hello-wasm","Action":"pull"}]}]}
      "#;
    #[test]
    fn test_deserialize_without_message_field() {
        let envelope: OciEnvelope =
            serde_json::from_str(EXAMPLE_ERROR_MISSING_MESSAGE).expect("parse example error");
        let e = &envelope.errors[0];
        assert_eq!(OciErrorCode::Unauthorized, e.code);
        assert_eq!(String::default(), e.message);
        assert_ne!(serde_json::value::Value::Null, e.detail);
    }

    const EXAMPLE_ERROR_MISSING_DETAIL: &str = r#"
      {"errors":[{"code":"UNAUTHORIZED","message":"authentication required"}]}
      "#;
    #[test]
    fn test_deserialize_without_detail_field() {
        let envelope: OciEnvelope =
            serde_json::from_str(EXAMPLE_ERROR_MISSING_DETAIL).expect("parse example error");
        let e = &envelope.errors[0];
        assert_eq!(OciErrorCode::Unauthorized, e.code);
        assert_eq!("authentication required", e.message);
        assert_eq!(serde_json::value::Value::Null, e.detail);
    }
}
