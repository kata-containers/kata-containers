// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, Result};
use oci_distribution::manifest::{self, OciDescriptor};
use tokio::io::AsyncRead;

/// Image layer encryption type information and associated methods to decrypt image layers.
#[derive(Default, Clone, Debug)]
pub struct Decryptor {
    /// The layer original media type before encryption.
    pub media_type: String,

    /// Whether layer is encrypted.
    encrypted: bool,
}

impl Decryptor {
    /// Check whether media_type is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.encrypted
    }
}

#[cfg(feature = "encryption")]
mod encryption {
    use super::*;
    use anyhow::anyhow;
    use ocicrypt_rs::config::CryptoConfig;
    use ocicrypt_rs::encryption::{
        async_decrypt_layer, decrypt_layer, decrypt_layer_key_opts_data,
    };
    use ocicrypt_rs::helpers::create_decrypt_config;
    use ocicrypt_rs::spec::{
        MEDIA_TYPE_LAYER_ENC, MEDIA_TYPE_LAYER_GZIP_ENC, MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_ENC,
        MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_GZIP_ENC,
    };
    use std::io::Read;

    impl Decryptor {
        const ERR_EMPTY_CFG: &'static str = "decrypt_config is empty";
        const ERR_UNENCRYPTED_MEDIA_TYPE: &'static str = "unencrypted media type";

        /// Construct Decryptor from media_type.
        pub fn from_media_type(media_type: &str) -> Self {
            let (media_type, encrypted) = match media_type {
                MEDIA_TYPE_LAYER_ENC | MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_ENC => {
                    (manifest::IMAGE_LAYER_MEDIA_TYPE.to_string(), true)
                }
                MEDIA_TYPE_LAYER_GZIP_ENC | MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_GZIP_ENC => {
                    (manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE.to_string(), true)
                }
                _ => ("".to_string(), false),
            };

            Decryptor {
                media_type,
                encrypted,
            }
        }

        /// get_plaintext_layer decrypts encrypted_layer data and return the plaintext_layer data.
        ///
        /// `descriptor` and `decrypt_config` are required for layer data decryption process.
        ///
        /// * `decrypt_config` - decryption key info in following format:\
        ///           - \<filename> \
        ///           - \<filename>:file=\<passwordfile> \
        ///           - \<filename>:pass=\<password> \
        ///           - \<filename>:fd=\<filedescriptor> \
        ///           - \<filename>:\<password> \
        ///           - provider:<cmd/gprc>
        pub fn get_plaintext_layer(
            &self,
            descriptor: &OciDescriptor,
            encrypted_layer: Vec<u8>,
            decrypt_config: &str,
        ) -> Result<Vec<u8>> {
            if !self.is_encrypted() {
                bail!("{}: {}", Self::ERR_UNENCRYPTED_MEDIA_TYPE, self.media_type);
            }
            if decrypt_config.is_empty() {
                bail!(Self::ERR_EMPTY_CFG);
            }

            let cc = create_decrypt_config(vec![decrypt_config.to_string()], vec![])?;
            decrypt_layer_data(&encrypted_layer, descriptor, &cc)
                .map(|(decrypted_data, _)| decrypted_data)
        }

        /// Get decryption key to decrypt an encrypted image layer.
        pub fn get_decrypt_key(
            &self,
            descriptor: &OciDescriptor,
            decrypt_config: &str,
        ) -> Result<Vec<u8>> {
            if !self.is_encrypted() {
                bail!("unencrypted media type: {}", self.media_type);
            }
            if decrypt_config.is_empty() {
                bail!("decrypt_config is empty");
            }

            let cc = create_decrypt_config(vec![decrypt_config.to_string()], vec![])?;
            if let Some(decrypt_config) = cc.decrypt_config {
                decrypt_layer_key_opts_data(&decrypt_config, descriptor.annotations.as_ref())
            } else {
                Err(anyhow!("failed to retrieve decrypt key!"))
            }
        }

