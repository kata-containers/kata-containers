//! OCI Annotation key constants, taken from:
//! <https://github.com/opencontainers/image-spec/blob/master/annotations.md#pre-defined-annotation-keys>

/// Date and time on which the image was built (string, date-time as defined by RFC 3339)
pub const ORG_OPENCONTAINERS_IMAGE_CREATED: &str = "org.opencontainers.image.created";
/// Contact details of the people or organization responsible for the image (freeform string)
pub const ORG_OPENCONTAINERS_IMAGE_AUTHORS: &str = "org.opencontainers.image.authors";
/// URL to find more information on the image (string)
pub const ORG_OPENCONTAINERS_IMAGE_URL: &str = "org.opencontainers.image.url";
/// URL to get documentation on the image (string)
pub const ORG_OPENCONTAINERS_IMAGE_DOCUMENTATION: &str = "org.opencontainers.image.documentation";
/// URL to get source code for building the image (string)
pub const ORG_OPENCONTAINERS_IMAGE_SOURCE: &str = "org.opencontainers.image.source";
/// Version of the packaged software
pub const ORG_OPENCONTAINERS_IMAGE_VERSION: &str = "org.opencontainers.image.version";
/// Source control revision identifier for the packaged software
pub const ORG_OPENCONTAINERS_IMAGE_REVISION: &str = "org.opencontainers.image.revision";
/// Name of the distributing entity, organization or individual
pub const ORG_OPENCONTAINERS_IMAGE_VENDOR: &str = "org.opencontainers.image.vendor";
/// License(s) under which contained software is distributed as an SPDX License Expression
pub const ORG_OPENCONTAINERS_IMAGE_LICENSES: &str = "org.opencontainers.image.licenses";
/// Name of the reference for a target (string)
pub const ORG_OPENCONTAINERS_IMAGE_REF_NAME: &str = "org.opencontainers.image.ref.name";
/// Human-readable title of the image (string)
pub const ORG_OPENCONTAINERS_IMAGE_TITLE: &str = "org.opencontainers.image.title";
/// Human-readable description of the software packaged in the image (string)
pub const ORG_OPENCONTAINERS_IMAGE_DESCRIPTION: &str = "org.opencontainers.image.description";
/// Digest of the image this image is based on (string)
pub const ORG_OPENCONTAINERS_IMAGE_BASE_DIGEST: &str = "org.opencontainers.image.base.digest";
/// If the `image.base.name` annotation is specified, the `image.base.digest`
/// annotation SHOULD be the digest of the manifest referenced by
/// the `image.ref.name` annotation.
pub const ORG_OPENCONTAINERS_IMAGE_BASE_NAME: &str = "org.opencontainers.image.base.name";
