// -*- mode: rust; -*-
//
// This file is part of ed25519-dalek.
// Copyright (c) 2017-2019 isis lovecruft
// See LICENSE for licensing information.
//
// Authors:
// - isis agora lovecruft <isis@patternsinthevoid.net>

//! Integration tests for ed25519-dalek.

#[cfg(all(test, feature = "serde"))]
extern crate bincode;
extern crate ed25519_dalek;
extern crate hex;
extern crate sha2;
extern crate rand;
#[cfg(all(test, feature = "serde"))]
extern crate serde_crate;
#[cfg(all(test, feature = "serde"))]
extern crate toml;

use ed25519_dalek::*;

use hex::FromHex;

use sha2::Sha512;

#[cfg(test)]
mod vectors {
    use ed25519::signature::Signature as _;

    use std::io::BufReader;
    use std::io::BufRead;
    use std::fs::File;

    use super::*;

    // TESTVECTORS is taken from sign.input.gz in agl's ed25519 Golang
    // package. It is a selection of test cases from
    // http://ed25519.cr.yp.to/python/sign.input
    #[test]
    fn against_reference_implementation() { // TestGolden
        let mut line: String;
        let mut lineno: usize = 0;

        let f = File::open("TESTVECTORS");
        if f.is_err() {
            println!("This test is only available when the code has been cloned \
                      from the git repository, since the TESTVECTORS file is large \
                      and is therefore not included within the distributed crate.");
            panic!();
        }
        let file = BufReader::new(f.unwrap());

        for l in file.lines() {
            lineno += 1;
            line = l.unwrap();

            let parts: Vec<&str> = line.split(':').collect();
            assert_eq!(parts.len(), 5, "wrong number of fields in line {}", lineno);

            let sec_bytes: Vec<u8> = FromHex::from_hex(&parts[0]).unwrap();
            let pub_bytes: Vec<u8> = FromHex::from_hex(&parts[1]).unwrap();
            let msg_bytes: Vec<u8> = FromHex::from_hex(&parts[2]).unwrap();
            let sig_bytes: Vec<u8> = FromHex::from_hex(&parts[3]).unwrap();

            let secret: SecretKey = SecretKey::from_bytes(&sec_bytes[..SECRET_KEY_LENGTH]).unwrap();
            let public: PublicKey = PublicKey::from_bytes(&pub_bytes[..PUBLIC_KEY_LENGTH]).unwrap();
            let keypair: Keypair  = Keypair{ secret: secret, public: public };

		    // The signatures in the test vectors also include the message
		    // at the end, but we just want R and S.
            let sig1: Signature = Signature::from_bytes(&sig_bytes[..64]).unwrap();
            let sig2: Signature = keypair.sign(&msg_bytes);

            assert!(sig1 == sig2, "Signature bytes not equal on line {}", lineno);
            assert!(keypair.verify(&msg_bytes, &sig2).is_ok(),
                    "Signature verification failed on line {}", lineno);
        }
    }

    // From https://tools.ietf.org/html/rfc8032#section-7.3
    #[test]
    fn ed25519ph_rf8032_test_vector() {
        let secret_key: &[u8] = b"833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42";
        let public_key: &[u8] = b"ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf";
        let message: &[u8] = b"616263";
        let signature: &[u8] = b"98a70222f0b8121aa9d30f813d683f809e462b469c7ff87639499bb94e6dae4131f85042463c2a355a2003d062adf5aaa10b8c61e636062aaad11c2a26083406";

        let sec_bytes: Vec<u8> = FromHex::from_hex(secret_key).unwrap();
        let pub_bytes: Vec<u8> = FromHex::from_hex(public_key).unwrap();
        let msg_bytes: Vec<u8> = FromHex::from_hex(message).unwrap();
        let sig_bytes: Vec<u8> = FromHex::from_hex(signature).unwrap();

        let secret: SecretKey = SecretKey::from_bytes(&sec_bytes[..SECRET_KEY_LENGTH]).unwrap();
        let public: PublicKey = PublicKey::from_bytes(&pub_bytes[..PUBLIC_KEY_LENGTH]).unwrap();
        let keypair: Keypair  = Keypair{ secret: secret, public: public };
        let sig1: Signature = Signature::from_bytes(&sig_bytes[..]).unwrap();

        let mut prehash_for_signing: Sha512 = Sha512::default();
        let mut prehash_for_verifying: Sha512 = Sha512::default();

        prehash_for_signing.update(&msg_bytes[..]);
        prehash_for_verifying.update(&msg_bytes[..]);

        let sig2: Signature = keypair.sign_prehashed(prehash_for_signing, None).unwrap();

        assert!(sig1 == sig2,
                "Original signature from test vectors doesn't equal signature produced:\
                \noriginal:\n{:?}\nproduced:\n{:?}", sig1, sig2);
        assert!(keypair.verify_prehashed(prehash_for_verifying, None, &sig2).is_ok(),
                "Could not verify ed25519ph signature!");
    }
}

