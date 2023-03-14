//! [OCI image spec](https://github.com/opencontainers/image-spec) types and definitions.

mod annotations;
mod config;
mod descriptor;
mod index;
mod manifest;
mod version;

use std::fmt::Display;

use serde::{Deserialize, Serialize};

pub use annotations::*;
pub use config::*;
pub use descriptor::*;
pub use index::*;
pub use manifest::*;
pub use version::*;

/// Media types used by OCI image format spec. Values MUST comply with RFC 6838,
/// including the naming requirements in its section 4.2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaType {
    /// MediaType Descriptor specifies the media type for a content descriptor.
    Descriptor,
    /// MediaType LayoutHeader specifies the media type for the oci-layout.
    LayoutHeader,
    /// MediaType ImageManifest specifies the media type for an image manifest.
    ImageManifest,
    /// MediaType ImageIndex specifies the media type for an image index.
    ImageIndex,
    /// MediaType ImageLayer is the media type used for layers referenced by the
    /// manifest.
    ImageLayer,
    /// MediaType ImageLayerGzip is the media type used for gzipped layers
    /// referenced by the manifest.
    ImageLayerGzip,
    /// MediaType ImageLayerZstd is the media type used for zstd compressed
    /// layers referenced by the manifest.
    ImageLayerZstd,
    /// MediaType ImageLayerNonDistributable is the media type for layers
    /// referenced by the manifest but with distribution restrictions.
    ImageLayerNonDistributable,
    /// MediaType ImageLayerNonDistributableGzip is the media type for
    /// gzipped layers referenced by the manifest but with distribution
    /// restrictions.
    ImageLayerNonDistributableGzip,
    /// MediaType ImageLayerNonDistributableZstd is the media type for zstd
    /// compressed layers referenced by the manifest but with distribution
    /// restrictions.
    ImageLayerNonDistributableZstd,
    /// MediaType ImageConfig specifies the media type for the image
    /// configuration.
    ImageConfig,
    /// MediaType not specified by OCI image format.
    Other(String),
}

impl Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Descriptor => write!(f, "application/vnd.oci.descriptor"),
            Self::LayoutHeader => write!(f, "application/vnd.oci.layout.header.v1+json"),
            Self::ImageManifest => write!(f, "application/vnd.oci.image.manifest.v1+json"),
            Self::ImageIndex => write!(f, "application/vnd.oci.image.index.v1+json"),
            Self::ImageLayer => write!(f, "application/vnd.oci.image.layer.v1.tar"),
            Self::ImageLayerGzip => write!(f, "application/vnd.oci.image.layer.v1.tar+gzip"),
            Self::ImageLayerZstd => write!(f, "application/vnd.oci.image.layer.v1.tar+zstd"),
            Self::ImageLayerNonDistributable => {
                write!(f, "application/vnd.oci.image.layer.nondistributable.v1.tar")
            }
            Self::ImageLayerNonDistributableGzip => write!(
                f,
                "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip"
            ),
            Self::ImageLayerNonDistributableZstd => write!(
                f,
                "application/vnd.oci.image.layer.nondistributable.v1.tar+zstd"
            ),
            Self::ImageConfig => write!(f, "application/vnd.oci.image.config.v1+json"),
            Self::Other(media_type) => write!(f, "{}", media_type),
        }
    }
}

impl From<&str> for MediaType {
    fn from(media_type: &str) -> Self {
        match media_type {
            "application/vnd.oci.descriptor" => MediaType::Descriptor,
            "application/vnd.oci.layout.header.v1+json" => MediaType::LayoutHeader,
            "application/vnd.oci.image.manifest.v1+json" => MediaType::ImageManifest,
            "application/vnd.oci.image.index.v1+json" => MediaType::ImageIndex,
            "application/vnd.oci.image.layer.v1.tar" => MediaType::ImageLayer,
            "application/vnd.oci.image.layer.v1.tar+gzip" => MediaType::ImageLayerGzip,
            "application/vnd.oci.image.layer.v1.tar+zstd" => MediaType::ImageLayerZstd,
            "application/vnd.oci.image.layer.nondistributable.v1.tar" => {
                MediaType::ImageLayerNonDistributable
            }
            "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip" => {
                MediaType::ImageLayerNonDistributableGzip
            }
            "application/vnd.oci.image.layer.nondistributable.v1.tar+zstd" => {
                MediaType::ImageLayerNonDistributableZstd
            }
            "application/vnd.oci.image.config.v1+json" => MediaType::ImageConfig,
            media => MediaType::Other(media.to_owned()),
        }
    }
}

