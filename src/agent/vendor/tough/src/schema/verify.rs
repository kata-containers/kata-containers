use super::error::{self, Result};
use super::{Delegations, Role, RoleType, Root, Signed, Targets};
use olpc_cjson::CanonicalFormatter;
use serde::Serialize;
use snafu::{ensure, OptionExt, ResultExt};
use std::collections::HashSet;

impl Root {
    /// Checks that the given metadata role is valid based on a threshold of key signatures.
    pub fn verify_role<T: Role + Serialize>(&self, role: &Signed<T>) -> Result<()> {
        let role_keys = self
            .roles
            .get(&T::TYPE)
            .context(error::MissingRoleSnafu { role: T::TYPE })?;
        let mut valid = 0;

        let mut data = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut data, CanonicalFormatter::new());
        role.signed
            .serialize(&mut ser)
            .context(error::JsonSerializationSnafu {
                what: format!("{} role", T::TYPE),
            })?;

        let mut valid_keyids = HashSet::new();

        for signature in &role.signatures {
            if role_keys.keyids.contains(&signature.keyid) {
                if let Some(key) = self.keys.get(&signature.keyid) {
                    if key.verify(&data, &signature.sig) {
                        // Ignore duplicate keyids.
                        if valid_keyids.insert(&signature.keyid) {
                            valid += 1;
                        }
                    }
                }
            }
        }

        ensure!(
            valid >= u64::from(role_keys.threshold),
            error::SignatureThresholdSnafu {
                role: T::TYPE,
                threshold: role_keys.threshold,
                valid,
            }
        );
        Ok(())
    }
}

impl Delegations {
    /// Verifies that roles matches contain valid keys
    pub fn verify_role(&self, role: &Signed<Targets>, name: &str) -> Result<()> {
        let role_keys =
            self.roles
                .iter()
                .find(|role| role.name == name)
                .ok_or(error::Error::RoleNotFound {
                    name: name.to_string(),
                })?;
        let mut valid = 0;

        // serialize the role to verify the key ID by using the JSON representation
        let mut data = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut data, CanonicalFormatter::new());
        role.signed
            .serialize(&mut ser)
            .context(error::JsonSerializationSnafu {
                what: format!("{} role", name),
            })?;
        for signature in &role.signatures {
            if role_keys.keyids.contains(&signature.keyid) {
                if let Some(key) = self.keys.get(&signature.keyid) {
                    if key.verify(&data, &signature.sig) {
                        valid += 1;
                    }
                }
            }
        }

        ensure!(
            valid >= u64::from(role_keys.threshold),
            error::SignatureThresholdSnafu {
                role: RoleType::Targets,
                threshold: role_keys.threshold,
                valid,
            }
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Root, Signed};

    #[test]
    fn simple_rsa() {
        let root: Signed<Root> =
            serde_json::from_str(include_str!("../../tests/data/simple-rsa/root.json")).unwrap();
        root.signed.verify_role(&root).unwrap();
    }

    #[test]
    fn no_root_json_signatures_is_err() {
        let root: Signed<Root> = serde_json::from_str(include_str!(
            "../../tests/data/no-root-json-signatures/root.json"
        ))
        .expect("should be parsable root.json");
        root.signed
            .verify_role(&root)
            .expect_err("missing signature should not verify");
    }

    #[test]
    fn invalid_root_json_signatures_is_err() {
        let root: Signed<Root> = serde_json::from_str(include_str!(
            "../../tests/data/invalid-root-json-signature/root.json"
        ))
        .expect("should be parsable root.json");
        root.signed
            .verify_role(&root)
            .expect_err("invalid (unauthentic) root signature should not verify");
    }

    #[test]
    // FIXME: this is not actually testing for expired metadata!
    // These tests should be transformed into full repositories and go through Repository::load
    #[ignore]
    fn expired_root_json_signature_is_err() {
        let root: Signed<Root> = serde_json::from_str(include_str!(
            "../../tests/data/expired-root-json-signature/root.json"
        ))
        .expect("should be parsable root.json");
        root.signed
            .verify_role(&root)
            .expect_err("expired root signature should not verify");
    }

    #[test]
    fn mismatched_root_json_keyids_is_err() {
        let root: Signed<Root> = serde_json::from_str(include_str!(
            "../../tests/data/mismatched-root-json-keyids/root.json"
        ))
        .expect("should be parsable root.json");
        root.signed
            .verify_role(&root)
            .expect_err("mismatched root role keyids (provided and signed) should not verify");
    }

    #[test]
    fn duplicate_sigs_is_err() {
        let root: Signed<Root> =
            serde_json::from_str(include_str!("../../tests/data/duplicate-sigs/root.json"))
                .expect("should be parsable root.json");
        root.signed
            .verify_role(&root)
            .expect_err("expired root signature should not verify");
    }

    #[test]
    fn duplicate_sig_keys_is_err() {
        // This metadata is signed with the non-deterministic rsassa-pss signing scheme to
        // demonstrate that we will will detect different signatures made by the same key.
        let root: Signed<Root> = serde_json::from_str(include_str!(
            "../../tests/data/duplicate-sig-keys/root.json"
        ))
        .expect("should be parsable root.json");
        root.signed
            .verify_role(&root)
            .expect_err("expired root signature should not verify");
    }
}
