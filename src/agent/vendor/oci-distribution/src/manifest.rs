//! OCI Manifest
use std::collections::HashMap;

use crate::{
    client::{Config, ImageLayer},
    sha256_digest,
};

/// The mediatype for WASM layers.
pub const WASM_LAYER_MEDIA_TYPE: &str = "application/vnd.wasm.content.layer.v1+wasm";
/// The mediatype for a WASM image config.
pub const WASM_CONFIG_MEDIA_TYPE: &str = "application/vnd.wasm.config.v1+json";
/// The mediatype for an docker v2 schema 2 manifest.
pub const IMAGE_MANIFEST_MEDIA_TYPE: &str = "application/vnd.docker.distribution.manifest.v2+json";
/// The mediatype for an docker v2 shema 2 manifest list.
pub const IMAGE_MANIFEST_LIST_MEDIA_TYPE: &str =
    "application/vnd.docker.distribution.manifest.list.v2+json";
/// The mediatype for an OCI image index manifest.
pub const OCI_IMAGE_INDEX_MEDIA_TYPE: &str = "application/vnd.oci.image.index.v1+json";
/// The mediatype for an OCI image manifest.
pub const OCI_IMAGE_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
/// The mediatype for an image config (manifest).
pub const IMAGE_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.image.config.v1+json";
/// The mediatype that Docker uses for image configs.
pub const IMAGE_DOCKER_CONFIG_MEDIA_TYPE: &str = "application/vnd.docker.container.image.v1+json";
/// The mediatype for a layer.
pub const IMAGE_LAYER_MEDIA_TYPE: &str = "application/vnd.oci.image.layer.v1.tar";
/// The mediatype for a layer that is gzipped.
pub const IMAGE_LAYER_GZIP_MEDIA_TYPE: &str = "application/vnd.oci.image.layer.v1.tar+gzip";
/// The mediatype that Docker uses for a layer that is tarred.
pub const IMAGE_DOCKER_LAYER_TAR_MEDIA_TYPE: &str = "application/vnd.docker.image.rootfs.diff.tar";
/// The mediatype that Docker uses for a layer that is gzipped.
pub const IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE: &str =
    "application/vnd.docker.image.rootfs.diff.tar.gzip";
/// The mediatype for a layer that is nondistributable.
pub const IMAGE_LAYER_NONDISTRIBUTABLE_MEDIA_TYPE: &str =
    "application/vnd.oci.image.layer.nondistributable.v1.tar";
/// The mediatype for a layer that is nondistributable and gzipped.
pub const IMAGE_LAYER_NONDISTRIBUTABLE_GZIP_MEDIA_TYPE: &str =
    "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip";

/// An image, or image index, OCI manifest
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum OciManifest {
    /// An OCI image manifest
    Image(OciImageManifest),
    /// An OCI image index manifest
    ImageIndex(OciImageIndex),
}

impl OciManifest {
    /// Returns the appropriate content-type for each variant.
    pub fn content_type(&self) -> &str {
        match self {
            OciManifest::Image(_) => OCI_IMAGE_MEDIA_TYPE,
            OciManifest::ImageIndex(_) => IMAGE_MANIFEST_LIST_MEDIA_TYPE,
        }
    }
}

