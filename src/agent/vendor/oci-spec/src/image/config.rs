use super::{Arch, Os};
use crate::{
    error::{OciSpecError, Result},
    from_file, from_reader, to_file, to_string, to_writer,
};
use derive_builder::Builder;
use getset::{CopyGetters, Getters, MutGetters, Setters};
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
#[cfg(test)]
use std::collections::BTreeMap;
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
};

#[derive(
    Builder,
    Clone,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    MutGetters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// The image configuration is associated with an image and describes some
/// basic information about the image such as date created, author, as
/// well as execution/runtime configuration like its entrypoint, default
/// arguments, networking, and volumes.
pub struct ImageConfiguration {
    /// An combined date and time at which the image was created,
    /// formatted as defined by [RFC 3339, section 5.6.](https://tools.ietf.org/html/rfc3339#section-5.6)
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<String>,
    /// Gives the name and/or email address of the person or entity
    /// which created and is responsible for maintaining the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    /// The CPU architecture which the binaries in this
    /// image are built to run on. Configurations SHOULD use, and
    /// implementations SHOULD understand, values listed in the Go
    /// Language document for [GOARCH](https://golang.org/doc/install/source#environment).
    architecture: Arch,
    /// The name of the operating system which the image is built to run on.
    /// Configurations SHOULD use, and implementations SHOULD understand,
    /// values listed in the Go Language document for [GOOS](https://golang.org/doc/install/source#environment).
    os: Os,
    /// This OPTIONAL property specifies the version of the operating
    /// system targeted by the referenced blob. Implementations MAY refuse
    /// to use manifests where os.version is not known to work with
    /// the host OS version. Valid values are
    /// implementation-defined. e.g. 10.0.14393.1066 on windows.
    #[serde(rename = "os.version", skip_serializing_if = "Option::is_none")]
    os_version: Option<String>,
    /// This OPTIONAL property specifies an array of strings,
    /// each specifying a mandatory OS feature. When os is windows, image
    /// indexes SHOULD use, and implementations SHOULD understand
    /// the following values:
    /// - win32k: image requires win32k.sys on the host (Note: win32k.sys is
    ///   missing on Nano Server)
    #[serde(rename = "os.features", skip_serializing_if = "Option::is_none")]
    os_features: Option<Vec<String>>,
    /// The variant of the specified CPU architecture. Configurations SHOULD
    /// use, and implementations SHOULD understand, variant values
    /// listed in the [Platform Variants](https://github.com/opencontainers/image-spec/blob/main/image-index.md#platform-variants) table.
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
    /// The execution parameters which SHOULD be used as a base when
    /// running a container using the image. This field can be None, in
    /// which case any execution parameters should be specified at
    /// creation of the container.
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<Config>,
    /// The rootfs key references the layer content addresses used by the
    /// image. This makes the image config hash depend on the
    /// filesystem hash.
    #[getset(get_mut = "pub", get = "pub", set = "pub")]
    rootfs: RootFs,
    /// Describes the history of each layer. The array is ordered from first
    /// to last.
    #[getset(get_mut = "pub", get = "pub", set = "pub")]
    history: Vec<History>,
}

impl ImageConfiguration {
    /// Attempts to load an image configuration from a file.
    /// # Errors
    /// This function will return an [OciSpecError::Io](crate::OciSpecError::Io)
    /// if the file does not exist or an
    /// [OciSpecError::SerDe](crate::OciSpecError::SerDe) if the image configuration
    /// cannot be deserialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    ///
    /// let image_index = ImageConfiguration::from_file("config.json").unwrap();
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<ImageConfiguration> {
        from_file(path)
    }

    /// Attempts to load an image configuration from a stream.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe)
    /// if the image configuration cannot be deserialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    /// use std::fs::File;
    ///
    /// let reader = File::open("config.json").unwrap();
    /// let image_index = ImageConfiguration::from_reader(reader).unwrap();
    /// ```
    pub fn from_reader<R: Read>(reader: R) -> Result<ImageConfiguration> {
        from_reader(reader)
    }

