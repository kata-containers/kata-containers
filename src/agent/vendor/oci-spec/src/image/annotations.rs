/// AnnotationCreated is the annotation key for the date and time on which the
/// image was built (date-time string as defined by RFC 3339).
pub const ANNOTATION_CREATED: &str = "org.opencontainers.image.created";

/// AnnotationAuthors is the annotation key for the contact details of the
/// people or organization responsible for the image (freeform string).
pub const ANNOTATION_AUTHORS: &str = "org.opencontainers.image.authors";

/// AnnotationURL is the annotation key for the URL to find more information on
/// the image.
pub const ANNOTATION_URL: &str = "org.opencontainers.image.url";

/// AnnotationDocumentation is the annotation key for the URL to get
/// documentation on the image.
pub const ANNOTATION_DOCUMENTATION: &str = "org.opencontainers.image.documentation";

/// AnnotationSource is the annotation key for the URL to get source code for
/// building the image.
pub const ANNOTATION_SOURCE: &str = "org.opencontainers.image.source";

/// AnnotationVersion is the annotation key for the version of the packaged
/// software. The version MAY match a label or tag in the source code
/// repository. The version MAY be Semantic versioning-compatible.
pub const ANNOTATION_VERSION: &str = "org.opencontainers.image.version";

/// AnnotationRevision is the annotation key for the source control revision
/// identifier for the packaged software.
pub const ANNOTATION_REVISION: &str = "org.opencontainers.image.revision";

/// AnnotationVendor is the annotation key for the name of the distributing
/// entity, organization or individual.
pub const ANNOTATION_VENDOR: &str = "org.opencontainers.image.vendor";

/// AnnotationLicenses is the annotation key for the license(s) under which
/// contained software is distributed as an SPDX License Expression.
pub const ANNOTATION_LICENSES: &str = "org.opencontainers.image.licenses";

/// AnnotationRefName is the annotation key for the name of the reference for a
/// target. SHOULD only be considered valid when on descriptors on `index.json`
/// within image layout.
pub const ANNOTATION_REF_NAME: &str = "org.opencontainers.image.ref.name";

/// AnnotationTitle is the annotation key for the human-readable title of the
/// image.
pub const ANNOTATION_TITLE: &str = "org.opencontainers.image.title";

/// AnnotationDescription is the annotation key for the human-readable
/// description of the software packaged in the image.
pub const ANNOTATION_DESCRIPTION: &str = "org.opencontainers.image.description";

/// AnnotationBaseImageDigest is the annotation key for the digest of the
/// image's base image.
pub const ANNOTATION_BASE_IMAGE_DIGEST: &str = "org.opencontainers.image.base.digest";

/// AnnotationBaseImageName is the annotation key for the image reference of the
/// image's base image.
pub const ANNOTATION_BASE_IMAGE_NAME: &str = "org.opencontainers.image.base.name";
