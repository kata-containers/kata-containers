use asn1_rs::{Any, Class, FromDer, Length, Result, Tag};
use colored::*;
use nom::HexDisplay;
use oid_registry::{format_oid, Oid as DerOid, OidRegistry};
use std::cmp::min;
use std::error::Error;
use std::marker::PhantomData;
use std::{env, fs};

struct Context<'a> {
    oid_registry: OidRegistry<'a>,
    hex_max: usize,
    t: PhantomData<&'a ()>,
}

impl<'a> Default for Context<'a> {
    fn default() -> Self {
        let oid_registry = OidRegistry::default().with_all_crypto().with_x509();
        Context {
            oid_registry,
            hex_max: 64,
            t: PhantomData,
        }
    }
}

#[macro_export]
macro_rules! indent_println {
    ( $depth: expr, $fmt:expr ) => {
        println!(concat!("{:indent$}",$fmt), "", indent = 2*$depth)
    };
    ( $depth: expr, $fmt:expr, $( $x:expr ),* ) => {
        println!(concat!("{:indent$}",$fmt), "", $($x),*, indent = 2*$depth)
    };
}

#[allow(dead_code)]
pub fn print_hex_dump(bytes: &[u8], max_len: usize) {
    let m = min(bytes.len(), max_len);
    print!("{}", &bytes[..m].to_hex(16));
    if bytes.len() > max_len {
        println!("... <continued>");
    }
}

fn main() -> std::result::Result<(), Box<dyn Error>> {
    let ctx = Context::default();
    for filename in env::args().skip(1) {
        eprintln!("File: {}", filename);
        let content = fs::read(&filename)?;
        // check for PEM file
        if filename.ends_with(".pem") || content.starts_with(b"----") {
            let pems = pem::parse_many(&content).expect("Parsing PEM failed");
            if pems.is_empty() {
                eprintln!("{}", "No PEM section decoded".bright_red());
                continue;
            }
            for (idx, pem) in pems.iter().enumerate() {
                eprintln!("Pem entry {} [{}]", idx, pem.tag.bright_blue());
                print_der(&pem.contents, 1, &ctx);
            }
        } else {
            print_der(&content, 1, &ctx);
        }
    }

    Ok(())
}

fn print_der(i: &[u8], depth: usize, ctx: &Context) {
    match Any::from_der(i) {
        Ok((rem, any)) => {
            print_der_any(any, depth, ctx);
            if !rem.is_empty() {
                let warning = format!("WARNING: {} extra bytes after object", rem.len());
                indent_println!(depth, "{}", warning.bright_red());
                print_hex_dump(rem, ctx.hex_max);
            }
        }
        Err(e) => {
            eprintln!("Error while parsing at depth {}: {:?}", depth, e);
        }
    }
}

fn print_der_result_any(r: Result<Any>, depth: usize, ctx: &Context) {
    match r {
        Ok(any) => print_der_any(any, depth, ctx),
        Err(e) => {
            eprintln!("Error while parsing at depth {}: {:?}", depth, e);
        }
    }
}