    /// Attempts to write an image configuration to a file as JSON. If the file already exists, it
    /// will be overwritten.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    ///
    /// let image_index = ImageConfiguration::from_file("config.json").unwrap();
    /// image_index.to_file("my-config.json").unwrap();
    /// ```
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        to_file(&self, path, false)
    }

    /// Attempts to write an image configuration to a file as pretty printed JSON. If the file
    /// already exists, it will be overwritten.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    ///
    /// let image_index = ImageConfiguration::from_file("config.json").unwrap();
    /// image_index.to_file_pretty("my-config.json").unwrap();
    /// ```
    pub fn to_file_pretty<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        to_file(&self, path, true)
    }

    /// Attempts to write an image configuration to a stream as JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    ///
    /// let image_index = ImageConfiguration::from_file("config.json").unwrap();
    /// let mut writer = Vec::new();
    /// image_index.to_writer(&mut writer);
    /// ```
    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        to_writer(&self, writer, false)
    }

    /// Attempts to write an image configuration to a stream as pretty printed JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    ///
    /// let image_index = ImageConfiguration::from_file("config.json").unwrap();
    /// let mut writer = Vec::new();
    /// image_index.to_writer_pretty(&mut writer);
    /// ```
    pub fn to_writer_pretty<W: Write>(&self, writer: &mut W) -> Result<()> {
        to_writer(&self, writer, true)
    }

    /// Attempts to write an image configuration to a string as JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    ///
    /// let image_configuration = ImageConfiguration::from_file("config.json").unwrap();
    /// let json_str = image_configuration.to_string().unwrap();
    /// ```
    pub fn to_string(&self) -> Result<String> {
        to_string(&self, false)
    }

    /// Attempts to write an image configuration to a string as pretty printed JSON.
    /// # Errors
    /// This function will return an [OciSpecError::SerDe](crate::OciSpecError::SerDe) if
    /// the image configuration cannot be serialized.
    /// # Example
    /// ``` no_run
    /// use oci_spec::image::ImageConfiguration;
    ///
    /// let image_configuration = ImageConfiguration::from_file("config.json").unwrap();
    /// let json_str = image_configuration.to_string_pretty().unwrap();
    /// ```
    pub fn to_string_pretty(&self) -> Result<String> {
        to_string(&self, true)
    }
}

/// Implement `ToString` directly since we cannot avoid twice memory allocation
/// when using auto-implementaion through `Display`.
impl ToString for ImageConfiguration {
    fn to_string(&self) -> String {
        // Serde seralization never fails since this is
        // a combination of String and enums.
        self.to_string_pretty()
            .expect("ImageConfiguration JSON convertion failed")
    }
}

#[derive(
    Builder,
    Clone,
    Debug,
    Default,
    Deserialize,
    Eq,
    Getters,
    MutGetters,
    Setters,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "PascalCase")]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// The execution parameters which SHOULD be used as a base when
/// running a container using the image.
pub struct Config {
    /// The username or UID which is a platform-specific
    /// structure that allows specific control over which
    /// user the process run as. This acts as a default
    /// value to use when the value is not specified when
    /// creating a container. For Linux based systems, all
    /// of the following are valid: user, uid, user:group,
    /// uid:gid, uid:group, user:gid. If group/gid is not
    /// specified, the default group and supplementary
    /// groups of the given user/uid in /etc/passwd from
    /// the container are applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    /// A set of ports to expose from a container running this
    /// image. Its keys can be in the format of: port/tcp, port/udp,
    /// port with the default protocol being tcp if not specified.
    /// These values act as defaults and are merged with any
    /// specified when creating a container.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_as_vec",
        serialize_with = "serialize_as_map"
    )]
    exposed_ports: Option<Vec<String>>,
    /// Entries are in the format of VARNAME=VARVALUE. These
    /// values act as defaults and are merged with any
    /// specified when creating a container.
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<Vec<String>>,
    /// A list of arguments to use as the command to execute
    /// when the container starts. These values act as defaults
    /// and may be replaced by an entrypoint specified when
    /// creating a container.
    #[serde(skip_serializing_if = "Option::is_none")]
    entrypoint: Option<Vec<String>>,
    /// Default arguments to the entrypoint of the container.
    /// These values act as defaults and may be replaced by any
    /// specified when creating a container. If an Entrypoint
    /// value is not specified, then the first entry of the Cmd
    /// array SHOULD be interpreted as the executable to run.
    #[serde(skip_serializing_if = "Option::is_none")]
    cmd: Option<Vec<String>>,
    /// A set of directories describing where the process is
    /// likely to write data specific to a container instance.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_as_vec",
        serialize_with = "serialize_as_map"
    )]
    volumes: Option<Vec<String>>,
    /// Sets the current working directory of the entrypoint process
    /// in the container. This value acts as a default and may be
    /// replaced by a working directory specified when creating
    /// a container.
    #[serde(skip_serializing_if = "Option::is_none")]
    working_dir: Option<String>,
    /// The field contains arbitrary metadata for the container.
    /// This property MUST use the annotation rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_mut = "pub", get = "pub", set = "pub")]
    labels: Option<HashMap<String, String>>,
    /// The field contains the system call signal that will be
    /// sent to the container to exit. The signal can be a signal
    /// name in the format SIGNAME, for instance SIGKILL or SIGRTMIN+3.
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_signal: Option<String>,
}

