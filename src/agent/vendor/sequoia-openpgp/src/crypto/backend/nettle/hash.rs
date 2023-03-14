use crate::crypto::hash::Digest;
use crate::{Error, Result};
use crate::types::{HashAlgorithm};

macro_rules! impl_digest_for {
    ($t: path, $algo: ident) => {
        impl Digest for $t {
            fn algo(&self) -> crate::types::HashAlgorithm {
                crate::types::HashAlgorithm::$algo
            }

            fn digest_size(&self) -> usize {
                nettle::hash::Hash::digest_size(self)
            }

            fn update(&mut self, data: &[u8]) {
                nettle::hash::Hash::update(self, data);
            }

            fn digest(&mut self, digest: &mut [u8]) -> Result<()> {
                nettle::hash::Hash::digest(self, digest);
                Ok(())
            }
        }
    }
}

impl_digest_for!(nettle::hash::Sha224, SHA224);
impl_digest_for!(nettle::hash::Sha256, SHA256);
impl_digest_for!(nettle::hash::Sha384, SHA384);
impl_digest_for!(nettle::hash::Sha512, SHA512);
impl_digest_for!(nettle::hash::insecure_do_not_use::Sha1, SHA1);
impl_digest_for!(nettle::hash::insecure_do_not_use::Md5, MD5);
impl_digest_for!(nettle::hash::insecure_do_not_use::Ripemd160, RipeMD);

impl HashAlgorithm {
    /// Whether Sequoia supports this algorithm.
    pub fn is_supported(self) -> bool {
        match self {
            HashAlgorithm::SHA1 => true,
            HashAlgorithm::SHA224 => true,
            HashAlgorithm::SHA256 => true,
            HashAlgorithm::SHA384 => true,
            HashAlgorithm::SHA512 => true,
            HashAlgorithm::RipeMD => true,
            HashAlgorithm::MD5 => true,
            HashAlgorithm::Private(_) => false,
            HashAlgorithm::Unknown(_) => false,
        }
    }

    /// Creates a new hash context for this algorithm.
    ///
    /// # Errors
    ///
    /// Fails with `Error::UnsupportedHashAlgorithm` if Sequoia does
    /// not support this algorithm. See
    /// [`HashAlgorithm::is_supported`].
    ///
    ///   [`HashAlgorithm::is_supported`]: HashAlgorithm::is_supported()
    pub(crate) fn new_hasher(self) -> Result<Box<dyn Digest>> {
        use nettle::hash::{Sha224, Sha256, Sha384, Sha512};
        use nettle::hash::insecure_do_not_use::{
            Sha1,
            Md5,
            Ripemd160,
        };

        match self {
            HashAlgorithm::SHA1 => Ok(Box::new(Sha1::default())),
            HashAlgorithm::SHA224 => Ok(Box::new(Sha224::default())),
            HashAlgorithm::SHA256 => Ok(Box::new(Sha256::default())),
            HashAlgorithm::SHA384 => Ok(Box::new(Sha384::default())),
            HashAlgorithm::SHA512 => Ok(Box::new(Sha512::default())),
            HashAlgorithm::MD5 => Ok(Box::new(Md5::default())),
            HashAlgorithm::RipeMD => Ok(Box::new(Ripemd160::default())),
            HashAlgorithm::Private(_) | HashAlgorithm::Unknown(_) =>
                Err(Error::UnsupportedHashAlgorithm(self).into()),
        }
    }
}

#[cfg(all(test, feature = "crypto-nettle"))]
mod tests {
    use super::*;
    use nettle::rsa;

    #[test]
    fn oids_match_nettle() {
        assert_eq!(HashAlgorithm::SHA1.oid().unwrap(), rsa::ASN1_OID_SHA1);
        assert_eq!(HashAlgorithm::SHA224.oid().unwrap(), rsa::ASN1_OID_SHA224);
        assert_eq!(HashAlgorithm::SHA256.oid().unwrap(), rsa::ASN1_OID_SHA256);
        assert_eq!(HashAlgorithm::SHA384.oid().unwrap(), rsa::ASN1_OID_SHA384);
        assert_eq!(HashAlgorithm::SHA512.oid().unwrap(), rsa::ASN1_OID_SHA512);
        assert_eq!(HashAlgorithm::MD5.oid().unwrap(), rsa::ASN1_OID_MD5);
        assert_eq!(HashAlgorithm::RipeMD.oid().unwrap(), rsa::ASN1_OID_RIPEMD160);
    }
}
