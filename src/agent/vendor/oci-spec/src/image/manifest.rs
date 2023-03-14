use super::{Descriptor, MediaType};
use crate::{
    error::{OciSpecError, Result},
    from_file, from_reader, to_file, to_string, to_writer,
};
use derive_builder::Builder;
use getset::{CopyGetters, Getters, MutGetters, Setters};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
};

#[derive(
    Builder,
    Clone,
    CopyGetters,
    Debug,
    Deserialize,
    Eq,
    Getters,
    MutGetters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "camelCase")]
#[builder(
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// Unlike the image index, which contains information about a set of images
/// that can span a variety of architectures and operating systems, an image
/// manifest provides a configuration and set of layers for a single
/// container image for a specific architecture and operating system.
pub struct ImageManifest {
    /// This REQUIRED property specifies the image manifest schema version.
    /// For this version of the specification, this MUST be 2 to ensure
    /// backward compatibility with older versions of Docker. The
    /// value of this field will not change. This field MAY be
    /// removed in a future version of the specification.
    #[getset(get_copy = "pub", set = "pub")]
    schema_version: u32,
    /// This property is reserved for use, to maintain compatibility. When
    /// used, this field contains the media type of this document,
    /// which differs from the descriptor use of mediaType.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    media_type: Option<MediaType>,
    /// This REQUIRED property references a configuration object for a
    /// container, by digest. Beyond the descriptor requirements,
    /// the value has the following additional restrictions:
    /// The media type descriptor property has additional restrictions for
    /// config. Implementations MUST support at least the following
    /// media types:
    /// - application/vnd.oci.image.config.v1+json
    /// Manifests concerned with portability SHOULD use one of the above
    /// media types.
    #[getset(get = "pub", set = "pub")]
    config: Descriptor,
    /// Each item in the array MUST be a descriptor. The array MUST have the
    /// base layer at index 0. Subsequent layers MUST then follow in
    /// stack order (i.e. from `layers[0]` to `layers[len(layers)-1]`).
    /// The final filesystem layout MUST match the result of applying
    /// the layers to an empty directory. The ownership, mode, and other
    /// attributes of the initial empty directory are unspecified.
    #[getset(get_mut = "pub", get = "pub", set = "pub")]
    layers: Vec<Descriptor>,
    /// This OPTIONAL property contains arbitrary metadata for the image
    /// manifest. This OPTIONAL property MUST use the annotation
    /// rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_mut = "pub", get = "pub", set = "pub")]
    #[builder(default)]
    annotations: Option<HashMap<String, String>>,
}

impl ImageManifest {
    /// Attempts to load an image manifest from a file.
    /// # Errors
    /// This function will return an [OciSpecError::Io](crate::OciSpecError::Io)
    /// if the file does not exist or an
    /// [OciSpecError::SerDe](crate::OciSpecError::SerDe) if the image manifest
    /// cannot be deserialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    ///
    /// let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<ImageManifest> {
        from_file(path)
    }

    /// Attempts to load an image manifest from a stream.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe)
    /// if the manifest cannot be deserialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    /// use std::fs::File;
    ///
    /// let reader = File::open("manifest.json").unwrap();
    /// let image_manifest = ImageManifest::from_reader(reader).unwrap();
    /// ```
    pub fn from_reader<R: Read>(reader: R) -> Result<ImageManifest> {
        from_reader(reader)
    }

    /// Attempts to write an image manifest to a file as JSON. If the file already exists, it
    /// will be overwritten.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image manifest cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    ///
    /// let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
    /// image_manifest.to_file("my-manifest.json").unwrap();
    /// ```
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        to_file(&self, path, false)
    }

    /// Attempts to write an image manifest to a file as pretty printed JSON. If the file already exists, it
    /// will be overwritten.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image manifest cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    ///
    /// let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
    /// image_manifest.to_file_pretty("my-manifest.json").unwrap();
    /// ```
    pub fn to_file_pretty<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        to_file(&self, path, true)
    }

    /// Attempts to write an image manifest to a stream as JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image manifest cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    ///
    /// let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
    /// let mut writer = Vec::new();
    /// image_manifest.to_writer(&mut writer);
    /// ```
    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        to_writer(&self, writer, false)
    }

    /// Attempts to write an image manifest to a stream as pretty printed JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image manifest cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    ///
    /// let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
    /// let mut writer = Vec::new();
    /// image_manifest.to_writer_pretty(&mut writer);
    /// ```
    pub fn to_writer_pretty<W: Write>(&self, writer: &mut W) -> Result<()> {
        to_writer(&self, writer, true)
    }

    /// Attempts to write an image manifest to a string as JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    ///
    /// let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
    /// let json_str = image_manifest.to_string().unwrap();
    /// ```
    pub fn to_string(&self) -> Result<String> {
        to_string(&self, false)
    }

    /// Attempts to write an image manifest to a string as pretty printed JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageManifest;
    ///
    /// let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
    /// let json_str = image_manifest.to_string_pretty().unwrap();
    /// ```
    pub fn to_string_pretty(&self) -> Result<String> {
        to_string(&self, true)
    }
}