#[cfg(test)]
mod integrations {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn sign_verify() {  // TestSignVerify
        let keypair: Keypair;
        let good_sig: Signature;
        let bad_sig:  Signature;

        let good: &[u8] = "test message".as_bytes();
        let bad:  &[u8] = "wrong message".as_bytes();

        let mut csprng = OsRng{};

        keypair  = Keypair::generate(&mut csprng);
        good_sig = keypair.sign(&good);
        bad_sig  = keypair.sign(&bad);

        assert!(keypair.verify(&good, &good_sig).is_ok(),
                "Verification of a valid signature failed!");
        assert!(keypair.verify(&good, &bad_sig).is_err(),
                "Verification of a signature on a different message passed!");
        assert!(keypair.verify(&bad,  &good_sig).is_err(),
                "Verification of a signature on a different message passed!");
    }

    #[test]
    fn ed25519ph_sign_verify() {
        let keypair: Keypair;
        let good_sig: Signature;
        let bad_sig:  Signature;

        let good: &[u8] = b"test message";
        let bad:  &[u8] = b"wrong message";

        let mut csprng = OsRng{};

        // ugh… there's no `impl Copy for Sha512`… i hope we can all agree these are the same hashes
        let mut prehashed_good1: Sha512 = Sha512::default();
        prehashed_good1.update(good);
        let mut prehashed_good2: Sha512 = Sha512::default();
        prehashed_good2.update(good);
        let mut prehashed_good3: Sha512 = Sha512::default();
        prehashed_good3.update(good);

        let mut prehashed_bad1: Sha512 = Sha512::default();
        prehashed_bad1.update(bad);
        let mut prehashed_bad2: Sha512 = Sha512::default();
        prehashed_bad2.update(bad);

        let context: &[u8] = b"testing testing 1 2 3";

        keypair  = Keypair::generate(&mut csprng);
        good_sig = keypair.sign_prehashed(prehashed_good1, Some(context)).unwrap();
        bad_sig  = keypair.sign_prehashed(prehashed_bad1,  Some(context)).unwrap();

        assert!(keypair.verify_prehashed(prehashed_good2, Some(context), &good_sig).is_ok(),
                "Verification of a valid signature failed!");
        assert!(keypair.verify_prehashed(prehashed_good3, Some(context), &bad_sig).is_err(),
                "Verification of a signature on a different message passed!");
        assert!(keypair.verify_prehashed(prehashed_bad2,  Some(context), &good_sig).is_err(),
                "Verification of a signature on a different message passed!");
    }

    #[cfg(feature = "batch")]
    #[test]
    fn verify_batch_seven_signatures() {
        let messages: [&[u8]; 7] = [
            b"Watch closely everyone, I'm going to show you how to kill a god.",
            b"I'm not a cryptographer I just encrypt a lot.",
            b"Still not a cryptographer.",
            b"This is a test of the tsunami alert system. This is only a test.",
            b"Fuck dumbin' it down, spit ice, skip jewellery: Molotov cocktails on me like accessories.",
            b"Hey, I never cared about your bucks, so if I run up with a mask on, probably got a gas can too.",
            b"And I'm not here to fill 'er up. Nope, we came to riot, here to incite, we don't want any of your stuff.", ];
        let mut csprng = OsRng{};
        let mut keypairs: Vec<Keypair> = Vec::new();
        let mut signatures: Vec<Signature> = Vec::new();

        for i in 0..messages.len() {
            let keypair: Keypair = Keypair::generate(&mut csprng);
            signatures.push(keypair.sign(&messages[i]));
            keypairs.push(keypair);
        }
        let public_keys: Vec<PublicKey> = keypairs.iter().map(|key| key.public).collect();

        let result = verify_batch(&messages, &signatures[..], &public_keys[..]);

        assert!(result.is_ok());
    }

    #[test]
    fn pubkey_from_secret_and_expanded_secret() {
        let mut csprng = OsRng{};
        let secret: SecretKey = SecretKey::generate(&mut csprng);
        let expanded_secret: ExpandedSecretKey = (&secret).into();
        let public_from_secret: PublicKey = (&secret).into(); // XXX eww
        let public_from_expanded_secret: PublicKey = (&expanded_secret).into(); // XXX eww

        assert!(public_from_secret == public_from_expanded_secret);
    }
}

#[serde(crate = "serde_crate")]
#[cfg(all(test, feature = "serde"))]
#[derive(Debug, serde_crate::Serialize, serde_crate::Deserialize)]
struct Demo {
    keypair: Keypair
}

