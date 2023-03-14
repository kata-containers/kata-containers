// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, bail, Result};
use oci_distribution::secrets::RegistryAuth;
use serde::Deserialize;
use std::collections::HashMap;
use std::vec::Vec;
use strum_macros::{Display, EnumString};
use tokio::fs;

use self::policy_requirement::PolicyReqType;

use super::image;
use super::mechanism::SignScheme;

pub mod policy_requirement;
pub mod ref_match;

#[derive(EnumString, Display, Debug, PartialEq, Eq)]
pub enum ErrorInfo {
    #[strum(to_string = "Match reference failed.")]
    MatchReference,
    #[strum(to_string = "The policy requirement type name is Unknown.")]
    UnknowPolicyReqType,
    #[strum(to_string = "The reference match policy type name is Unknown.")]
    UnknownMatchPolicyType,
    #[strum(to_string = "The signature scheme is Unknown.")]
    UnknownScheme,
}

// Policy defines requirements for considering a signature, or an image, valid.
// The spec of it is defined in https://github.com/containers/image/blob/main/docs/containers-policy.json.5.md
#[derive(Deserialize)]
pub struct Policy {
    // `default` applies to any image which does not have a matching policy in Transports.
    // Note that this can happen even if a matching `PolicyTransportScopes` exists in `transports`.
    default: Vec<PolicyReqType>,
    transports: HashMap<String, PolicyTransportScopes>,
}

pub type PolicyTransportScopes = HashMap<String, Vec<PolicyReqType>>;

impl Policy {
    // Parse the JSON file of policy (policy.json).
    pub async fn from_file(file_path: &str) -> Result<Self> {
        let policy_json_string = fs::read_to_string(file_path)
            .await
            .map_err(|e| anyhow!("Read policy.json file failed: {:?}", e))?;

        let policy = serde_json::from_str::<Policy>(&policy_json_string)?;
        Ok(policy)
    }

    // Returns Ok(()) if the requirement allows running an image.
    // WARNING: This validates signatures and the manifest, but does not download or validate the
    // layers. Users must validate that the layers match their expected digests.
    pub async fn is_image_allowed(
        &mut self,
        mut image: image::Image,
        auth: &RegistryAuth,
    ) -> Result<()> {
        // Get the policy set that matches the image.
        let reqs = self.requirements_for_image(&image);
        if reqs.is_empty() {
            bail!("List of verification policy requirements must not be empty");
        }

        // The image must meet the requirements of each policy in the policy set.
        for req in reqs.iter() {
            req.allows_image(&mut image, auth).await?;
        }

        Ok(())
    }

    // Get the set of signature schemes that need to be verified of the image.
    pub fn signature_schemes(&mut self, image: &image::Image) -> Vec<&mut dyn SignScheme> {
        self.requirements_for_image(image)
            .iter_mut()
            .filter_map(|req| req.try_into_sign_scheme())
            .collect()
    }

    // selects the appropriate requirements for the image from Policy.
    fn requirements_for_image(&mut self, image: &image::Image) -> &mut Vec<PolicyReqType> {
        // Get transport name of the image
        let transport_name = image.transport_name();

        if let Some(transport_scopes) = self.transports.get_mut(&transport_name) {
            // Look for a full match.
            let identity = image.reference.whole();
            if transport_scopes.contains_key(&identity) {
                return transport_scopes
                    .get_mut(&identity)
                    .expect("Unexpected contains");
            }

            // Look for a match of the possible parent namespaces.
            for name in image::get_image_namespaces(&image.reference).iter() {
                if transport_scopes.contains_key(name) {
                    return transport_scopes.get_mut(name).expect("Unexpected contains");
                }
            }

            // Look for a default match for the transport.
            if let Some(reqs) = transport_scopes.get_mut("") {
                return reqs;
            }
        }

        &mut self.default
    }
}
