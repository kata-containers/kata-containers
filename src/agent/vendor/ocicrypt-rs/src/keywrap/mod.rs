// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::Result;

use crate::config::{DecryptConfig, EncryptConfig};

#[cfg(feature = "keywrap-jwe")]
pub mod jwe;
#[cfg(feature = "keywrap-keyprovider")]
pub mod keyprovider;

/// KeyWrapper is the interface used for wrapping keys using
/// a specific encryption technology (pgp, jwe, pkcs7, pkcs11, keyprovider)
#[allow(unused_variables)]
pub trait KeyWrapper: Send + Sync {
    /// wrap keys data with encrypt config.
    fn wrap_keys(&self, ec: &EncryptConfig, opts_data: &[u8]) -> Result<Vec<u8>>;

    /// unwrap keys data with decrypt config.
    fn unwrap_keys(&self, dc: &DecryptConfig, annotation: &[u8]) -> Result<Vec<u8>>;

    /// return the keywraper annotation id.
    fn annotation_id(&self) -> String;

    /// Check whether the driver could handle the decryption request.
    fn probe(&self, dc_param: &HashMap<String, Vec<Vec<u8>>>) -> bool;

    /// private_keys (optional) gets the array of private keys. It is an optional implementation
    /// as in some key services, a private key may not be exportable (i.e. HSM)
    /// If not implemented, return `None`.
    fn private_keys(&self, dc_param: &HashMap<String, Vec<Vec<u8>>>) -> Option<Vec<Vec<u8>>> {
        None
    }

    /// keyids_from_packet (optional) gets a list of key IDs. This is optional as some encryption
    /// schemes may not have a notion of key IDs
    /// If not implemented, return `None`.
    fn keyids_from_packet(&self, packet: String) -> Option<Vec<u64>> {
        None
    }

    /// recipients (optional) gets a list of recipients. It is optional due to the validity of
    /// recipients in a particular encryption scheme
    /// If not implemented, return `None`.
    fn recipients(&self, recipients: String) -> Option<Vec<String>> {
        None
    }
}

impl<W: KeyWrapper + ?Sized> KeyWrapper for Box<W> {
    #[inline]
    fn wrap_keys(&self, ec: &EncryptConfig, opts_data: &[u8]) -> Result<Vec<u8>> {
        (**self).wrap_keys(ec, opts_data)
    }

    #[inline]
    fn unwrap_keys(&self, dc: &DecryptConfig, annotation: &[u8]) -> Result<Vec<u8>> {
        (**self).unwrap_keys(dc, annotation)
    }

    #[inline]
    fn annotation_id(&self) -> String {
        (**self).annotation_id()
    }

    #[inline]
    fn probe(&self, dc_param: &HashMap<String, Vec<Vec<u8>>>) -> bool {
        (**self).probe(dc_param)
    }

    #[inline]
    fn private_keys(&self, dc_param: &HashMap<String, Vec<Vec<u8>>>) -> Option<Vec<Vec<u8>>> {
        (**self).private_keys(dc_param)
    }

    #[inline]
    fn keyids_from_packet(&self, packet: String) -> Option<Vec<u64>> {
        (**self).keyids_from_packet(packet)
    }

    #[inline]
    fn recipients(&self, recipients: String) -> Option<Vec<String>> {
        (**self).recipients(recipients)
    }
}
