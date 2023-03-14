use crate::validate::*;
use crate::x509::*;
use asn1_rs::Tag;

#[derive(Debug)]
pub struct X509NameStructureValidator;

impl<'a> Validator<'a> for X509NameStructureValidator {
    type Item = X509Name<'a>;

    fn validate<L: Logger>(&self, item: &'a Self::Item, l: &'_ mut L) -> bool {
        let res = true;
        // subject/issuer: verify charsets
        // - wildcards in PrintableString
        // - non-IA5 in IA5String
        for attr in item.iter_attributes() {
            match attr.attr_value().tag() {
                Tag::PrintableString | Tag::Ia5String => {
                    let b = attr.attr_value().as_bytes();
                    if !b.iter().all(u8::is_ascii) {
                        l.warn(&format!(
                            "Invalid charset in X.509 Name, component {}",
                            attr.attr_type()
                        ));
                    }
                }
                _ => (),
            }
        }
        res
    }
}