// Some fields of the image configuration are a json serialization of a
// Go map[string]struct{} leading to the following json:
// {
//    "ExposedPorts": {
//       "8080/tcp": {},
//       "443/tcp": {},
//    }
// }
// Instead we treat this as a list
#[derive(Deserialize, Serialize)]
struct GoMapSerde {}

fn deserialize_as_vec<'de, D>(deserializer: D) -> std::result::Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    // ensure stable order of keys in json document for comparison between expected and actual
    #[cfg(test)]
    let opt = Option::<BTreeMap<String, GoMapSerde>>::deserialize(deserializer)?;
    #[cfg(not(test))]
    let opt = Option::<HashMap<String, GoMapSerde>>::deserialize(deserializer)?;

    if let Some(data) = opt {
        let vec: Vec<String> = data.keys().cloned().collect();
        return Ok(Some(vec));
    }

    Ok(None)
}

fn serialize_as_map<S>(
    target: &Option<Vec<String>>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match target {
        Some(values) => {
            // ensure stable order of keys in json document for comparison between expected and actual
            #[cfg(test)]
            let map: BTreeMap<_, _> = values.iter().map(|v| (v, GoMapSerde {})).collect();
            #[cfg(not(test))]
            let map: HashMap<_, _> = values.iter().map(|v| (v, GoMapSerde {})).collect();

            let mut map_ser = serializer.serialize_map(Some(map.len()))?;
            for (key, value) in map {
                map_ser.serialize_entry(key, &value)?;
            }
            map_ser.end()
        }
        _ => unreachable!(),
    }
}

#[derive(
    Builder, Clone, Debug, Deserialize, Eq, Getters, MutGetters, Setters, PartialEq, Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
#[getset(get = "pub", set = "pub")]
/// RootFs references the layer content addresses used by the image.
pub struct RootFs {
    /// MUST be set to layers.
    #[serde(rename = "type")]
    typ: String,
    /// An array of layer content hashes (DiffIDs), in order
    /// from first to last.
    #[getset(get_mut = "pub", get = "pub", set = "pub")]
    diff_ids: Vec<String>,
}

impl Default for RootFs {
    fn default() -> Self {
        Self {
            typ: "layers".to_owned(),
            diff_ids: Default::default(),
        }
    }
}

#[derive(
    Builder,
    Clone,
    Debug,
    Default,
    Deserialize,
    Eq,
    CopyGetters,
    Getters,
    Setters,
    PartialEq,
    Serialize,
)]
#[builder(
    default,
    pattern = "owned",
    setter(into, strip_option),
    build_fn(error = "OciSpecError")
)]
/// Describes the history of a layer.
pub struct History {
    /// A combined date and time at which the layer was created,
    /// formatted as defined by [RFC 3339, section 5.6.](https://tools.ietf.org/html/rfc3339#section-5.6).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    created: Option<String>,
    /// The author of the build point.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    author: Option<String>,
    /// The command which created the layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    created_by: Option<String>,
    /// A custom message set when creating the layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get = "pub", set = "pub")]
    comment: Option<String>,
    /// This field is used to mark if the history item created
    /// a filesystem diff. It is set to true if this history item
    /// doesn't correspond to an actual layer in the rootfs section
    #[serde(skip_serializing_if = "Option::is_none")]
    #[getset(get_copy = "pub", set = "pub")]
    empty_layer: Option<bool>,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::image::Os;