        pub fn async_get_plaintext_layer(
            &self,
            encrypted_layer: impl AsyncRead + Send,
            descriptor: &OciDescriptor,
            priv_opts_data: &[u8],
        ) -> Result<impl AsyncRead + Send> {
            let (layer_decryptor, _dec_digest) = async_decrypt_layer(
                encrypted_layer,
                descriptor.annotations.as_ref(),
                priv_opts_data,
            )
            .map_err(|e| anyhow!("failed to async decrypt layer {}", e.to_string()))?;
            Ok(layer_decryptor)
        }
    }

    fn decrypt_layer_data(
        encrypted_layer: &[u8],
        descriptor: &OciDescriptor,
        crypto_config: &CryptoConfig,
    ) -> Result<(Vec<u8>, String)> {
        if let Some(decrypt_config) = &crypto_config.decrypt_config {
            let (layer_decryptor, dec_digest) = decrypt_layer(
                decrypt_config,
                encrypted_layer,
                descriptor.annotations.as_ref(),
                false,
            )?;
            let mut plaintext_data: Vec<u8> = Vec::new();
            let mut decryptor =
                layer_decryptor.ok_or_else(|| anyhow!("missing layer decryptor"))?;

            decryptor.read_to_end(&mut plaintext_data)?;

            Ok((plaintext_data, dec_digest))
        } else {
            Err(anyhow!("no decrypt config available"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_from_media_type() {
            #[derive(Debug)]
            struct TestData<'a> {
                media_type: &'a str,
                result: Decryptor,
            }

            let tests = &[
                TestData {
                    media_type: "",
                    result: Decryptor {
                        media_type: "".into(),
                        encrypted: false,
                    },
                },
                TestData {
                    media_type: "invalid",
                    result: Decryptor {
                        media_type: "".into(),
                        encrypted: false,
                    },
                },
                TestData {
                    media_type: "foo bar",
                    result: Decryptor {
                        media_type: "".into(),
                        encrypted: false,
                    },
                },
                TestData {
                    media_type: MEDIA_TYPE_LAYER_ENC,
                    result: Decryptor {
                        media_type: manifest::IMAGE_LAYER_MEDIA_TYPE.to_string(),
                        encrypted: true,
                    },
                },
                TestData {
                    media_type: MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_ENC,
                    result: Decryptor {
                        media_type: manifest::IMAGE_LAYER_MEDIA_TYPE.to_string(),
                        encrypted: true,
                    },
                },
                TestData {
                    media_type: MEDIA_TYPE_LAYER_GZIP_ENC,
                    result: Decryptor {
                        media_type: manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE.to_string(),
                        encrypted: true,
                    },
                },
                TestData {
                    media_type: MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_GZIP_ENC,
                    result: Decryptor {
                        media_type: manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE.to_string(),
                        encrypted: true,
                    },
                },
            ];

            for (i, d) in tests.iter().enumerate() {
                let msg = format!("test[{}]: {:?}", i, d);

                let result = Decryptor::from_media_type(d.media_type);

                let msg = format!("{}: result: {:?}", msg, result);

                assert_eq!(result.media_type, d.result.media_type, "{:?}", msg);
                assert_eq!(result.encrypted, d.result.encrypted, "{:?}", msg);
            }
        }

        #[tokio::test]
        async fn test_get_plaintext_layer() {
            use std::io::Write;

            #[derive(Debug)]
            struct TestData<'a> {
                encrypted: bool,
                media_type: &'a str,
                descriptor: OciDescriptor,
                encrypted_layer: Vec<u8>,
                decrypt_config: &'a str,
                result: Result<Vec<u8>>,
            }

            let tests = &[
                TestData {
                    encrypted: false,
                    media_type: "",
                    descriptor: OciDescriptor::default(),
                    encrypted_layer: Vec::<u8>::new(),
                    decrypt_config: "",
                    result: Err(anyhow!("{}: {}", Decryptor::ERR_UNENCRYPTED_MEDIA_TYPE, "")),
                },
                TestData {
                    encrypted: false,
                    media_type: "",
                    descriptor: OciDescriptor::default(),
                    encrypted_layer: Vec::<u8>::new(),
                    decrypt_config: "foo",
                    result: Err(anyhow!("{}: {}", Decryptor::ERR_UNENCRYPTED_MEDIA_TYPE, "")),
                },
                TestData {
                    encrypted: true,
                    media_type: "",
                    descriptor: OciDescriptor::default(),
                    encrypted_layer: Vec::<u8>::new(),
                    decrypt_config: "provider:grpc",
                    result: Err(anyhow!("missing private key needed for decryption")),
                },
                TestData {
                    encrypted: true,
                    media_type: MEDIA_TYPE_LAYER_ENC,
                    descriptor: OciDescriptor::default(),
                    encrypted_layer: Vec::<u8>::new(),
                    decrypt_config: "provider:grpc",
                    result: Err(anyhow!("missing private key needed for decryption")),
                },
            ];

            let tempdir = tempfile::tempdir().unwrap();

            let keyprovider_config_path = tempdir.path().join("keyconfig");
            let mut keyprovider_config =
                std::fs::File::create(keyprovider_config_path.clone()).unwrap();

            let data = r#"
        {
            "key-providers": {
                "keyprovider1": {
                    "cmd": {
                        "path": "/bin/true",
                        "args": []
                    }
                }
            }
        }
        "#;

            keyprovider_config.write_all(data.as_bytes()).unwrap();

            std::env::set_var(
                ocicrypt_rs::config::OCICRYPT_ENVVARNAME,
                keyprovider_config_path,
            );

            for (i, d) in tests.iter().enumerate() {
                let msg = format!("test[{}]: {:?}", i, d);

                let decryptor = Decryptor {
                    media_type: d.media_type.to_string(),
                    encrypted: d.encrypted,
                };

                let result = decryptor.get_plaintext_layer(
                    &d.descriptor,
                    d.encrypted_layer.clone(),
                    d.decrypt_config,
                );
                let msg = format!("{}: result: {:?}", msg, result);

                test_utils::assert_result!(d.result, result, msg);
            }
        }
    }
}

#[cfg(not(feature = "encryption"))]
impl Decryptor {
    /// Construct Decryptor from media_type.
    pub fn from_media_type(media_type: &str) -> Self {
        let (media_type, encrypted) = match media_type {
            "application/vnd.oci.image.layer.v1.tar+encrypted"
            | "application/vnd.oci.image.layer.nondistributable.v1.tar+encrypted" => {
                (manifest::IMAGE_LAYER_MEDIA_TYPE.to_string(), true)
            }
            "application/vnd.oci.image.layer.v1.tar+gzip+encrypted"
            | "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip+encrypted" => {
                (manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE.to_string(), true)
            }
            _ => ("".to_string(), false),
        };

        Decryptor {
            media_type,
            encrypted,
        }
    }

    pub fn get_plaintext_layer(
        &self,
        _descriptor: &OciDescriptor,
        _encrypted_layer: Vec<u8>,
        _decrypt_config: &str,
    ) -> Result<Vec<u8>> {
        bail!(
            "no support of encryption, can't handle '{}'",
            self.media_type
        );
    }

    pub fn get_decrypt_key(
        &self,
        _descriptor: &OciDescriptor,
        _decrypt_config: &str,
    ) -> Result<Vec<u8>> {
        bail!(
            "no support of encryption, can't handle '{}'",
            self.media_type
        );
    }

    pub fn async_get_plaintext_layer(
        &self,
        encrypted_layer: impl AsyncRead,
        _descriptor: &OciDescriptor,
        _priv_opts_data: &[u8],
    ) -> Result<impl AsyncRead> {
        if self.is_encrypted() {
            bail!(
                "no support of encryption, can't handle '{}'",
                self.media_type
            );
        } else {
            Ok(encrypted_layer)
        }
    }
}