/// The OCI image manifest describes an OCI image.
///
/// It is part of the OCI specification, and is defined [here](https://github.com/opencontainers/image-spec/blob/main/manifest.md)
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OciImageManifest {
    /// This is a schema version.
    ///
    /// The specification does not specify the width of this integer.
    /// However, the only version allowed by the specification is `2`.
    /// So we have made this a u8.
    pub schema_version: u8,

    /// This is an optional media type describing this manifest.
    ///
    /// This property SHOULD be used and [remain compatible](https://github.com/opencontainers/image-spec/blob/main/media-types.md#compatibility-matrix)
    /// with earlier versions of this specification and with other similar external formats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,

    /// The image configuration.
    ///
    /// This object is required.
    pub config: OciDescriptor,

    /// The OCI image layers
    ///
    /// The specification is unclear whether this is required. We have left it
    /// required, assuming an empty vector can be used if necessary.
    pub layers: Vec<OciDescriptor>,

    /// The annotations for this manifest
    ///
    /// The specification says "If there are no annotations then this property
    /// MUST either be absent or be an empty map."
    /// TO accomodate either, this is optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

impl Default for OciImageManifest {
    fn default() -> Self {
        OciImageManifest {
            schema_version: 2,
            media_type: None,
            config: OciDescriptor::default(),
            layers: vec![],
            annotations: None,
        }
    }
}

impl OciImageManifest {
    /// Create a new OciImageManifest using the given parameters
    ///
    /// This can be useful to create an OCI Image Manifest with
    /// custom annotations.
    pub fn build(
        layers: &[ImageLayer],
        config: &Config,
        annotations: Option<HashMap<String, String>>,
    ) -> Self {
        let mut manifest = OciImageManifest::default();

        manifest.config.media_type = config.media_type.to_string();
        manifest.config.size = config.data.len() as i64;
        manifest.config.digest = sha256_digest(&config.data);
        manifest.annotations = annotations;

        for layer in layers {
            let digest = sha256_digest(&layer.data);

            let descriptor = OciDescriptor {
                size: layer.data.len() as i64,
                digest,
                media_type: layer.media_type.to_string(),
                annotations: layer.annotations.clone(),
                ..Default::default()
            };

            manifest.layers.push(descriptor);
        }

        manifest
    }
}

impl From<OciImageIndex> for OciManifest {
    fn from(m: OciImageIndex) -> Self {
        Self::ImageIndex(m)
    }
}
impl From<OciImageManifest> for OciManifest {
    fn from(m: OciImageManifest) -> Self {
        Self::Image(m)
    }
}

impl std::fmt::Display for OciManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OciManifest::Image(oci_image_manifest) => write!(f, "{}", oci_image_manifest),
            OciManifest::ImageIndex(oci_image_index) => write!(f, "{}", oci_image_index),
        }
    }
}

impl std::fmt::Display for OciImageIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let media_type = self
            .media_type
            .clone()
            .unwrap_or_else(|| String::from("N/A"));
        let manifests: Vec<String> = self.manifests.iter().map(|m| m.to_string()).collect();
        write!(
            f,
            "OCI Image Index( schema-version: '{}', media-type: '{}', manifests: '{}' )",
            self.schema_version,
            media_type,
            manifests.join(","),
        )
    }
}

impl std::fmt::Display for OciImageManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let media_type = self
            .media_type
            .clone()
            .unwrap_or_else(|| String::from("N/A"));
        let annotations = self.annotations.clone().unwrap_or_default();
        let layers: Vec<String> = self.layers.iter().map(|l| l.to_string()).collect();

        write!(
            f,
            "OCI Image Manifest( schema-version: '{}', media-type: '{}', config: '{}', layers: '{:?}', annotations: '{:?}' )",
            self.schema_version,
            media_type,
            self.config,
            layers,
            annotations,
        )
    }
}

/// Versioned provides a struct with the manifest's schemaVersion and mediaType.
/// Incoming content with unknown schema versions can be decoded against this
/// struct to check the version.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Versioned {
    /// schema_version is the image manifest schema that this image follows
    pub schema_version: i32,

    /// media_type is the media type of this schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