#[cfg(all(test, feature = "serde"))]
mod serialisation {
    use super::*;

    use ed25519::signature::Signature as _;

    // The size for bincode to serialize the length of a byte array.
    static BINCODE_INT_LENGTH: usize = 8;

    static PUBLIC_KEY_BYTES: [u8; PUBLIC_KEY_LENGTH] = [
        130, 039, 155, 015, 062, 076, 188, 063,
        124, 122, 026, 251, 233, 253, 225, 220,
        014, 041, 166, 120, 108, 035, 254, 077,
        160, 083, 172, 058, 219, 042, 086, 120, ];

    static SECRET_KEY_BYTES: [u8; SECRET_KEY_LENGTH] = [
        062, 070, 027, 163, 092, 182, 011, 003,
        077, 234, 098, 004, 011, 127, 079, 228,
        243, 187, 150, 073, 201, 137, 076, 022,
        085, 251, 152, 002, 241, 042, 072, 054, ];

    /// Signature with the above keypair of a blank message.
    static SIGNATURE_BYTES: [u8; SIGNATURE_LENGTH] = [
        010, 126, 151, 143, 157, 064, 047, 001,
        196, 140, 179, 058, 226, 152, 018, 102,
        160, 123, 080, 016, 210, 086, 196, 028,
        053, 231, 012, 157, 169, 019, 158, 063,
        045, 154, 238, 007, 053, 185, 227, 229,
        079, 108, 213, 080, 124, 252, 084, 167,
        216, 085, 134, 144, 129, 149, 041, 081,
        063, 120, 126, 100, 092, 059, 050, 011, ];

    static KEYPAIR_BYTES: [u8; KEYPAIR_LENGTH] = [
        239, 085, 017, 235, 167, 103, 034, 062,
        007, 010, 032, 146, 113, 039, 096, 174,
        003, 219, 232, 166, 240, 121, 167, 013,
        098, 238, 122, 116, 193, 114, 215, 213,
        175, 181, 075, 166, 224, 164, 140, 146,
        053, 120, 010, 037, 104, 094, 136, 225,
        249, 102, 171, 160, 097, 132, 015, 071,
        035, 056, 000, 074, 130, 168, 225, 071, ];

    #[test]
    fn serialize_deserialize_signature_bincode() {
        let signature: Signature = Signature::from_bytes(&SIGNATURE_BYTES).unwrap();
        let encoded_signature: Vec<u8> = bincode::serialize(&signature).unwrap();
        let decoded_signature: Signature = bincode::deserialize(&encoded_signature).unwrap();

        assert_eq!(signature, decoded_signature);
    }

    #[test]
    fn serialize_deserialize_signature_json() {
        let signature: Signature = Signature::from_bytes(&SIGNATURE_BYTES).unwrap();
        let encoded_signature = serde_json::to_string(&signature).unwrap();
        let decoded_signature: Signature = serde_json::from_str(&encoded_signature).unwrap();

        assert_eq!(signature, decoded_signature);
    }

    #[test]
    fn serialize_deserialize_public_key_bincode() {
        let public_key: PublicKey = PublicKey::from_bytes(&PUBLIC_KEY_BYTES).unwrap();
        let encoded_public_key: Vec<u8> = bincode::serialize(&public_key).unwrap();
        let decoded_public_key: PublicKey = bincode::deserialize(&encoded_public_key).unwrap();

        assert_eq!(&PUBLIC_KEY_BYTES[..], &encoded_public_key[encoded_public_key.len() - PUBLIC_KEY_LENGTH..]);
        assert_eq!(public_key, decoded_public_key);
    }

    #[test]
    fn serialize_deserialize_public_key_json() {
        let public_key: PublicKey = PublicKey::from_bytes(&PUBLIC_KEY_BYTES).unwrap();
        let encoded_public_key = serde_json::to_string(&public_key).unwrap();
        let decoded_public_key: PublicKey = serde_json::from_str(&encoded_public_key).unwrap();

        assert_eq!(public_key, decoded_public_key);
    }

    #[test]
    fn serialize_deserialize_secret_key_bincode() {
        let secret_key: SecretKey = SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap();
        let encoded_secret_key: Vec<u8> = bincode::serialize(&secret_key).unwrap();
        let decoded_secret_key: SecretKey = bincode::deserialize(&encoded_secret_key).unwrap();

        for i in 0..SECRET_KEY_LENGTH {
            assert_eq!(SECRET_KEY_BYTES[i], decoded_secret_key.as_bytes()[i]);
        }
    }