/// Trait to get the Docker Image Manifest V2 Schema 2 media type for an OCI media type
///
/// This may be necessary for compatibility with tools that do not recognize the OCI media types.
/// Where a [`MediaType`] is expected you can use `MediaType::ImageManifest.to_docker_v2s2()?` instead and
/// `impl From<&str> for MediaType` will create a [`MediaType::Other`] for it.
///
/// Not all OCI Media Types have an equivalent Docker V2S2 Media Type. In those cases, `to_docker_v2s2` will error.
pub trait ToDockerV2S2 {
    /// Get the [Docker Image Manifest V2 Schema 2](https://docs.docker.com/registry/spec/manifest-v2-2/)
    /// media type equivalent for an OCI media type
    fn to_docker_v2s2(&self) -> Result<&str, std::fmt::Error>;
}

impl ToDockerV2S2 for MediaType {
    fn to_docker_v2s2(&self) -> Result<&str, std::fmt::Error> {
        Ok(match self {
            Self::ImageIndex => "application/vnd.docker.distribution.manifest.list.v2+json",
            Self::ImageManifest => "application/vnd.docker.distribution.manifest.v2+json",
            Self::ImageConfig => "application/vnd.docker.container.image.v1+json",
            Self::ImageLayerGzip => "application/vnd.docker.image.rootfs.diff.tar.gzip",
            _ => return Err(std::fmt::Error),
        })
    }
}

impl Serialize for MediaType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let media_type = format!("{}", self);
        media_type.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MediaType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let media_type = String::deserialize(deserializer)?;
        Ok(media_type.as_str().into())
    }
}

/// Name of the target operating system.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Os {
    AIX,
    Android,
    Darwin,
    DragonFlyBSD,
    FreeBSD,
    Hurd,
    Illumos,
    #[allow(non_camel_case_types)]
    iOS,
    Js,
    Linux,
    Nacl,
    NetBSD,
    OpenBSD,
    Plan9,
    Solaris,
    Windows,
    #[allow(non_camel_case_types)]
    zOS,
    Other(String),
}

impl From<&str> for Os {
    fn from(os: &str) -> Self {
        match os {
            "aix" => Os::AIX,
            "android" => Os::Android,
            "darwin" => Os::Darwin,
            "dragonfly" => Os::DragonFlyBSD,
            "freebsd" => Os::FreeBSD,
            "hurd" => Os::Hurd,
            "illumos" => Os::Illumos,
            "ios" => Os::iOS,
            "js" => Os::Js,
            "linux" => Os::Linux,
            "nacl" => Os::Nacl,
            "netbsd" => Os::NetBSD,
            "openbsd" => Os::OpenBSD,
            "plan9" => Os::Plan9,
            "solaris" => Os::Solaris,
            "windows" => Os::Windows,
            "zos" => Os::zOS,
            name => Os::Other(name.to_owned()),
        }
    }
}

impl Display for Os {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match self {
            Os::AIX => "aix",
            Os::Android => "android",
            Os::Darwin => "darwin",
            Os::DragonFlyBSD => "dragonfly",
            Os::FreeBSD => "freebsd",
            Os::Hurd => "hurd",
            Os::Illumos => "illumos",
            Os::iOS => "ios",
            Os::Js => "js",
            Os::Linux => "linux",
            Os::Nacl => "nacl",
            Os::NetBSD => "netbsd",
            Os::OpenBSD => "openbsd",
            Os::Plan9 => "plan9",
            Os::Solaris => "solaris",
            Os::Windows => "windows",
            Os::zOS => "zos",
            Os::Other(name) => name,
        };

        write!(f, "{}", print)
    }
}

impl Serialize for Os {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let os = format!("{}", self);
        os.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Os {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let os = String::deserialize(deserializer)?;
        Ok(os.as_str().into())
    }
}

impl Default for Os {
    fn default() -> Self {
        Os::from(std::env::consts::OS)
    }
}

