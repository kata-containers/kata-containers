// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

#[macro_use]
extern crate serde;
#[macro_use]
extern crate lazy_static;

use crate::keywrap::KeyWrapper;
use anyhow::{anyhow, Result};
use std::collections::HashMap;

pub mod config;
pub mod helpers;
pub mod keywrap;
pub mod spec;
pub mod utils;

#[cfg(feature = "block-cipher")]
pub mod blockcipher;
#[cfg(feature = "block-cipher")]
pub mod encryption;

lazy_static! {
    pub static ref KEY_WRAPPERS: HashMap<String, Box<dyn KeyWrapper>> = {
        #[allow(unused_mut)]
        let mut m = HashMap::new();

        #[cfg(feature = "keywrap-jwe")]
        {
            m.insert(
                "jwe".to_string(),
                Box::new(crate::keywrap::jwe::JweKeyWrapper {}) as Box<dyn KeyWrapper>,
            );
        }

        #[cfg(feature = "keywrap-keyprovider")]
        {
            let ocicrypt_config =
                crate::config::OcicryptConfig::from_env(crate::config::OCICRYPT_ENVVARNAME)
                    .expect("Unable to read ocicrypt config file");
            if let Some(ocicrypt_config) = ocicrypt_config {
                let key_providers = ocicrypt_config.key_providers;
                for (provider_name, attrs) in key_providers.iter() {
                    let key_wrapper =
                        Box::new(crate::keywrap::keyprovider::KeyProviderKeyWrapper::new(
                            provider_name.to_string(),
                            attrs.clone(),
                            None,
                        )) as Box<dyn KeyWrapper>;
                    m.insert("provider.".to_owned() + provider_name, key_wrapper);
                }
            }
        }

        m
    };
    static ref KEY_WRAPPERS_ANNOTATIONS: HashMap<String, String> = {
        let mut m = HashMap::new();
        for (scheme, key_wrapper) in KEY_WRAPPERS.iter() {
            m.insert(key_wrapper.annotation_id().to_string(), scheme.clone());
        }
        m
    };
}

/// get_key_wrapper looks up the encryptor interface given an encryption scheme (gpg, jwe)
#[allow(clippy::borrowed_box)]
pub fn get_key_wrapper(scheme: &str) -> Result<&Box<dyn KeyWrapper>> {
    KEY_WRAPPERS
        .get(scheme)
        .ok_or_else(|| anyhow!("key wrapper not supported!"))
}

/// get_wrapped_keys_map returns a option contains map of wrapped_keys
/// as values and the encryption scheme(s) as the key(s)
pub fn get_wrapped_keys_map(annotations: &HashMap<String, String>) -> HashMap<String, String> {
    let mut wrapped_keys_map = HashMap::new();

    for (annotations_id, scheme) in KEY_WRAPPERS_ANNOTATIONS.iter() {
        if let Some(value) = annotations.get(annotations_id) {
            wrapped_keys_map.insert(scheme.clone(), value.clone());
        }
    }

    wrapped_keys_map
}
