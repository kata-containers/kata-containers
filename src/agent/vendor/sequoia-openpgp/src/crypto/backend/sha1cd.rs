use crate::crypto::hash::Digest;
use crate::Result;

pub(crate) fn build() -> sha1collisiondetection::Sha1CD {
    sha1collisiondetection::Builder::default()
        .detect_collisions(true)
        .use_ubc(true)
        .safe_hash(true)
        .build()
}

impl Digest for sha1collisiondetection::Sha1CD {
    fn algo(&self) -> crate::types::HashAlgorithm {
        crate::types::HashAlgorithm::SHA1
    }

    fn digest_size(&self) -> usize {
        20
    }

    fn update(&mut self, data: &[u8]) {
        sha1collisiondetection::Sha1CD::update(self, data);
    }

    fn digest(&mut self, digest: &mut [u8]) -> Result<()> {
        let mut d = sha1collisiondetection::Output::default();
        let r = self.finalize_into_dirty_cd(&mut d);
        self.reset();
        let l = digest.len().min(d.len());
        digest[..l].copy_from_slice(&d[..l]);
        r.map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use crate::parse::{Parse, stream::*};
    use crate::policy::StandardPolicy;

    /// Test vector from the "SHA-1 is a Shambles" paper.
    ///
    /// The scenario is the following.  Bob obtains a certification
    /// from a CA, and transfers it to a key claiming to belong to
    /// Alice.  Now the CA certifies an illegitimate binding to
    /// Alice's userid.
    #[test]
    fn shambles() -> Result<()> {
        let alice =
            PacketPile::from_bytes(crate::tests::key("sha-mbles.alice.asc"))?;
        let bob =
            PacketPile::from_bytes(crate::tests::key("sha-mbles.bob.asc"))?;
        let ca_keyid: KeyID = "AFBB 1FED 6951 A956".parse()?;

        assert_eq!(alice.children().count(), 4);
        assert_eq!(bob.children().count(), 7);

        let alice_sha1_fingerprint: Fingerprint =
            "43CD 5C5B 04FF 5742 FA14  1ABC A9D7 55A9 6354 8C78".parse()?;
        let bob_sha1_fingerprint: Fingerprint =
            "C6BF E2FC BBE5 1A89 2BEB  7798 1233 D4CC 61DB D9C4".parse()?;

        let alice_sha1cd_fingerprint: Fingerprint =
            "4D84 B08A A181 21DB D79E  EA05 9CD0 8D5B 1680 87E2".parse()?;
        let bob_sha1cd_fingerprint: Fingerprint =
            "6434 B04B 4648 BA41 15BD  C5C2 B67A DB26 6F74 DF89".parse()?;

        // The illegitimate certification is on Bob's user attribute.
        assert_eq!(bob.path_ref(&[6]).unwrap(), alice.path_ref(&[3]).unwrap());
        match bob.path_ref(&[6]).unwrap() {
            Packet::Signature(s) => {
                assert_eq!(s.issuers().next().unwrap(), &ca_keyid);
            },
            o => panic!("unexpected packet: {:?}", o),
        }

        let alice = Cert::from_packets(alice.into_children())?;
        let bob = Cert::from_packets(bob.into_children())?;

        // Check mitigations.  First, the illegitimate certification
        // should be discarded.
        assert_eq!(alice.bad_signatures().count(), 1);
        // Bob's userid also got certified, hence there are two bad
        // signatures.
        assert_eq!(bob.bad_signatures().count(), 2);

        // The mitigation also changes the identities of the keys
        // containing the collision attack.  This is a good thing,
        // because we cannot trust SHA-1 to discriminate keys
        // containing attacks.
        assert!(alice.fingerprint() != alice_sha1_fingerprint);
        assert_eq!(alice.fingerprint(), alice_sha1cd_fingerprint);
        assert!(bob.fingerprint() != bob_sha1_fingerprint);
        assert_eq!(bob.fingerprint(), bob_sha1cd_fingerprint);
        Ok(())
    }

    /// Test vector from the paper "The first collision for full SHA-1".
    #[test]
    fn shattered() -> Result<()> {
        let cert =
            Cert::from_bytes(crate::tests::key("testy-new.pgp"))?;
        let shattered_1 = crate::tests::message("shattered-1.pdf");
        let shattered_1_sig = crate::tests::message("shattered-1.pdf.sig");
        let shattered_2 = crate::tests::message("shattered-2.pdf");
        let shattered_2_sig = crate::tests::message("shattered-2.pdf.sig");

        let mut p = StandardPolicy::new();
        p.accept_hash(types::HashAlgorithm::SHA1);

        // This fetches keys and computes the validity of the verification.
        struct Helper(Cert);
        impl VerificationHelper for Helper {
            fn get_certs(&mut self, _ids: &[KeyHandle]) -> Result<Vec<Cert>> {
                Ok(vec![self.0.clone()])
            }
            fn check(&mut self, structure: MessageStructure) -> Result<()> {
                if let MessageLayer::SignatureGroup { results } =
                    structure.into_iter().next().unwrap()
                {
                    assert_eq!(results.len(), 1);
                    assert!(results[0].is_err());
                } else {
                    unreachable!()
                }
                Ok(())
            }
        }

        let h = Helper(cert.clone());
        let mut v = DetachedVerifierBuilder::from_bytes(shattered_1_sig)?
            .with_policy(&p, None, h)?;
        v.verify_bytes(shattered_1)?;

        let h = Helper(cert);
        let mut v = DetachedVerifierBuilder::from_bytes(shattered_2_sig)?
            .with_policy(&p, None, h)?;
        v.verify_bytes(shattered_2)?;

        Ok(())
    }
}