    #[test]
    fn serialize_deserialize_secret_key_json() {
        let secret_key: SecretKey = SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap();
        let encoded_secret_key = serde_json::to_string(&secret_key).unwrap();
        let decoded_secret_key: SecretKey = serde_json::from_str(&encoded_secret_key).unwrap();

        for i in 0..SECRET_KEY_LENGTH {
            assert_eq!(SECRET_KEY_BYTES[i], decoded_secret_key.as_bytes()[i]);
        }
    }

    #[test]
    fn serialize_deserialize_expanded_secret_key_bincode() {
        let expanded_secret_key = ExpandedSecretKey::from(&SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap());
        let encoded_expanded_secret_key: Vec<u8> = bincode::serialize(&expanded_secret_key).unwrap();
        let decoded_expanded_secret_key: ExpandedSecretKey = bincode::deserialize(&encoded_expanded_secret_key).unwrap();

        for i in 0..EXPANDED_SECRET_KEY_LENGTH {
            assert_eq!(expanded_secret_key.to_bytes()[i], decoded_expanded_secret_key.to_bytes()[i]);
        }
    }

    #[test]
    fn serialize_deserialize_expanded_secret_key_json() {
        let expanded_secret_key = ExpandedSecretKey::from(&SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap());
        let encoded_expanded_secret_key = serde_json::to_string(&expanded_secret_key).unwrap();
        let decoded_expanded_secret_key: ExpandedSecretKey = serde_json::from_str(&encoded_expanded_secret_key).unwrap();

        for i in 0..EXPANDED_SECRET_KEY_LENGTH {
            assert_eq!(expanded_secret_key.to_bytes()[i], decoded_expanded_secret_key.to_bytes()[i]);
        }
    }

    #[test]
    fn serialize_deserialize_keypair_bincode() {
        let keypair = Keypair::from_bytes(&KEYPAIR_BYTES).unwrap();
        let encoded_keypair: Vec<u8> = bincode::serialize(&keypair).unwrap();
        let decoded_keypair: Keypair = bincode::deserialize(&encoded_keypair).unwrap();

        for i in 0..KEYPAIR_LENGTH {
            assert_eq!(KEYPAIR_BYTES[i], decoded_keypair.to_bytes()[i]);
        }
    }

    #[test]
    fn serialize_deserialize_keypair_json() {
        let keypair = Keypair::from_bytes(&KEYPAIR_BYTES).unwrap();
        let encoded_keypair = serde_json::to_string(&keypair).unwrap();
        let decoded_keypair: Keypair = serde_json::from_str(&encoded_keypair).unwrap();

        for i in 0..KEYPAIR_LENGTH {
            assert_eq!(KEYPAIR_BYTES[i], decoded_keypair.to_bytes()[i]);
        }
    }

    #[test]
    fn serialize_deserialize_keypair_toml() {
        let demo = Demo { keypair: Keypair::from_bytes(&KEYPAIR_BYTES).unwrap() };

        println!("\n\nWrite to toml");
        let demo_toml = toml::to_string(&demo).unwrap();
        println!("{}", demo_toml);
        let demo_toml_rebuild: Result<Demo, _> = toml::from_str(&demo_toml);
        println!("{:?}", demo_toml_rebuild);
    }

    #[test]
    fn serialize_public_key_size() {
        let public_key: PublicKey = PublicKey::from_bytes(&PUBLIC_KEY_BYTES).unwrap();
        assert_eq!(bincode::serialized_size(&public_key).unwrap() as usize, BINCODE_INT_LENGTH + PUBLIC_KEY_LENGTH);
    }

    #[test]
    fn serialize_signature_size() {
        let signature: Signature = Signature::from_bytes(&SIGNATURE_BYTES).unwrap();
        assert_eq!(bincode::serialized_size(&signature).unwrap() as usize, SIGNATURE_LENGTH);
    }

    #[test]
    fn serialize_secret_key_size() {
        let secret_key: SecretKey = SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap();
        assert_eq!(bincode::serialized_size(&secret_key).unwrap() as usize, BINCODE_INT_LENGTH + SECRET_KEY_LENGTH);
    }

    #[test]
    fn serialize_expanded_secret_key_size() {
        let expanded_secret_key = ExpandedSecretKey::from(&SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap());
        assert_eq!(bincode::serialized_size(&expanded_secret_key).unwrap() as usize, BINCODE_INT_LENGTH + EXPANDED_SECRET_KEY_LENGTH);
    }

    #[test]
    fn serialize_keypair_size() {
        let keypair = Keypair::from_bytes(&KEYPAIR_BYTES).unwrap();
        assert_eq!(bincode::serialized_size(&keypair).unwrap() as usize, BINCODE_INT_LENGTH + KEYPAIR_LENGTH);
    }
}
