// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod image;
pub mod mechanism;
pub mod payload;
pub mod policy;

#[cfg(feature = "getresource")]
pub use getresource::allows_image;

#[cfg(not(feature = "getresource"))]
pub use no_getresource::allows_image;

#[cfg(feature = "getresource")]
pub mod getresource {
    use crate::config::Paths;
    use crate::secure_channel::SecureChannel;
    use crate::signature::policy::Policy;

    use super::image::Image;

    use std::convert::TryFrom;
    use std::sync::Arc;

    use anyhow::Result;
    use oci_distribution::secrets::RegistryAuth;
    use tokio::sync::Mutex;

    /// `allows_image` will check all the `PolicyRequirements` suitable for
    /// the given image. The `PolicyRequirements` is defined in
    /// [`policy_path`] and may include signature verification.
    #[cfg(all(feature = "getresource", feature = "signature"))]
    pub async fn allows_image(
        image_reference: &str,
        image_digest: &str,
        secure_channel: Arc<Mutex<SecureChannel>>,
        auth: &RegistryAuth,
        paths: &Paths,
    ) -> Result<()> {
        // if Policy config file does not exist, get if from KBS.
        if !std::path::Path::new(&paths.policy_path).exists() {
            secure_channel
                .lock()
                .await
                .get_resource(
                    "Policy",
                    std::collections::HashMap::new(),
                    &paths.policy_path,
                )
                .await?;
        }

        let reference = oci_distribution::Reference::try_from(image_reference)?;
        let mut image = Image::default_with_reference(reference);
        image.set_manifest_digest(image_digest)?;

        // Read the set of signature schemes that need to be verified
        // of the image from the policy configuration.
        let mut policy = Policy::from_file(&paths.policy_path).await?;
        let schemes = policy.signature_schemes(&image);

        // Get the necessary resources from KBS if needed.
        for scheme in schemes {
            scheme.init(paths).await?;
            let resource_manifest = scheme.resource_manifest();
            for (resource_name, path) in resource_manifest {
                secure_channel
                    .lock()
                    .await
                    .get_resource(resource_name, std::collections::HashMap::new(), path)
                    .await?;
            }
        }

        policy
            .is_image_allowed(image, auth)
            .await
            .map_err(|e| anyhow::anyhow!("Validate image failed: {:?}", e))
    }
}

#[cfg(not(feature = "getresource"))]
pub mod no_getresource {
    use std::convert::TryFrom;

    use anyhow::Result;
    use log::warn;
    use oci_distribution::secrets::RegistryAuth;

    use crate::{
        config::Paths,
        signature::{image::Image, policy::Policy},
    };

    pub async fn allows_image(
        image_reference: &str,
        image_digest: &str,
        auth: &RegistryAuth,
        paths: &Paths,
    ) -> Result<()> {
        // if Policy config file does not exist, get if from KBS.
        let policy_path = &paths.policy_path;

        if !std::path::Path::new(policy_path).exists() {
            warn!("Non {policy_path} found, pass validation.");
            return Ok(());
        }

        let reference = oci_distribution::Reference::try_from(image_reference)?;
        let mut image = Image::default_with_reference(reference);
        image.set_manifest_digest(image_digest)?;

        // Read the set of signature schemes that need to be verified
        // of the image from the policy configuration.
        let mut policy = Policy::from_file(policy_path).await?;
        let schemes = policy.signature_schemes(&image);

        // Get the necessary resources from KBS if needed.
        for scheme in schemes {
            scheme.init(paths).await?;
        }

        policy
            .is_image_allowed(image, auth)
            .await
            .map_err(|e| anyhow::anyhow!("Validate image failed: {:?}", e))
    }
}