/// Name of the CPU target architecture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arch {
    /// 32 bit x86, little-endian
    #[allow(non_camel_case_types)]
    i386,
    /// 64 bit x86, little-endian
    Amd64,
    /// 64 bit x86 with 32 bit pointers, little-endian
    Amd64p32,
    /// 32 bit ARM, little-endian
    ARM,
    /// 32 bit ARM, big-endian
    ARMbe,
    /// 64 bit ARM, little-endian
    ARM64,
    /// 64 bit ARM, big-endian
    ARM64be,
    /// 64 bit Loongson RISC CPU, little-endian
    LoongArch64,
    /// 32 bit Mips, big-endian
    Mips,
    /// 32 bit Mips, little-endian
    Mipsle,
    /// 64 bit Mips, big-endian
    Mips64,
    /// 64 bit Mips, little-endian
    Mips64le,
    /// 64 bit Mips with 32 bit pointers, big-endian
    Mips64p32,
    /// 64 bit Mips with 32 bit pointers, little-endian
    Mips64p32le,
    /// 32 bit PowerPC, big endian
    PowerPC,
    /// 64 bit PowerPC, big-endian
    PowerPC64,
    /// 64 bit PowerPC, little-endian
    PowerPC64le,
    /// 32 bit RISC-V, little-endian
    RISCV,
    /// 64 bit RISC-V, little-endian
    RISCV64,
    /// 32 bit IBM System/390, big-endian
    #[allow(non_camel_case_types)]
    s390,
    /// 64 bit IBM System/390, big-endian
    #[allow(non_camel_case_types)]
    s390x,
    /// 32 bit SPARC, big-endian
    SPARC,
    /// 64 bit SPARC, bi-endian
    SPARC64,
    /// 32 bit Web Assembly
    Wasm,
    /// Architecture not specified by OCI image format
    Other(String),
}

impl Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let print = match self {
            Arch::i386 => "386",
            Arch::Amd64 => "amd64",
            Arch::Amd64p32 => "amd64p32",
            Arch::ARM => "arm",
            Arch::ARMbe => "armbe",
            Arch::ARM64 => "arm64",
            Arch::ARM64be => "arm64be",
            Arch::LoongArch64 => "loong64",
            Arch::Mips => "mips",
            Arch::Mipsle => "mipsle",
            Arch::Mips64 => "mips64",
            Arch::Mips64le => "mips64le",
            Arch::Mips64p32 => "mips64p32",
            Arch::Mips64p32le => "mips64p32le",
            Arch::PowerPC => "ppc",
            Arch::PowerPC64 => "ppc64",
            Arch::PowerPC64le => "ppc64le",
            Arch::RISCV => "riscv",
            Arch::RISCV64 => "riscv64",
            Arch::s390 => "s390",
            Arch::s390x => "s390x",
            Arch::SPARC => "sparc",
            Arch::SPARC64 => "sparc64",
            Arch::Wasm => "wasm",
            Arch::Other(arch) => arch,
        };

        write!(f, "{}", print)
    }
}

impl From<&str> for Arch {
    fn from(arch: &str) -> Self {
        match arch {
            "386" => Arch::i386,
            "amd64" => Arch::Amd64,
            "amd64p32" => Arch::Amd64p32,
            "arm" => Arch::ARM,
            "armbe" => Arch::ARM64be,
            "arm64" => Arch::ARM64,
            "arm64be" => Arch::ARM64be,
            "loong64" => Arch::LoongArch64,
            "mips" => Arch::Mips,
            "mipsle" => Arch::Mipsle,
            "mips64" => Arch::Mips64,
            "mips64le" => Arch::Mips64le,
            "mips64p32" => Arch::Mips64p32,
            "mips64p32le" => Arch::Mips64p32le,
            "ppc" => Arch::PowerPC,
            "ppc64" => Arch::PowerPC64,
            "ppc64le" => Arch::PowerPC64le,
            "riscv" => Arch::RISCV,
            "riscv64" => Arch::RISCV64,
            "s390" => Arch::s390,
            "s390x" => Arch::s390x,
            "sparc" => Arch::SPARC,
            "sparc64" => Arch::SPARC64,
            "wasm" => Arch::Wasm,
            arch => Arch::Other(arch.to_owned()),
        }
    }
}

impl Serialize for Arch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let arch = format!("{}", self);
        arch.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Arch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let arch = String::deserialize(deserializer)?;
        Ok(arch.as_str().into())
    }
}

impl Default for Arch {
    fn default() -> Self {
        // Translate from the Rust architecture names to the Go versions.
        // It seems like the Rust ones are the same GNU/Linux...except for `powerpc64` and not `ppc64le`?
        // This list just contains exceptions, everything else is passed through literally.
        // See also https://github.com/containerd/containerd/blob/140ecc9247386d3be21616fe285021c081f4ea08/platforms/database.go
        let goarch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "powerpc64" if cfg!(target_endian = "big") => "ppc64",
            "powerpc64" if cfg!(target_endian = "little") => "ppc64le",
            o => o,
        };
        Arch::from(goarch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_translation() {
        let a = Arch::default();
        match a {
            // If you hit this, please update the mapping above.
            Arch::Other(o) => panic!("Architecture {} not mapped between Rust and OCI", o),
            _ => {}
        }
    }
}
