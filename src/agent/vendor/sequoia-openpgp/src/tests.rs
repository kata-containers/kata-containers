//! Test data for Sequoia.
//!
//! This module includes the test data from `openpgp/tests/data` in a
//! structured way.

use std::fmt;
use std::collections::BTreeMap;

pub struct Test {
    path: &'static str,
    pub bytes: &'static [u8],
}

impl fmt::Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "openpgp/tests/data/{}", self.path)
    }
}

macro_rules! t {
    ( $path: expr ) => {
        &Test {
            path: $path,
            bytes: include_bytes!(concat!("../tests/data/", $path)),
        }
    }
}

pub const CERTS: &[&Test] = &[
    t!("keys/dennis-simon-anton.pgp"),
    t!("keys/dsa2048-elgamal3072.pgp"),
    t!("keys/emmelie-dorothea-dina-samantha-awina-ed25519.pgp"),
    t!("keys/erika-corinna-daniela-simone-antonia-nistp256.pgp"),
    t!("keys/erika-corinna-daniela-simone-antonia-nistp384.pgp"),
    t!("keys/erika-corinna-daniela-simone-antonia-nistp521.pgp"),
    t!("keys/testy-new.pgp"),
    t!("keys/testy.pgp"),
    t!("keys/neal.pgp"),
    t!("keys/dkg-sigs-out-of-order.pgp"),
];

pub const TSKS: &[&Test] = &[
    t!("keys/dennis-simon-anton-private.pgp"),
    t!("keys/dsa2048-elgamal3072-private.pgp"),
    t!("keys/emmelie-dorothea-dina-samantha-awina-ed25519-private.pgp"),
    t!("keys/erika-corinna-daniela-simone-antonia-nistp256-private.pgp"),
    t!("keys/erika-corinna-daniela-simone-antonia-nistp384-private.pgp"),
    t!("keys/erika-corinna-daniela-simone-antonia-nistp521-private.pgp"),
    t!("keys/testy-new-private.pgp"),
    t!("keys/testy-nistp256-private.pgp"),
    t!("keys/testy-nistp384-private.pgp"),
    t!("keys/testy-nistp521-private.pgp"),
    t!("keys/testy-private.pgp"),
];

/// Returns the content of the given file below `openpgp/tests/data`.
pub fn file(name: &str) -> &'static [u8] {
    lazy_static::lazy_static! {
        static ref FILES: BTreeMap<&'static str, &'static [u8]> = {
            let mut m: BTreeMap<&'static str, &'static [u8]> =
                Default::default();

            macro_rules! add {
                ( $key: expr, $path: expr ) => {
                    m.insert($key, include_bytes!($path))
                }
            }
            include!(concat!(env!("OUT_DIR"), "/tests.index.rs.inc"));

            // Sanity checks.
            assert!(m.contains_key("messages/a-cypherpunks-manifesto.txt"));
            assert!(m.contains_key("keys/testy.pgp"));
            assert!(m.contains_key("keys/testy-private.pgp"));
            m
        };
    }

    FILES.get(name).unwrap_or_else(|| panic!("No such file {:?}", name))
}

/// Returns the content of the given file below `openpgp/tests/data/keys`.
pub fn key(name: &str) -> &'static [u8] {
    file(&format!("keys/{}", name))
}

/// Returns the content of the given file below `openpgp/tests/data/messages`.
pub fn message(name: &str) -> &'static [u8] {
    file(&format!("messages/{}", name))
}

/// Returns the cypherpunks manifesto.
pub fn manifesto() -> &'static [u8] {
    message("a-cypherpunks-manifesto.txt")
}