    fn create_config() -> ImageConfiguration {
        let configuration = ImageConfigurationBuilder::default()
            .created("2015-10-31T22:22:56.015925234Z".to_owned())
            .author("Alyssa P. Hacker <alyspdev@example.com>".to_owned())
            .architecture(Arch::Amd64)
            .os(Os::Linux)
            .config(
                ConfigBuilder::default()
                    .user("alice".to_owned())
                    .exposed_ports(vec!["8080/tcp".to_owned()])
                    .env(vec![
                        "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_owned(),
                        "FOO=oci_is_a".to_owned(),
                        "BAR=well_written_spec".to_owned(),
                    ])
                    .entrypoint(vec!["/bin/my-app-binary".to_owned()])
                    .cmd(vec![
                        "--foreground".to_owned(),
                        "--config".to_owned(),
                        "/etc/my-app.d/default.cfg".to_owned(),
                    ])
                    .volumes(vec![
                        "/var/job-result-data".to_owned(),
                        "/var/log/my-app-logs".to_owned(),
                    ])
                    .working_dir("/home/alice".to_owned())
                    .build().expect("build config"),
            )
            .rootfs(RootFsBuilder::default()
            .diff_ids(vec![
                "sha256:c6f988f4874bb0add23a778f753c65efe992244e148a1d2ec2a8b664fb66bbd1".to_owned(),
                "sha256:5f70bf18a086007016e948b04aed3b82103a36bea41755b6cddfaf10ace3c6ef".to_owned(),
            ])
            .build()
            .expect("build rootfs"))
            .history(vec![
                HistoryBuilder::default()
                .created("2015-10-31T22:22:54.690851953Z".to_owned())
                .created_by("/bin/sh -c #(nop) ADD file:a3bc1e842b69636f9df5256c49c5374fb4eef1e281fe3f282c65fb853ee171c5 in /".to_owned())
                .build()
                .expect("build history"),
                HistoryBuilder::default()
                .created("2015-10-31T22:22:55.613815829Z".to_owned())
                .created_by("/bin/sh -c #(nop) CMD [\"sh\"]".to_owned())
                .empty_layer(true)
                .build()
                .expect("build history"),
            ])
            .build()
            .expect("build configuration");

        configuration
    }

    fn get_config_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/data/config.json")
    }

    #[test]
    fn load_configuration_from_file() {
        // arrange
        let config_path = get_config_path();
        let expected = create_config();

        // act
        let actual = ImageConfiguration::from_file(config_path).expect("from file");

        // assert
        assert_eq!(actual, expected);
    }

    #[test]
    fn load_configuration_from_reader() {
        // arrange
        let reader = fs::read(get_config_path()).expect("read config");

        // act
        let actual = ImageConfiguration::from_reader(&*reader).expect("from reader");
        println!("{:#?}", actual);

        // assert
        let expected = create_config();
        println!("{:#?}", expected);

        assert_eq!(actual, expected);
    }

    #[test]
    fn save_config_to_file() {
        // arrange
        let tmp = std::env::temp_dir().join("save_config_to_file");
        fs::create_dir_all(&tmp).expect("create test directory");
        let config = create_config();
        let config_path = tmp.join("config.json");

        // act
        config
            .to_file_pretty(&config_path)
            .expect("write config to file");

        // assert
        let actual = fs::read_to_string(config_path).expect("read actual");
        let expected = fs::read_to_string(get_config_path()).expect("read expected");
        assert_eq!(actual, expected);
    }

    #[test]
    fn save_config_to_writer() {
        // arrange
        let config = create_config();
        let mut actual = Vec::new();

        // act
        config.to_writer_pretty(&mut actual).expect("to writer");

        // assert
        let expected = fs::read(get_config_path()).expect("read expected");
        assert_eq!(actual, expected);
    }

    #[test]
    fn save_config_to_string() {
        // arrange
        let config = create_config();

        // act
        let actual = config.to_string_pretty().expect("to string");

        // assert
        let expected = fs::read_to_string(get_config_path()).expect("read expected");
        assert_eq!(actual, expected);
    }
}
