// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Fetch confidential resources from KBS (Relying Party).

use std::collections::HashMap;

#[cfg(not(features = "keywrap-native"))]
use anyhow::Context;
use anyhow::{bail, Result};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[cfg(feature = "keywrap-grpc")]
mod grpc;

#[cfg(feature = "keywrap-ttrpc")]
mod ttrpc;

#[cfg(feature = "keywrap-ttrpc")]
mod ttrpc_proto;

#[cfg(feature = "keywrap-native")]
mod native;

#[cfg(any(
    not(any(
        feature = "keywrap-grpc",
        feature = "keywrap-ttrpc",
        feature = "keywrap-native"
    )),
    all(
        feature = "keywrap-grpc",
        any(feature = "keywrap-ttrpc", feature = "keywrap-native")
    ),
    all(
        feature = "keywrap-ttrpc",
        any(feature = "keywrap-grpc", feature = "keywrap-native")
    ),
    all(
        feature = "keywrap-native",
        any(feature = "keywrap-grpc", feature = "keywrap-ttrpc")
    ),
))]
compile_error!("One and exactly one feature of `keywrap-grpc`, `keywrap-ttrpc`, and `keywrap-native` must be enabled.");

/// The resource description that will be passed to AA when get resource.
#[derive(Serialize, Deserialize, Debug)]
struct ResourceDescription {
    name: String,
    optional: HashMap<String, String>,
}

impl ResourceDescription {
    /// Create a new ResourceDescription with resource name.
    pub fn new(name: &str, optional: HashMap<String, String>) -> Self {
        ResourceDescription {
            name: name.to_string(),
            optional,
        }
    }
}

/// SecureChannel to connect with KBS
pub struct SecureChannel {
    /// Get Resource Service client.
    client: Box<dyn Client>,
    kbc_name: String,
    kbs_uri: String,
}

#[async_trait]
pub trait Client: Send + Sync {
    async fn get_resource(
        &mut self,
        kbc_name: &str,
        kbs_uri: &str,
        resource_description: String,
    ) -> Result<Vec<u8>>;
}

impl SecureChannel {
    /// Create a new [`SecureChannel`], the input parameter:
    /// * `aa_kbc_params`: s string with format `<kbc_name>::<kbs_uri>`.
    pub async fn new(aa_kbc_params: &str) -> Result<Self> {
        // unzip here is unstable
        if let Some((kbc_name, kbs_uri)) = aa_kbc_params.split_once("::") {
            if kbc_name.is_empty() {
                bail!("aa_kbc_params: missing KBC name");
            }

            if kbs_uri.is_empty() {
                bail!("aa_kbc_params: missing KBS URI");
            }

            let client: Box<dyn Client> = {
                #[cfg(feature = "keywrap-grpc")]
                {
                    Box::new(grpc::Grpc::new().await.context("grpc client init failed")?)
                }

                #[cfg(feature = "keywrap-ttrpc")]
                {
                    Box::new(ttrpc::Ttrpc::new().context("ttrpc client init failed")?)
                }

                #[cfg(feature = "keywrap-native")]
                {
                    Box::new(native::Native::default())
                }
            };

            Ok(Self {
                client,
                kbc_name: kbc_name.into(),
                kbs_uri: kbs_uri.into(),
            })
        } else {
            bail!("aa_kbc_params: KBC/KBS pair not found")
        }
    }

    /// Get resource from using, using `resource_name` as `name` in a ResourceDescription,
    /// then save the gathered data into `path`
    ///
    /// Please refer to https://github.com/confidential-containers/image-rs/blob/main/docs/ccv1_image_security_design.md#get-resource-service
    /// for more information.
    pub async fn get_resource(
        &mut self,
        resource_name: &str,
        optional: HashMap<String, String>,
        path: &str,
    ) -> Result<()> {
        let resource_description =
            serde_json::to_string(&ResourceDescription::new(resource_name, optional))?;
        let res = self
            .client
            .get_resource(&self.kbc_name, &self.kbs_uri, resource_description)
            .await?;
        fs::write(path, res).await?;
        Ok(())
    }
}