/// The OCI descriptor is a generic object used to describe other objects.
///
/// It is defined in the [OCI Image Specification](https://github.com/opencontainers/image-spec/blob/main/descriptor.md#properties):
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OciDescriptor {
    /// The media type of this descriptor.
    ///
    /// Layers, config, and manifests may all have descriptors. Each
    /// is differentiated by its mediaType.
    ///
    /// This REQUIRED property contains the media type of the referenced
    /// content. Values MUST comply with RFC 6838, including the naming
    /// requirements in its section 4.2.
    pub media_type: String,
    /// The SHA 256 or 512 digest of the object this describes.
    ///
    /// This REQUIRED property is the digest of the targeted content, conforming
    /// to the requirements outlined in Digests. Retrieved content SHOULD be
    /// verified against this digest when consumed via untrusted sources.
    pub digest: String,
    /// The size, in bytes, of the object this describes.
    ///
    /// This REQUIRED property specifies the size, in bytes, of the raw
    /// content. This property exists so that a client will have an expected
    /// size for the content before processing. If the length of the retrieved
    /// content does not match the specified length, the content SHOULD NOT be
    /// trusted.
    pub size: i64,
    /// This OPTIONAL property specifies a list of URIs from which this
    /// object MAY be downloaded. Each entry MUST conform to RFC 3986.
    /// Entries SHOULD use the http and https schemes, as defined in RFC 7230.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<String>>,

    /// This OPTIONAL property contains arbitrary metadata for this descriptor.
    /// This OPTIONAL property MUST use the annotation rules.
    /// <https://github.com/opencontainers/image-spec/blob/main/annotations.md#rules>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

impl std::fmt::Display for OciDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let urls = self.urls.clone().unwrap_or_default();
        let annotations = self.annotations.clone().unwrap_or_default();

        write!(
            f,
            "( media-type: '{}', digest: '{}', size: '{}', urls: '{:?}', annotations: '{:?}' )",
            self.media_type, self.digest, self.size, urls, annotations,
        )
    }
}

impl Default for OciDescriptor {
    fn default() -> Self {
        OciDescriptor {
            media_type: IMAGE_CONFIG_MEDIA_TYPE.to_owned(),
            digest: "".to_owned(),
            size: 0,
            urls: None,
            annotations: None,
        }
    }
}

/// The image index is a higher-level manifest which points to specific image manifest.
///
/// It is part of the OCI specification, and is defined [here](https://github.com/opencontainers/image-spec/blob/main/image-index.md):
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OciImageIndex {
    /// This is a schema version.
    ///
    /// The specification does not specify the width of this integer.
    /// However, the only version allowed by the specification is `2`.
    /// So we have made this a u8.
    pub schema_version: u8,

    /// This is an optional media type describing this manifest.
    ///
    /// It is reserved for compatibility, but the specification does not seem
    /// to recommend setting it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,

    /// This property contains a list of manifests for specific platforms.
    /// The spec says this field must be present but the value may be an empty array.
    pub manifests: Vec<ImageIndexEntry>,

    /// The annotations for this manifest
    ///
    /// The specification says "If there are no annotations then this property
    /// MUST either be absent or be an empty map."
    /// TO accomodate either, this is optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

/// The manifest entry of an `ImageIndex`.
///
/// It is part of the OCI specification, and is defined in the `manifests`
/// section [here](https://github.com/opencontainers/image-spec/blob/main/image-index.md#image-index-property-descriptions):
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageIndexEntry {
    /// The media type of this descriptor.
    ///
    /// Layers, config, and manifests may all have descriptors. Each
    /// is differentiated by its mediaType.
    ///
    /// This REQUIRED property contains the media type of the referenced
    /// content. Values MUST comply with RFC 6838, including the naming
    /// requirements in its section 4.2.
    pub media_type: String,
    /// The SHA 256 or 512 digest of the object this describes.
    ///
    /// This REQUIRED property is the digest of the targeted content, conforming
    /// to the requirements outlined in Digests. Retrieved content SHOULD be
    /// verified against this digest when consumed via untrusted sources.
    pub digest: String,
    /// The size, in bytes, of the object this describes.
    ///
    /// This REQUIRED property specifies the size, in bytes, of the raw
    /// content. This property exists so that a client will have an expected
    /// size for the content before processing. If the length of the retrieved
    /// content does not match the specified length, the content SHOULD NOT be
    /// trusted.
    pub size: i64,
    /// This OPTIONAL property describes the minimum runtime requirements of the image.
    /// This property SHOULD be present if its target is platform-specific.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,

    /// This OPTIONAL property contains arbitrary metadata for the image index.
    /// This OPTIONAL property MUST use the [annotation rules](https://github.com/opencontainers/image-spec/blob/main/annotations.md#rules).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

impl std::fmt::Display for ImageIndexEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let platform = self
            .platform
            .clone()
            .map(|p| p.to_string())
            .unwrap_or_else(|| String::from("N/A"));
        let annotations = self.annotations.clone().unwrap_or_default();

        write!(
            f,
            "(media-type: '{}', digest: '{}', size: '{}', platform: '{}', annotations: {:?})",
            self.media_type, self.digest, self.size, platform, annotations,
        )
    }
}

