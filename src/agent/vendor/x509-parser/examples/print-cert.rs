use der_parser::der::Tag;
use der_parser::oid::Oid;
use nom::HexDisplay;
use std::cmp::min;
use std::convert::TryFrom;
use std::env;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
use x509_parser::prelude::*;
use x509_parser::public_key::PublicKey;
use x509_parser::signature_algorithm::SignatureAlgorithm;

const PARSE_ERRORS_FATAL: bool = false;
#[cfg(feature = "validate")]
const VALIDATE_ERRORS_FATAL: bool = false;

fn print_hex_dump(bytes: &[u8], max_len: usize) {
    let m = min(bytes.len(), max_len);
    print!("{}", &bytes[..m].to_hex(16));
    if bytes.len() > max_len {
        println!("... <continued>");
    }
}

fn format_oid(oid: &Oid) -> String {
    match oid2sn(oid, oid_registry()) {
        Ok(s) => s.to_owned(),
        _ => format!("{}", oid),
    }
}

fn generalname_to_string(gn: &GeneralName) -> String {
    match gn {
        GeneralName::DNSName(name) => format!("DNSName:{}", name),
        GeneralName::DirectoryName(n) => format!("DirName:{}", n),
        GeneralName::EDIPartyName(obj) => format!("EDIPartyName:{:?}", obj),
        GeneralName::IPAddress(n) => format!("IPAddress:{:?}", n),
        GeneralName::OtherName(oid, n) => format!("OtherName:{}, {:?}", oid, n),
        GeneralName::RFC822Name(n) => format!("RFC822Name:{}", n),
        GeneralName::RegisteredID(oid) => format!("RegisteredID:{}", oid),
        GeneralName::URI(n) => format!("URI:{}", n),
        GeneralName::X400Address(obj) => format!("X400Address:{:?}", obj),
    }
}

fn print_x509_extension(oid: &Oid, ext: &X509Extension) {
    println!(
        "    [crit:{} l:{}] {}: ",
        ext.critical,
        ext.value.len(),
        format_oid(oid)
    );
    match ext.parsed_extension() {
        ParsedExtension::AuthorityKeyIdentifier(aki) => {
            println!("      X509v3 Authority Key Identifier");
            if let Some(key_id) = &aki.key_identifier {
                println!("        Key Identifier: {:x}", key_id);
            }
            if let Some(issuer) = &aki.authority_cert_issuer {
                for name in issuer {
                    println!("        Cert Issuer: {}", name);
                }
            }
            if let Some(serial) = aki.authority_cert_serial {
                println!("        Cert Serial: {}", format_serial(serial));
            }
        }
        ParsedExtension::BasicConstraints(bc) => {
            println!("      X509v3 CA: {}", bc.ca);
        }
        ParsedExtension::CRLDistributionPoints(points) => {
            println!("      X509v3 CRL Distribution Points:");
            for point in points {
                if let Some(name) = &point.distribution_point {
                    println!("        Full Name: {:?}", name);
                }
                if let Some(reasons) = &point.reasons {
                    println!("        Reasons: {}", reasons);
                }
                if let Some(crl_issuer) = &point.crl_issuer {
                    print!("        CRL Issuer: ");
                    for gn in crl_issuer {
                        print!("{} ", generalname_to_string(gn));
                    }
                    println!();
                }
                println!();
            }
        }
        ParsedExtension::KeyUsage(ku) => {
            println!("      X509v3 Key Usage: {}", ku);
        }
        ParsedExtension::NSCertType(ty) => {
            println!("      Netscape Cert Type: {}", ty);
        }
        ParsedExtension::SubjectAlternativeName(san) => {
            for name in &san.general_names {
                let s = match name {
                    GeneralName::DNSName(s) => {
                        format!("DNS:{}", s)
                    }
                    GeneralName::IPAddress(b) => {
                        let ip = match b.len() {
                            4 => {
                                let b = <[u8; 4]>::try_from(*b).unwrap();
                                let ip = Ipv4Addr::from(b);
                                format!("{}", ip)
                            }
                            16 => {
                                let b = <[u8; 16]>::try_from(*b).unwrap();
                                let ip = Ipv6Addr::from(b);
                                format!("{}", ip)
                            }
                            l => format!("invalid (len={})", l),
                        };
                        format!("IP Address:{}", ip)
                    }
                    _ => {
                        format!("{:?}", name)
                    }
                };
                println!("      X509v3 SAN: {}", s);
            }
        }
        ParsedExtension::SubjectKeyIdentifier(id) => {
            println!("      X509v3 Subject Key Identifier: {:x}", id);
        }
        x => println!("      {:?}", x),
    }
}

