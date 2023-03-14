use crate::error::Error;
use crate::key;
use crate::limited_cache;
use crate::server;
use crate::server::ClientHello;
use crate::sign;

use std::collections;
use std::sync::{Arc, Mutex};

/// Something which never stores sessions.
pub struct NoServerSessionStorage {}

impl server::StoresServerSessions for NoServerSessionStorage {
    fn put(&self, _id: Vec<u8>, _sec: Vec<u8>) -> bool {
        false
    }
    fn get(&self, _id: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn take(&self, _id: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn can_cache(&self) -> bool {
        false
    }
}

/// An implementer of `StoresServerSessions` that stores everything
/// in memory.  If enforces a limit on the number of stored sessions
/// to bound memory usage.
pub struct ServerSessionMemoryCache {
    cache: Mutex<limited_cache::LimitedCache<Vec<u8>, Vec<u8>>>,
}

impl ServerSessionMemoryCache {
    /// Make a new ServerSessionMemoryCache.  `size` is the maximum
    /// number of stored sessions, and may be rounded-up for
    /// efficiency.
    pub fn new(size: usize) -> Arc<Self> {
        Arc::new(Self {
            cache: Mutex::new(limited_cache::LimitedCache::new(size)),
        })
    }
}

impl server::StoresServerSessions for ServerSessionMemoryCache {
    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> bool {
        self.cache
            .lock()
            .unwrap()
            .insert(key, value);
        true
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.cache
            .lock()
            .unwrap()
            .get(key)
            .cloned()
    }

    fn take(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.cache.lock().unwrap().remove(key)
    }

    fn can_cache(&self) -> bool {
        true
    }
}

/// Something which never produces tickets.
pub(super) struct NeverProducesTickets {}

impl server::ProducesTickets for NeverProducesTickets {
    fn enabled(&self) -> bool {
        false
    }
    fn lifetime(&self) -> u32 {
        0
    }
    fn encrypt(&self, _bytes: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn decrypt(&self, _bytes: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

/// Something which always resolves to the same cert chain.
pub(super) struct AlwaysResolvesChain(Arc<sign::CertifiedKey>);

impl AlwaysResolvesChain {
    /// Creates an `AlwaysResolvesChain`, auto-detecting the underlying private
    /// key type and encoding.
    pub(super) fn new(
        chain: Vec<key::Certificate>,
        priv_key: &key::PrivateKey,
    ) -> Result<Self, Error> {
        let key = sign::any_supported_type(priv_key)
            .map_err(|_| Error::General("invalid private key".into()))?;
        Ok(Self(Arc::new(sign::CertifiedKey::new(chain, key))))
    }

    /// Creates an `AlwaysResolvesChain`, auto-detecting the underlying private
    /// key type and encoding.
    ///
    /// If non-empty, the given OCSP response and SCTs are attached.
    pub(super) fn new_with_extras(
        chain: Vec<key::Certificate>,
        priv_key: &key::PrivateKey,
        ocsp: Vec<u8>,
        scts: Vec<u8>,
    ) -> Result<Self, Error> {
        let mut r = Self::new(chain, priv_key)?;

        {
            let cert = Arc::make_mut(&mut r.0);
            if !ocsp.is_empty() {
                cert.ocsp = Some(ocsp);
            }
            if !scts.is_empty() {
                cert.sct_list = Some(scts);
            }
        }

        Ok(r)
    }
}

impl server::ResolvesServerCert for AlwaysResolvesChain {
    fn resolve(&self, _client_hello: ClientHello) -> Option<Arc<sign::CertifiedKey>> {
        Some(Arc::clone(&self.0))
    }
}

/// Something that resolves do different cert chains/keys based
/// on client-supplied server name (via SNI).
pub struct ResolvesServerCertUsingSni {
    by_name: collections::HashMap<String, Arc<sign::CertifiedKey>>,
}

impl ResolvesServerCertUsingSni {
    /// Create a new and empty (i.e., knows no certificates) resolver.
    pub fn new() -> Self {
        Self {
            by_name: collections::HashMap::new(),
        }
    }

    /// Add a new `sign::CertifiedKey` to be used for the given SNI `name`.
    ///
    /// This function fails if `name` is not a valid DNS name, or if
    /// it's not valid for the supplied certificate, or if the certificate
    /// chain is syntactically faulty.
    pub fn add(&mut self, name: &str, ck: sign::CertifiedKey) -> Result<(), Error> {
        let checked_name = webpki::DnsNameRef::try_from_ascii_str(name)
            .map_err(|_| Error::General("Bad DNS name".into()))?
            .to_owned();

        ck.cross_check_end_entity_cert(Some(checked_name.as_ref()))?;
        let as_str: &str = checked_name.as_ref().into();
        self.by_name
            .insert(as_str.to_string(), Arc::new(ck));
        Ok(())
    }
}

impl server::ResolvesServerCert for ResolvesServerCertUsingSni {
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<sign::CertifiedKey>> {
        if let Some(name) = client_hello.server_name() {
            self.by_name.get(name).map(Arc::clone)
        } else {
            // This kind of resolver requires SNI
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::server::ProducesTickets;
    use crate::server::ResolvesServerCert;
    use crate::server::StoresServerSessions;

    #[test]
    fn test_noserversessionstorage_drops_put() {
        let c = NoServerSessionStorage {};
        assert!(!c.put(vec![0x01], vec![0x02]));
    }

    #[test]
    fn test_noserversessionstorage_denies_gets() {
        let c = NoServerSessionStorage {};
        c.put(vec![0x01], vec![0x02]);
        assert_eq!(c.get(&[]), None);
        assert_eq!(c.get(&[0x01]), None);
        assert_eq!(c.get(&[0x02]), None);
    }

    #[test]
    fn test_noserversessionstorage_denies_takes() {
        let c = NoServerSessionStorage {};
        assert_eq!(c.take(&[]), None);
        assert_eq!(c.take(&[0x01]), None);
        assert_eq!(c.take(&[0x02]), None);
    }

    #[test]
    fn test_serversessionmemorycache_accepts_put() {
        let c = ServerSessionMemoryCache::new(4);
        assert!(c.put(vec![0x01], vec![0x02]));
    }

    #[test]
    fn test_serversessionmemorycache_persists_put() {
        let c = ServerSessionMemoryCache::new(4);
        assert!(c.put(vec![0x01], vec![0x02]));
        assert_eq!(c.get(&[0x01]), Some(vec![0x02]));
        assert_eq!(c.get(&[0x01]), Some(vec![0x02]));
    }

    #[test]
    fn test_serversessionmemorycache_overwrites_put() {
        let c = ServerSessionMemoryCache::new(4);
        assert!(c.put(vec![0x01], vec![0x02]));
        assert!(c.put(vec![0x01], vec![0x04]));
        assert_eq!(c.get(&[0x01]), Some(vec![0x04]));
    }

    #[test]
    fn test_serversessionmemorycache_drops_to_maintain_size_invariant() {
        let c = ServerSessionMemoryCache::new(2);
        assert!(c.put(vec![0x01], vec![0x02]));
        assert!(c.put(vec![0x03], vec![0x04]));
        assert!(c.put(vec![0x05], vec![0x06]));
        assert!(c.put(vec![0x07], vec![0x08]));
        assert!(c.put(vec![0x09], vec![0x0a]));

        let count = c.get(&[0x01]).iter().count()
            + c.get(&[0x03]).iter().count()
            + c.get(&[0x05]).iter().count()
            + c.get(&[0x07]).iter().count()
            + c.get(&[0x09]).iter().count();

        assert!(count < 5);
    }

    #[test]
    fn test_neverproducestickets_does_nothing() {
        let npt = NeverProducesTickets {};
        assert!(!npt.enabled());
        assert_eq!(0, npt.lifetime());
        assert_eq!(None, npt.encrypt(&[]));
        assert_eq!(None, npt.decrypt(&[]));
    }

    #[test]
    fn test_resolvesservercertusingsni_requires_sni() {
        let rscsni = ResolvesServerCertUsingSni::new();
        assert!(rscsni
            .resolve(ClientHello::new(&None, &[], None, &[]))
            .is_none());
    }

    #[test]
    fn test_resolvesservercertusingsni_handles_unknown_name() {
        let rscsni = ResolvesServerCertUsingSni::new();
        let name = webpki::DnsNameRef::try_from_ascii_str("hello.com")
            .unwrap()
            .to_owned();
        assert!(rscsni
            .resolve(ClientHello::new(&Some(name), &[], None, &[]))
            .is_none());
    }
}
