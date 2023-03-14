// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! # Signing schemes
//! different signatures defination and the top level interfaces.
//!
//! ### Design
//! Due to the format of policy requirement in
//! <https://github.com/containers/image/blob/main/docs/containers-policy.json.5.md#policy-requirements>,
//! a signing scheme is also treated as a policy
//! requirement. To support different kinds of signing
//! schemes, we use a trait [`SignScheme`] to define. The trait object
//! will be included into [`crate::policy::PolicyReqType`].

use std::collections::HashMap;

use anyhow::*;
use async_trait::async_trait;
use oci_distribution::secrets::RegistryAuth;

use crate::config::Paths;

use super::image::Image;

#[cfg(feature = "signature-simple")]
pub mod simple;

#[cfg(feature = "signature-cosign")]
pub mod cosign;

/// The interface of a signing scheme
#[async_trait]
pub trait SignScheme: Send + Sync {
    /// Do initialization jobs for this scheme. This may include the following
    /// * preparing runtime directories for storing signatures, configurations, etc.
    /// * gathering necessary files.
    async fn init(&mut self, config: &Paths) -> Result<()>;

    /// Reture a HashMap including a resource's name => file path in fs.
    ///
    /// Here `resource's name` is the `name` field for a ResourceDescription
    /// in GetResourceRequest.
    /// Please refer to <https://github.com/confidential-containers/image-rs/blob/main/docs/ccv1_image_security_design.md#get-resource-service>
    /// for more information about the `GetResourceRequest`.
    ///
    /// This function will be called by `Agent`, to get the manifest
    /// of all the resources to be gathered from kbs. The gathering
    /// operation will happen after `init_scheme()`, to prepare necessary
    /// resources. The HashMap here uses &str rather than String,
    /// which encourages developer of new signing schemes to define
    /// const &str for these information.
    fn resource_manifest(&self) -> HashMap<&str, &str>;

    /// Judge whether an image is allowed by this SignScheme.
    async fn allows_image(&self, image: &mut Image, auth: &RegistryAuth) -> Result<()>;
}