/// Platform specific fields of an Image Index manifest entry.
///
/// It is part of the OCI specification, and is in the `platform`
/// section [here](https://github.com/opencontainers/image-spec/blob/main/image-index.md#image-index-property-descriptions):
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    /// This REQUIRED property specifies the CPU architecture.
    /// Image indexes SHOULD use, and implementations SHOULD understand, values
    /// listed in the Go Language document for [`GOARCH`](https://golang.org/doc/install/source#environment).
    pub architecture: String,
    /// This REQUIRED property specifies the operating system.
    /// Image indexes SHOULD use, and implementations SHOULD understand, values
    /// listed in the Go Language document for [`GOOS`](https://golang.org/doc/install/source#environment).
    pub os: String,
    /// This OPTIONAL property specifies the version of the operating system
    /// targeted by the referenced blob.
    /// Implementations MAY refuse to use manifests where `os.version` is not known
    /// to work with the host OS version.
    /// Valid values are implementation-defined. e.g. `10.0.14393.1066` on `windows`.
    #[serde(rename = "os.version")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// This OPTIONAL property specifies an array of strings, each specifying a mandatory OS feature.
    /// When `os` is `windows`, image indexes SHOULD use, and implementations SHOULD understand the following values:
    /// - `win32k`: image requires `win32k.sys` on the host (Note: `win32k.sys` is missing on Nano Server)
    /// When `os` is not `windows`, values are implementation-defined and SHOULD be submitted to this specification for standardization.
    #[serde(rename = "os.features")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_features: Option<Vec<String>>,
    /// This OPTIONAL property specifies the variant of the CPU.
    /// Image indexes SHOULD use, and implementations SHOULD understand, `variant` values listed in the [Platform Variants](#platform-variants) table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// This property is RESERVED for future versions of the specification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let os_version = self
            .os_version
            .clone()
            .unwrap_or_else(|| String::from("N/A"));
        let os_features = self.os_features.clone().unwrap_or_default();
        let variant = self.variant.clone().unwrap_or_else(|| String::from("N/A"));
        let features = self.os_features.clone().unwrap_or_default();
        write!(f, "( architecture: '{}', os: '{}', os-version: '{}', os-features: '{:?}', variant: '{}', features: '{:?}' )",
            self.architecture,
            self.os,
            os_version,
            os_features,
            variant,
            features,
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    const TEST_MANIFEST: &str = r#"{
        "schemaVersion": 2,
        "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
        "config": {
            "mediaType": "application/vnd.docker.container.image.v1+json",
            "size": 2,
            "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        },
        "layers": [
            {
                "mediaType": "application/vnd.wasm.content.layer.v1+wasm",
                "size": 1615998,
                "digest": "sha256:f9c91f4c280ab92aff9eb03b279c4774a80b84428741ab20855d32004b2b983f",
                "annotations": {
                    "org.opencontainers.image.title": "module.wasm"
                }
            }
        ]
    }
    "#;

    #[test]
    fn test_manifest() {
        let manifest: OciImageManifest =
            serde_json::from_str(TEST_MANIFEST).expect("parsed manifest");
        assert_eq!(2, manifest.schema_version);
        assert_eq!(
            Some(IMAGE_MANIFEST_MEDIA_TYPE.to_owned()),
            manifest.media_type
        );
        let config = manifest.config;
        // Note that this is the Docker config media type, not the OCI one.
        assert_eq!(IMAGE_DOCKER_CONFIG_MEDIA_TYPE.to_owned(), config.media_type);
        assert_eq!(2, config.size);
        assert_eq!(
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a".to_owned(),
            config.digest
        );

        assert_eq!(1, manifest.layers.len());
        let wasm_layer = &manifest.layers[0];
        assert_eq!(1_615_998, wasm_layer.size);
        assert_eq!(WASM_LAYER_MEDIA_TYPE.to_owned(), wasm_layer.media_type);
        assert_eq!(
            1,
            wasm_layer
                .annotations
                .as_ref()
                .expect("annotations map")
                .len()
        );
    }
}