/// Implement `ToString` directly since we cannot avoid twice memory allocation
/// when using auto-implementaion through `Display`.
impl ToString for ImageManifest {
    fn to_string(&self) -> String {
        // Serde seralization never fails since this is
        // a combination of String and enums.
        self.to_string_pretty()
            .expect("ImageManifest to JSON convertion failed")
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::image::{Descriptor, DescriptorBuilder};

    fn create_manifest() -> ImageManifest {
        use crate::image::SCHEMA_VERSION;

        let config = DescriptorBuilder::default()
            .media_type(MediaType::ImageConfig)
            .size(7023)
            .digest("sha256:b5b2b2c507a0944348e0303114d8d93aaaa081732b86451d9bce1f432a537bc7")
            .build()
            .expect("build config descriptor");

        let layers: Vec<Descriptor> = [
            (
                32654,
                "sha256:9834876dcfb05cb167a5c24953eba58c4ac89b1adf57f28f2f9d09af107ee8f0",
            ),
            (
                16724,
                "sha256:3c3a4604a545cdc127456d94e421cd355bca5b528f4a9c1905b15da2eb4a4c6b",
            ),
            (
                73109,
                "sha256:ec4b8955958665577945c89419d1af06b5f7636b4ac3da7f12184802ad867736",
            ),
        ]
        .iter()
        .map(|l| {
            DescriptorBuilder::default()
                .media_type(MediaType::ImageLayerGzip)
                .size(l.0)
                .digest(l.1.to_owned())
                .build()
                .expect("build layer")
        })
        .collect();

        let manifest = ImageManifestBuilder::default()
            .schema_version(SCHEMA_VERSION)
            .config(config)
            .layers(layers)
            .build()
            .expect("build image manifest");

        manifest
    }

    fn get_manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/data/manifest.json")
    }

    #[test]
    fn load_manifest_from_file() {
        // arrange
        let manifest_path = get_manifest_path();
        let expected = create_manifest();

        // act
        let actual = ImageManifest::from_file(manifest_path).expect("from file");

        // assert
        assert_eq!(actual, expected);
    }

    #[test]
    fn getset() {
        let mut manifest = create_manifest();
        assert_eq!(manifest.layers().len(), 3);
        let layer_copy = manifest.layers()[0].clone();
        manifest.layers_mut().push(layer_copy);
        assert_eq!(manifest.layers().len(), 4);
    }

    #[test]
    fn load_manifest_from_reader() {
        // arrange
        let reader = fs::read(get_manifest_path()).expect("read manifest");

        // act
        let actual = ImageManifest::from_reader(&*reader).expect("from reader");

        // assert
        let expected = create_manifest();
        assert_eq!(actual, expected);
    }

    #[test]
    fn save_manifest_to_file() {
        // arrange
        let tmp = std::env::temp_dir().join("save_manifest_to_file");
        fs::create_dir_all(&tmp).expect("create test directory");
        let manifest = create_manifest();
        let manifest_path = tmp.join("manifest.json");

        // act
        manifest
            .to_file_pretty(&manifest_path)
            .expect("write manifest to file");

        // assert
        let actual = fs::read_to_string(manifest_path).expect("read actual");
        let expected = fs::read_to_string(get_manifest_path()).expect("read expected");
        assert_eq!(actual, expected);
    }

    #[test]
    fn save_manifest_to_writer() {
        // arrange
        let manifest = create_manifest();
        let mut actual = Vec::new();

        // act
        manifest.to_writer_pretty(&mut actual).expect("to writer");

        // assert
        let expected = fs::read(get_manifest_path()).expect("read expected");
        assert_eq!(actual, expected);
    }

    #[test]
    fn save_manifest_to_string() {
        // arrange
        let manifest = create_manifest();

        // act
        let actual = manifest.to_string_pretty().expect("to string");

        // assert
        let expected = fs::read_to_string(get_manifest_path()).expect("read expected");
        assert_eq!(actual, expected);
    }
}
