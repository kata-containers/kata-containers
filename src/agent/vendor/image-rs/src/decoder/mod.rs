// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::convert::TryFrom;
use std::fmt;
use std::io;

use anyhow::{bail, Result};
use oci_distribution::manifest;
use oci_spec::image::MediaType;
use serde::Deserialize;
use tokio::io::{AsyncRead, BufReader};

/// Error message for unhandled media type.
pub const ERR_BAD_MEDIA_TYPE: &str = "unhandled media type";

/// Represents the layer compression algorithm type,
/// and allows to decompress corresponding compressed data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Compression {
    Uncompressed,
    #[default]
    Gzip,
    Zstd,
}

impl fmt::Display for Compression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self {
            Compression::Uncompressed => "uncompressed",
            Compression::Gzip => "gzip",
            Compression::Zstd => "zstd",
        };

        write!(f, "{output}")
    }
}

impl Compression {
    /// Decompress input data from one `Read` and output data to one `Write`.
    /// Uncompressed data are not supported and an error will be returned.
    pub fn decompress<R, W>(&self, input: R, output: &mut W) -> io::Result<()>
    where
        R: io::Read,
        W: io::Write,
    {
        match self {
            Self::Gzip => gzip_decode(input, output),
            Self::Zstd => zstd_decode(input, output),
            Self::Uncompressed => Err(io::Error::new(
                io::ErrorKind::Other,
                "uncompressed input data".to_string(),
            )),
        }
    }

    /// Create an `AsyncRead` to decode input stream.
    pub fn async_decompress<'a>(
        &self,
        input: (impl AsyncRead + Unpin + 'a + Send),
    ) -> Box<dyn AsyncRead + Unpin + 'a + Send> {
        match self {
            Self::Gzip => Box::new(async_compression::tokio::bufread::GzipDecoder::new(
                BufReader::new(input),
            )),
            Self::Zstd => Box::new(async_compression::tokio::bufread::ZstdDecoder::new(
                BufReader::new(input),
            )),
            Self::Uncompressed => Box::new(input),
        }
    }

    /// Create an `AsyncRead` to decode input gzip stream.
    pub fn async_gzip_decompress(input: (impl AsyncRead + Unpin)) -> impl AsyncRead + Unpin {
        async_compression::tokio::bufread::GzipDecoder::new(BufReader::new(input))
    }

    /// Create an `AsyncRead` to decode input zstd stream.
    pub fn async_zstd_decompress(input: (impl AsyncRead + Unpin)) -> impl AsyncRead + Unpin {
        async_compression::tokio::bufread::ZstdDecoder::new(BufReader::new(input))
    }
}

// Decompress a gzip encoded data with flate2 crate.
fn gzip_decode<R, W>(input: R, output: &mut W) -> std::io::Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut decoder = flate2::read::GzDecoder::new(input);
    io::copy(&mut decoder, output)?;
    Ok(())
}

// Decompress a zstd encoded data with zstd crate.
fn zstd_decode<R, W>(input: R, output: &mut W) -> std::io::Result<()>
where
    R: io::Read,
    W: io::Write,
{
    let mut decoder = zstd::Decoder::new(input)?;
    io::copy(&mut decoder, output)?;
    Ok(())
}

impl TryFrom<&str> for Compression {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut media_type_str = s;

        // convert docker layer media type to oci format
        if media_type_str == manifest::IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE {
            media_type_str = manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE;
        }

        let media_type = MediaType::from(media_type_str);

        let decoder = match media_type {
            MediaType::ImageLayer | MediaType::ImageLayerNonDistributable => {
                Compression::Uncompressed
            }
            MediaType::ImageLayerGzip | MediaType::ImageLayerNonDistributableGzip => {
                Compression::Gzip
            }
            MediaType::ImageLayerZstd | MediaType::ImageLayerNonDistributableZstd => {
                Compression::Zstd
            }
            _ => bail!("{}: {}", ERR_BAD_MEDIA_TYPE, media_type),
        };

