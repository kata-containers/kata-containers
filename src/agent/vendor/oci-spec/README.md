# oci-spec-rs

[![ci](https://github.com/containers/oci-spec-rs/workflows/ci/badge.svg)](https://github.com/containers/oci-spec-rs/actions)
[![gh-pages](https://github.com/containers/oci-spec-rs/workflows/gh-pages/badge.svg)](https://github.com/containers/oci-spec-rs/actions)
[![crates.io](https://img.shields.io/crates/v/oci-spec.svg)](https://crates.io/crates/oci-spec)
[![codecov](https://codecov.io/gh/containers/oci-spec-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/containers/oci-spec-rs)
[![docs](https://img.shields.io/badge/docs-main-blue.svg)](https://containers.github.io/oci-spec-rs/oci_spec/index.html)
[![docs.rs](https://docs.rs/oci-spec/badge.svg)](https://docs.rs/oci-spec)
[![dependencies](https://deps.rs/repo/github/containers/oci-spec-rs/status.svg)](https://deps.rs/repo/github/containers/oci-spec-rs)
[![license](https://img.shields.io/github/license/containers/oci-spec-rs.svg)](https://github.com/containers/oci-spec-rs/blob/master/LICENSE)

### Open Container Initiative (OCI) Specifications for Rust

This library provides a convenient way to interact with the specifications defined by the [Open Container Initiative (OCI)](https://opencontainers.org). 

- [Image Format Specification](https://github.com/opencontainers/image-spec/blob/main/spec.md)
- [Runtime Specification](https://github.com/opencontainers/runtime-spec/blob/master/spec.md)
- [Distribution Specification](https://github.com/opencontainers/distribution-spec/blob/main/spec.md)

```toml
[dependencies]
oci-spec = "0.5.8"
```
*Compiler support: requires rustc 1.54+*

If you want to propose or cut a new release, then please follow our 
[release process documentation](./release.md).

## Image Format Spec Examples
- Load image manifest from filesystem
```rust no_run
use oci_spec::image::ImageManifest;

let image_manifest = ImageManifest::from_file("manifest.json").unwrap();
assert_eq!(image_manifest.layers().len(), 5);
```

- Create new image manifest using builder
```rust no_run
use oci_spec::image::{
    Descriptor, 
    DescriptorBuilder, 
    ImageManifest, 
    ImageManifestBuilder, 
    MediaType, 
    SCHEMA_VERSION
};

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

let image_manifest = ImageManifestBuilder::default()
    .schema_version(SCHEMA_VERSION)
    .config(config)
    .layers(layers)
    .build()
    .expect("build image manifest");

image_manifest.to_file_pretty("my-manifest.json").unwrap();
```

- Content of my-manifest.json
```json
{
  "schemaVersion": 2,
  "config": {
    "mediaType": "application/vnd.oci.image.config.v1+json",
    "digest": "sha256:b5b2b2c507a0944348e0303114d8d93aaaa081732b86451d9bce1f432a537bc7",
    "size": 7023
  },
  "layers": [
    {
      "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
      "digest": "sha256:9834876dcfb05cb167a5c24953eba58c4ac89b1adf57f28f2f9d09af107ee8f0",
      "size": 32654
    },
    {
      "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
      "digest": "sha256:3c3a4604a545cdc127456d94e421cd355bca5b528f4a9c1905b15da2eb4a4c6b",
      "size": 16724
    },
    {
      "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip",
      "digest": "sha256:ec4b8955958665577945c89419d1af06b5f7636b4ac3da7f12184802ad867736",
      "size": 73109
    }
  ]
}
```

## Distribution Spec Examples
- Create a list of repositories 
```rust
use oci_spec::distribution::RepositoryListBuilder;

let list = RepositoryListBuilder::default()
            .repositories(vec!["busybox".to_owned()])
            .build().unwrap();
```

# Contributing
This project welcomes your PRs and issues. Should you wish to work on an issue, please claim it first by commenting on the 
issue that you want to work on it. This is to prevent duplicated efforts from contributers on the same issue.