fn print_x509_digest_algorithm(alg: &AlgorithmIdentifier, level: usize) {
    println!(
        "{:indent$}Oid: {}",
        "",
        format_oid(&alg.algorithm),
        indent = level
    );
    if let Some(parameter) = &alg.parameters {
        let s = match parameter.tag() {
            Tag::Oid => {
                let oid = parameter.as_oid().unwrap();
                format_oid(&oid)
            }
            _ => format!("{}", parameter.tag()),
        };
        println!("{:indent$}Parameter: <PRESENT> {}", "", s, indent = level);
        let bytes = parameter.as_bytes();
        print_hex_dump(bytes, 32);
    } else {
        println!("{:indent$}Parameter: <ABSENT>", "", indent = level);
    }
}

fn print_x509_info(x509: &X509Certificate) -> io::Result<()> {
    let version = x509.version();
    if version.0 < 3 {
        println!("  Version: {}", version);
    } else {
        println!("  Version: INVALID({})", version.0);
    }
    println!("  Serial: {}", x509.tbs_certificate.raw_serial_as_string());
    println!("  Subject: {}", x509.subject());
    println!("  Issuer: {}", x509.issuer());
    println!("  Validity:");
    println!("    NotBefore: {}", x509.validity().not_before);
    println!("    NotAfter:  {}", x509.validity().not_after);
    println!("    is_valid:  {}", x509.validity().is_valid());
    println!("  Subject Public Key Info:");
    print_x509_ski(x509.public_key());
    print_x509_signature_algorithm(&x509.signature_algorithm, 4);

    println!("  Signature Value:");
    for l in format_number_to_hex_with_colon(&x509.signature_value.data, 16) {
        println!("      {}", l);
    }
    println!("  Extensions:");
    for ext in x509.extensions() {
        print_x509_extension(&ext.oid, ext);
    }
    println!();
    print!("Structure validation status: ");
    #[cfg(feature = "validate")]
    {
        let mut logger = VecLogger::default();
        // structure validation status
        let ok = X509StructureValidator
            .chain(X509CertificateValidator)
            .validate(x509, &mut logger);
        if ok {
            println!("Ok");
        } else {
            println!("FAIL");
        }
        for warning in logger.warnings() {
            println!("  [W] {}", warning);
        }
        for error in logger.errors() {
            println!("  [E] {}", error);
        }
        println!();
        if VALIDATE_ERRORS_FATAL && !logger.errors().is_empty() {
            return Err(io::Error::new(io::ErrorKind::Other, "validation failed"));
        }
    }
    #[cfg(not(feature = "validate"))]
    {
        println!("Unknown (feature 'validate' not enabled)");
    }
    #[cfg(feature = "verify")]
    {
        print!("Signature verification: ");
        if x509.subject() == x509.issuer() {
            if x509.verify_signature(None).is_ok() {
                println!("OK");
                println!("  [I] certificate is self-signed");
            } else if x509.subject() == x509.issuer() {
                println!("FAIL");
                println!("  [W] certificate looks self-signed, but signature verification failed");
            }
        } else {
            // if subject is different from issuer, we cannot verify certificate without the public key of the issuer
            println!("N/A");
        }
    }
    Ok(())
}

