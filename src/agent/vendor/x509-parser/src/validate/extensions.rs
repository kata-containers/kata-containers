use crate::extensions::*;
use crate::validate::*;
use std::collections::HashSet;

// extra-pedantic checks

const WARN_SHOULD_BE_CRITICAL: bool = false;

macro_rules! test_critical {
    (MUST $ext:ident, $l:ident, $name:expr) => {
        if !$ext.critical {
            $l.err(&format!("Extension {} MUST be critical, but is not", $name));
        }
    };
    (MUST NOT $ext:ident, $l:ident, $name:expr) => {
        if $ext.critical {
            $l.err(&format!("Extension {} MUST NOT be critical, but is", $name));
        }
    };
    (SHOULD $ext:ident, $l:ident, $name:expr) => {
        if WARN_SHOULD_BE_CRITICAL && !$ext.critical {
            $l.warn(&format!(
                "Extension {} SHOULD be critical, but is not",
                $name
            ));
        }
    };
    (SHOULD NOT $ext:ident, $l:ident, $name:expr) => {
        if WARN_SHOULD_BE_CRITICAL && $ext.critical {
            $l.warn(&format!(
                "Extension {} SHOULD NOT be critical, but is",
                $name
            ));
        }
    };
}

#[derive(Debug)]
pub struct X509ExtensionsValidator;

impl<'a> Validator<'a> for X509ExtensionsValidator {
    type Item = &'a [X509Extension<'a>];

    fn validate<L: Logger>(&self, item: &'a Self::Item, l: &'_ mut L) -> bool {
        let mut res = true;
        // check for duplicate extensions
        {
            let mut m = HashSet::new();
            for ext in item.iter() {
                if m.contains(&ext.oid) {
                    l.err(&format!("Duplicate extension {}", ext.oid));
                    res = false;
                } else {
                    m.insert(ext.oid.clone());
                }
            }
        }

        for ext in item.iter() {
            // specific extension checks
            match ext.parsed_extension() {
                ParsedExtension::AuthorityKeyIdentifier(aki) => {
                    // Conforming CAs MUST mark this extension as non-critical
                    test_critical!(MUST NOT ext, l, "AKI");
                    // issuer or serial is present must be either both present or both absent
                    if aki.authority_cert_issuer.is_some() ^ aki.authority_cert_serial.is_some() {
                        l.warn("AKI: only one of Issuer and Serial is present");
                    }
                }
                ParsedExtension::CertificatePolicies(policies) => {
                    // A certificate policy OID MUST NOT appear more than once in a
                    // certificate policies extension.
                    let mut policy_oids = HashSet::new();
                    for policy_info in policies {
                        if policy_oids.contains(&policy_info.policy_id) {
                            l.err(&format!(
                                "Certificate Policies: duplicate policy {}",
                                policy_info.policy_id
                            ));
                            res = false;
                        } else {
                            policy_oids.insert(policy_info.policy_id.clone());
                        }
                    }
                }
                ParsedExtension::KeyUsage(ku) => {
                    // SHOULD be critical
                    test_critical!(SHOULD ext, l, "KeyUsage");
                    // When the keyUsage extension appears in a certificate, at least one of the bits
                    // MUST be set to 1.
                    if ku.flags == 0 {
                        l.err("KeyUsage: all flags are set to 0");
                    }
                }
                ParsedExtension::SubjectAlternativeName(san) => {
                    // SHOULD be non-critical
                    test_critical!(SHOULD NOT ext, l, "SubjectAltName");
                    for name in &san.general_names {
                        match name {
                            GeneralName::DNSName(ref s) | GeneralName::RFC822Name(ref s) => {
                                // should be an ia5string
                                if !s.as_bytes().iter().all(u8::is_ascii) {
                                    l.warn(&format!("Invalid charset in 'SAN' entry '{}'", s));
                                }
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }
        res
    }
}
