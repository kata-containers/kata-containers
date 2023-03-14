// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Helps to get auths directly from [`DockerAuthConfig`]s
//! inside a `auth.json`

use std::collections::HashMap;

use anyhow::*;
use oci_distribution::{secrets::RegistryAuth, Reference};

use super::DockerAuthConfig;

/// Read credentials from the auth config, s.t.
/// try to search the target auth due to the
/// image reference as the key. If one is matched,
/// returns it as HTTP Basic authentication. If no
/// one is matched, returns an Anonymous credential.
pub fn credential_from_auth_config(
    reference: &Reference,
    auths: &HashMap<String, DockerAuthConfig>,
) -> Result<RegistryAuth> {
    let registry_auth_map: HashMap<&str, &str> = auths
        .iter()
        .map(|auth| (&auth.0[..], &auth.1.auth[..]))
        .collect();

    let image_ref = full_name(reference);
    let image_registry = reference.resolve_registry();
    let auth_keys = auth_keys_for_key(&image_ref);
    if let Some(auth_str) = auth_keys
        .iter()
        .find_map(|key| registry_auth_map.get(&key[..]))
    {
        let (username, password) = decode_auth(auth_str)?;
        return Ok(RegistryAuth::Basic(username, password));
    }

    // If no exactly auth is for the image reference, normalize the registry
    // keys of the Auth file and try to get the auth again
    let image_registry = normalize_registry(image_registry);
    if let Some((_, auth_str)) = registry_auth_map
        .iter()
        .find(|(key, _)| normalize_key_to_registry(key) == image_registry)
    {
        let (username, password) = decode_auth(auth_str)?;
        return Ok(RegistryAuth::Basic(username, password));
    }

    Ok(RegistryAuth::Anonymous)
}

/// full_name returns the full registry and repository.
fn full_name(reference: &Reference) -> String {
    if reference.registry() == "" {
        reference.repository().to_string()
    } else {
        format!("{}/{}", reference.registry(), reference.repository())
    }
}

/// Normalizes a given key (image reference) into its resulting registry
fn normalize_key_to_registry(key: &str) -> &str {
    let stripped = key.strip_prefix("http://").unwrap_or(key);
    let mut stripped = key.strip_prefix("https://").unwrap_or(stripped);
    if stripped != key {
        stripped = stripped.split_once('/').unwrap_or((stripped, "")).0;
    }

    normalize_registry(stripped)
}

/// Converts the provided registry if a known `docker.io` host
/// is provided.
fn normalize_registry(registry: &str) -> &str {
    match registry {
        "registry-1.docker.io" | "docker.io" => "index.docker.io",
        _ => registry,
    }
}

/// Decode base64-encoded `<username>:<password>`,
/// return (`<username>`, `<password>`). We support both
/// `<username>` and `<password>` are in utf8
fn decode_auth(auth: &str) -> Result<(String, String)> {
    let decoded = base64::decode(auth)?;
    let auth = String::from_utf8(decoded)?;
    let (username, password) = auth
        .split_once(':')
        .ok_or_else(|| anyhow!("Illegal auth content: {auth}"))?;
    Ok((
        username.to_string(),
        password.trim_matches('\n').to_string(),
    ))
}

/// Returns the keys matching a provoded auth file key, in
/// order from the best to worst. For example,
/// when given a repository key `quay.io/confidential-containers/image`,
/// it returns
/// - `quay.io/confidential-containers/image`
/// - `quay.io/confidential-containers`
/// - `quay.io`
fn auth_keys_for_key(key: &str) -> Vec<String> {
    let mut key = key.to_string();
    let mut res = vec![key.clone()];

    while let Some(r) = key.rfind('/') {
        key.truncate(r);
        res.push(key.clone());
    }

    res
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, convert::TryFrom};

    use oci_distribution::{secrets::RegistryAuth, Reference};
    use rstest::rstest;

    use crate::auth::DockerAuthConfig;

    const DOCKER_AUTH_CONFIGS: &str = r#"{
        "https://index.docker.io/v1/": {
            "auth": "bGl1ZGFsaWJqOlBhc3N3MHJkIXFhego="
        },
        "quay.io": {
            "auth": "bGl1ZGFsaWJqOlBhc3N3MHJkIXFhego="
        }
    }"#;

    #[rstest]
    #[case("quay.io/confidential-containers/image", vec!["quay.io/confidential-containers/image", "quay.io/confidential-containers", "quay.io"])]
    #[case("quay.io/image", vec!["quay.io/image", "quay.io"])]
    #[case("quay.io/a/b/c/d/image", vec!["quay.io/a/b/c/d/image", "quay.io/a/b/c/d", "quay.io/a/b/c", "quay.io/a/b", "quay.io/a", "quay.io"])]
    fn test_auth_key_for_key(#[case] key: &str, #[case] expected: Vec<&str>) {
        let res = super::auth_keys_for_key(key);
        let expected: Vec<String> = expected.iter().map(|it| it.to_string()).collect();
        assert_eq!(res, expected);
    }

    #[rstest]
    #[case("bGl1ZGFsaWJqOlBhc3N3MHJkIXFhego=", ("liudalibj","Passw0rd!qaz"))]
    fn test_decode_auth(#[case] auth: &str, #[case] expected: (&str, &str)) {
        let (user, pswd) = super::decode_auth(auth).expect("decode auth failed");
        assert_eq!((&user[..], &pswd[..]), expected);
    }

    #[rstest]
    #[case("mysql:latest", RegistryAuth::Basic("liudalibj".to_string(),"Passw0rd!qaz".to_string()))]
    #[case("quay.io/confidential-containers/image:latest", RegistryAuth::Basic("liudalibj".to_string(),"Passw0rd!qaz".to_string()))]
    #[case("gcr.io/google-containers/busybox:1.27.2", RegistryAuth::Anonymous)]
    fn test_credential_from_auth_config(#[case] reference: &str, #[case] _auth: RegistryAuth) {
        let reference = Reference::try_from(reference).expect("reference creation failed");
        let auths: HashMap<String, DockerAuthConfig> = serde_json::from_str(DOCKER_AUTH_CONFIGS)
            .expect("deserialize DOCKER_AUTH_CONFIGS failed");
        let _got_auth =
            super::credential_from_auth_config(&reference, &auths).expect("get auth failed");
        // TODO: This assert_eq! test needs the following tasks to be Done
        // - wait for this PR to be merged into upstream: https://github.com/krustlet/oci-distribution/pull/48
        // - wait for `oci-distribution` to publish a new release
        // - let `ocicrypt-rs` follow the new release
        // - let `image-rs` follow the new release and delete the notes below
        // assert_eq!(got_auth, auth);
    }

    #[rstest]
    #[case("https://index.docker.io/v1/", "index.docker.io")]
    #[case("https://docker.io/v1/", "index.docker.io")]
    #[case("quay.io", "quay.io")]
    fn test_normalize_key_to_registry(#[case] key: &str, #[case] expected: &str) {
        let registry = super::normalize_key_to_registry(key);
        assert_eq!(registry, expected);
    }
}