fn print_x509_signature_algorithm(signature_algorithm: &AlgorithmIdentifier, indent: usize) {
    match SignatureAlgorithm::try_from(signature_algorithm) {
        Ok(sig_alg) => {
            print!("  Signature Algorithm: ");
            match sig_alg {
                SignatureAlgorithm::DSA => println!("DSA"),
                SignatureAlgorithm::ECDSA => println!("ECDSA"),
                SignatureAlgorithm::ED25519 => println!("ED25519"),
                SignatureAlgorithm::RSA => println!("RSA"),
                SignatureAlgorithm::RSASSA_PSS(params) => {
                    println!("RSASSA-PSS");
                    let indent_s = format!("{:indent$}", "", indent = indent + 2);
                    println!(
                        "{}Hash Algorithm: {}",
                        indent_s,
                        format_oid(params.hash_algorithm_oid()),
                    );
                    print!("{}Mask Generation Function: ", indent_s);
                    if let Ok(mask_gen) = params.mask_gen_algorithm() {
                        println!(
                            "{}/{}",
                            format_oid(&mask_gen.mgf),
                            format_oid(&mask_gen.hash),
                        );
                    } else {
                        println!("INVALID");
                    }
                    println!("{}Salt Length: {}", indent_s, params.salt_length());
                }
                SignatureAlgorithm::RSAAES_OAEP(params) => {
                    println!("RSAAES-OAEP");
                    let indent_s = format!("{:indent$}", "", indent = indent + 2);
                    println!(
                        "{}Hash Algorithm: {}",
                        indent_s,
                        format_oid(params.hash_algorithm_oid()),
                    );
                    print!("{}Mask Generation Function: ", indent_s);
                    if let Ok(mask_gen) = params.mask_gen_algorithm() {
                        println!(
                            "{}/{}",
                            format_oid(&mask_gen.mgf),
                            format_oid(&mask_gen.hash),
                        );
                    } else {
                        println!("INVALID");
                    }
                    println!(
                        "{}pSourceFunc: {}",
                        indent_s,
                        format_oid(&params.p_source_alg().algorithm),
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Could not parse signature algorithm: {}", e);
            println!("  Signature Algorithm:");
            print_x509_digest_algorithm(signature_algorithm, indent);
        }
    }
}

fn print_x509_ski(public_key: &SubjectPublicKeyInfo) {
    println!("    Public Key Algorithm:");
    print_x509_digest_algorithm(&public_key.algorithm, 6);
    match public_key.parsed() {
        Ok(PublicKey::RSA(rsa)) => {
            println!("    RSA Public Key: ({} bit)", rsa.key_size());
            // print_hex_dump(rsa.modulus, 1024);
            for l in format_number_to_hex_with_colon(rsa.modulus, 16) {
                println!("        {}", l);
            }
            if let Ok(e) = rsa.try_exponent() {
                println!("    exponent: 0x{:x} ({})", e, e);
            } else {
                println!("    exponent: <INVALID>:");
                print_hex_dump(rsa.exponent, 32);
            }
        }
        Ok(PublicKey::EC(ec)) => {
            println!("    EC Public Key: ({} bit)", ec.key_size());
            for l in format_number_to_hex_with_colon(ec.data(), 16) {
                println!("        {}", l);
            }
            // // identify curve
            // if let Some(params) = &public_key.algorithm.parameters {
            //     let curve_oid = params.as_oid();
            //     let curve = curve_oid
            //         .map(|oid| {
            //             oid_registry()
            //                 .get(oid)
            //                 .map(|entry| entry.sn())
            //                 .unwrap_or("<UNKNOWN>")
            //         })
            //         .unwrap_or("<ERROR: NOT AN OID>");
            //     println!("    Curve: {}", curve);
            // }
        }
        Ok(PublicKey::DSA(y)) => {
            println!("    DSA Public Key: ({} bit)", 8 * y.len());
            for l in format_number_to_hex_with_colon(y, 16) {
                println!("        {}", l);
            }
        }
        Ok(PublicKey::GostR3410(y)) => {
            println!("    GOST R 34.10-94 Public Key: ({} bit)", 8 * y.len());
            for l in format_number_to_hex_with_colon(y, 16) {
                println!("        {}", l);
            }
        }
        Ok(PublicKey::GostR3410_2012(y)) => {
            println!("    GOST R 34.10-2012 Public Key: ({} bit)", 8 * y.len());
            for l in format_number_to_hex_with_colon(y, 16) {
                println!("        {}", l);
            }
        }
        Ok(PublicKey::Unknown(b)) => {
            println!("    Unknown key type");
            print_hex_dump(b, 256);
            if let Ok((rem, res)) = der_parser::parse_der(b) {
                eprintln!("rem: {} bytes", rem.len());
                eprintln!("{:?}", res);
            } else {
                eprintln!("      <Could not parse key as DER>");
            }
        }
        Err(_) => {
            println!("    INVALID PUBLIC KEY");
        }
    }
    // dbg!(&public_key);
    // todo!();
}

fn format_number_to_hex_with_colon(b: &[u8], row_size: usize) -> Vec<String> {
    let mut v = Vec::with_capacity(1 + b.len() / row_size);
    for r in b.chunks(row_size) {
        let s = r.iter().fold(String::with_capacity(3 * r.len()), |a, b| {
            a + &format!("{:02x}:", b)
        });
        v.push(s)
    }
    v
}

fn handle_certificate(file_name: &str, data: &[u8]) -> io::Result<()> {
    match parse_x509_certificate(data) {
        Ok((_, x509)) => {
            print_x509_info(&x509)?;
            Ok(())
        }
        Err(e) => {
            let s = format!("Error while parsing {}: {}", file_name, e);
            if PARSE_ERRORS_FATAL {
                Err(io::Error::new(io::ErrorKind::Other, s))
            } else {
                eprintln!("{}", s);
                Ok(())
            }
        }
    }
}

pub fn main() -> io::Result<()> {
    for file_name in env::args().skip(1) {
        println!("File: {}", file_name);
        let data = std::fs::read(file_name.clone()).expect("Unable to read file");
        if matches!((data[0], data[1]), (0x30, 0x81..=0x83)) {
            // probably DER
            handle_certificate(&file_name, &data)?;
        } else {
            // try as PEM
            for (n, pem) in Pem::iter_from_buffer(&data).enumerate() {
                match pem {
                    Ok(pem) => {
                        let data = &pem.contents;
                        println!("Certificate [{}]", n);
                        handle_certificate(&file_name, data)?;
                    }
                    Err(e) => {
                        eprintln!("Error while decoding PEM entry {}: {}", n, e);
                    }
                }
            }
        }
    }
    Ok(())
}
