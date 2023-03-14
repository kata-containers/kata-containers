//! This example prints all algorithms supported by the currently
//! selected cryptographic backend.

use sequoia_openpgp as openpgp;
use openpgp::types::*;
use openpgp::cert::CipherSuite;

fn main() {
    println!("Cipher suites:");
    for a in &[
        CipherSuite::Cv25519,
        CipherSuite::P256,
        CipherSuite::P384,
        CipherSuite::P521,
        CipherSuite::RSA2k,
        CipherSuite::RSA3k,
        CipherSuite::RSA4k,
    ] {
        println!(" - {:70} {:?}", format!("{:?}", a), a.is_supported().is_ok());
    }
    println!();

    println!("Public-Key algorithms:");
    for a in &[
        PublicKeyAlgorithm::RSAEncryptSign,
        PublicKeyAlgorithm::ElGamalEncrypt,
        PublicKeyAlgorithm::DSA,
        PublicKeyAlgorithm::ECDH,
        PublicKeyAlgorithm::ECDSA,
        PublicKeyAlgorithm::EdDSA,
    ] {
        println!(" - {:70} {:?}", a.to_string(), a.is_supported());
    }
    println!();

    println!("ECC algorithms:");
    for a in &[
        Curve::NistP256,
        Curve::NistP384,
        Curve::NistP521,
        Curve::BrainpoolP256,
        Curve::BrainpoolP512,
        Curve::Ed25519,
        Curve::Cv25519,
    ] {
        println!(" - {:70} {:?}", a.to_string(), a.is_supported());
    }
    println!();

    println!("Symmetric algorithms:");
    for a in &[
        SymmetricAlgorithm::IDEA,
        SymmetricAlgorithm::TripleDES,
        SymmetricAlgorithm::CAST5,
        SymmetricAlgorithm::Blowfish,
        SymmetricAlgorithm::AES128,
        SymmetricAlgorithm::AES192,
        SymmetricAlgorithm::AES256,
        SymmetricAlgorithm::Twofish,
        SymmetricAlgorithm::Camellia128,
        SymmetricAlgorithm::Camellia192,
        SymmetricAlgorithm::Camellia256,
    ] {
        println!(" - {:70} {:?}", a.to_string(), a.is_supported());
    }
    println!();

    println!("AEAD algorithms:");
    for a in &[
        AEADAlgorithm::EAX,
        AEADAlgorithm::OCB,
    ] {
        println!(" - {:70} {:?}", a.to_string(), a.is_supported());
    }
    println!();

    println!("Hash algorithms:");
    for a in &[
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
        HashAlgorithm::RipeMD,
        HashAlgorithm::SHA256,
        HashAlgorithm::SHA384,
        HashAlgorithm::SHA512,
        HashAlgorithm::SHA224,
    ] {
        println!(" - {:70} {:?}", a.to_string(), a.is_supported());
    }
    println!();

    println!("Compression algorithms:");
    for a in &[
        CompressionAlgorithm::Zip,
        CompressionAlgorithm::Zlib,
        CompressionAlgorithm::BZip2,
    ] {
        println!(" - {:70} {:?}", a.to_string(), a.is_supported());
    }
    println!();
}