fn print_der_any(any: Any, depth: usize, ctx: &Context) {
    let class = match any.header.class() {
        Class::Universal => "UNIVERSAL".to_string().white(),
        c => c.to_string().cyan(),
    };
    let hdr = format!(
        "[c:{} t:{}({}) l:{}]",
        class,
        any.header.tag().0,
        any.header.tag().to_string().white(),
        str_of_length(any.header.length())
    );
    indent_println!(depth, "{}", hdr);
    match any.header.class() {
        Class::Universal => (),
        Class::ContextSpecific | Class::Application => {
            // attempt to decode inner object (if EXPLICIT)
            match Any::from_der(any.data) {
                Ok((rem2, inner)) => {
                    indent_println!(
                        depth + 1,
                        "{} (rem.len={})",
                        format!("EXPLICIT [{}]", any.header.tag().0).green(),
                        // any.header.tag.0,
                        rem2.len()
                    );
                    print_der_any(inner, depth + 2, ctx);
                }
                Err(_) => {
                    // assume tagged IMPLICIT
                    indent_println!(
                        depth + 1,
                        "{}",
                        "could not decode (IMPLICIT tagging?)".bright_red()
                    );
                }
            }
            return;
        }
        _ => {
            indent_println!(
                depth + 1,
                "tagged: [{}] {}",
                any.header.tag().0,
                "*NOT SUPPORTED*".red()
            );
            return;
        }
    }
    match any.header.tag() {
        Tag::BitString => {
            let b = any.bitstring().unwrap();
            indent_println!(depth + 1, "BITSTRING");
            print_hex_dump(b.as_ref(), ctx.hex_max);
        }
        Tag::Boolean => {
            let b = any.bool().unwrap();
            indent_println!(depth + 1, "BOOLEAN: {}", b.to_string().green());
        }
        Tag::EmbeddedPdv => {
            let e = any.embedded_pdv().unwrap();
            indent_println!(depth + 1, "EMBEDDED PDV: {:?}", e);
            print_hex_dump(e.data_value, ctx.hex_max);
        }
        Tag::Enumerated => {
            let i = any.enumerated().unwrap();
            indent_println!(depth + 1, "ENUMERATED: {}", i.0);
        }
        Tag::GeneralizedTime => {
            let s = any.generalizedtime().unwrap();
            indent_println!(depth + 1, "GeneralizedTime: {}", s);
        }
        Tag::GeneralString => {
            let s = any.generalstring().unwrap();
            indent_println!(depth + 1, "GeneralString: {}", s.as_ref());
        }
        Tag::Ia5String => {
            let s = any.ia5string().unwrap();
            indent_println!(depth + 1, "IA5String: {}", s.as_ref());
        }
        Tag::Integer => {
            let i = any.integer().unwrap();
            match i.as_i128() {
                Ok(i) => {
                    indent_println!(depth + 1, "{}", i);
                }
                Err(_) => {
                    print_hex_dump(i.as_ref(), ctx.hex_max);
                }
            }
        }
        Tag::Null => (),
        Tag::OctetString => {
            let b = any.octetstring().unwrap();
            indent_println!(depth + 1, "OCTETSTRING");
            print_hex_dump(b.as_ref(), ctx.hex_max);
        }
        Tag::Oid => {
            let oid = any.oid().unwrap();
            let der_oid = DerOid::new(oid.as_bytes().into());
            indent_println!(
                depth + 1,
                "OID: {}",
                format_oid(&der_oid, &ctx.oid_registry).cyan()
            );
        }
        Tag::PrintableString => {
            let s = any.printablestring().unwrap();
            indent_println!(depth + 1, "PrintableString: {}", s.as_ref());
        }
        Tag::RelativeOid => {
            let oid = any.oid().unwrap();
            let der_oid = DerOid::new(oid.as_bytes().into());
            indent_println!(
                depth + 1,
                "RELATIVE-OID: {}",
                format_oid(&der_oid, &ctx.oid_registry).cyan()
            );
        }
        Tag::Set => {
            let seq = any.set().unwrap();
            for item in seq.der_iter::<Any, asn1_rs::Error>() {
                print_der_result_any(item, depth + 1, ctx);
            }
        }
        Tag::Sequence => {
            let seq = any.sequence().unwrap();
            for item in seq.der_iter::<Any, asn1_rs::Error>() {
                print_der_result_any(item, depth + 1, ctx);
            }
        }
        Tag::UtcTime => {
            let s = any.utctime().unwrap();
            indent_println!(depth + 1, "UtcTime: {}", s);
        }
        Tag::Utf8String => {
            let s = any.utf8string().unwrap();
            indent_println!(depth + 1, "UTF-8: {}", s.as_ref());
        }
        _ => unimplemented!("unsupported tag {}", any.header.tag()),
    }
}

fn str_of_length(l: Length) -> String {
    match l {
        Length::Definite(l) => l.to_string(),
        Length::Indefinite => "Indefinite".to_string(),
    }
}