        Ok(decoder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use flate2::write::GzEncoder;
    use std::io::Write;

    use test_utils::assert_result;

    #[test]
    fn test_uncompressed_decode() {
        let bytes = Vec::new();
        let mut output = Vec::new();
        let compression = Compression::Uncompressed;
        assert!(compression
            .decompress(bytes.as_slice(), &mut output)
            .is_err());
    }

    #[test]
    fn test_gzip_decode() {
        let data: Vec<u8> = b"This is some text!".to_vec();

        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&data).unwrap();
        let bytes = encoder.finish().unwrap();

        let mut output = Vec::new();

        let compression = Compression::Uncompressed;
        assert!(compression
            .decompress(bytes.as_slice(), &mut output)
            .is_err());

        let compression = Compression::default();
        assert!(compression
            .decompress(bytes.as_slice(), &mut output)
            .is_ok());
        assert_eq!(data, output);
    }

    #[test]
    fn test_zstd_decode() {
        let data: Vec<u8> = b"This is some text!".to_vec();
        let level = 1;

        let bytes = zstd::encode_all(&data[..], level).unwrap();

        let mut output = Vec::new();
        let compression = Compression::Zstd;
        assert!(compression
            .decompress(bytes.as_slice(), &mut output)
            .is_ok());
        assert_eq!(data, output);
    }

    #[tokio::test]
    async fn test_async_gzip_decode() {
        let data: Vec<u8> = b"This is some text!".to_vec();
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&data).unwrap();
        let bytes = encoder.finish().unwrap();

        let mut output = Vec::new();
        let mut reader = Compression::Gzip.async_decompress(bytes.as_slice());
        assert!(
            tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut output)
                .await
                .is_ok()
        );
        assert_eq!(data, output);

        let mut output = Vec::new();
        let mut reader = Compression::async_gzip_decompress(bytes.as_slice());
        assert!(
            tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut output)
                .await
                .is_ok()
        );
        assert_eq!(data, output);
    }

    #[tokio::test]
    async fn test_async_zstd_decode() {
        let data: Vec<u8> = b"This is some text!".to_vec();
        let level = 1;
        let bytes = zstd::encode_all(&data[..], level).unwrap();

        let mut output = Vec::new();
        let mut reader = Compression::Zstd.async_decompress(bytes.as_slice());
        assert!(
            tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut output)
                .await
                .is_ok()
        );
        assert_eq!(data, output);

        let mut output = Vec::new();
        let mut reader = Compression::async_zstd_decompress(bytes.as_slice());
        assert!(
            tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut output)
                .await
                .is_ok()
        );
        assert_eq!(data, output);
    }

    #[tokio::test]
    async fn test_try_from_compression() {
        #[derive(Debug)]
        struct TestData<'a> {
            media_type_str: &'a str,
            result: Result<Compression>,
        }

        let tests = &[
            TestData {
                media_type_str: "",
                result: Err(anyhow!("{}: {}", ERR_BAD_MEDIA_TYPE, "")),
            },
            TestData {
                media_type_str: "foo",
                result: Err(anyhow!("{}: {}", ERR_BAD_MEDIA_TYPE, "foo")),
            },
            TestData {
                media_type_str: "foo/ bar",
                result: Err(anyhow!("{}: {}", ERR_BAD_MEDIA_TYPE, "foo/ bar")),
            },
            TestData {
                media_type_str: manifest::IMAGE_LAYER_MEDIA_TYPE,
                result: Ok(Compression::Uncompressed),
            },
            TestData {
                media_type_str: "application/vnd.oci.image.layer.nondistributable.v1.tar",
                result: Ok(Compression::Uncompressed),
            },
            TestData {
                media_type_str: manifest::IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE,
                result: Ok(Compression::Gzip),
            },
            TestData {
                media_type_str: manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE,
                result: Ok(Compression::Gzip),
            },
            TestData {
                media_type_str: "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip",
                result: Ok(Compression::Gzip),
            },
            TestData {
                media_type_str: "application/vnd.oci.image.layer.v1.tar+gzip",
                result: Ok(Compression::Gzip),
            },
            TestData {
                media_type_str: "application/vnd.oci.image.layer.v1.tar+zstd",
                result: Ok(Compression::Zstd),
            },
            TestData {
                media_type_str: "application/vnd.oci.image.layer.nondistributable.v1.tar+zstd",
                result: Ok(Compression::Zstd),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = Compression::try_from(d.media_type_str);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }
}
