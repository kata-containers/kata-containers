use super::{BerObject, BerObjectContent, BitStringObject};
use asn1_rs::{ASN1DateTime, Any, Class, Oid, Tag};

/// BER object tree traversal to walk a shared borrow of a BER object
///
/// When implementing your own visitor, define your own `visit_ber_xxx` methods.
///
/// Note that `visit_ber` is called for every object, so if you implement multiple visitor methods they
/// will be called multiple times for the same object. Generally, if `visit_ber` is implemented, then other
/// methods are not needed.
///
/// For example, on a `Sequence` item, `visit_ber` is called first, then `visit_ber_sequence`, and then
/// `visit_ber` for every sequence object (recursively).
///
/// Entry point: use the [`Visit::run`] or [`Visit::run_at`] methods.
///
/// Visitor functions
#[allow(unused_variables)]
pub trait Visit<'a> {
    /// Called for every BER object
    fn visit_ber(&mut self, ber: &'_ BerObject<'a>, depth: usize) {}

    /// Called for BER bitstring objects
    fn visit_ber_bitstring(&mut self, ignored: u8, data: &'a BitStringObject, depth: usize) {}

    /// Called for BER bmpstring objects
    fn visit_ber_bmpstring(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER boolean objects
    fn visit_ber_boolean(&mut self, b: bool, depth: usize) {}

    /// Called for BER end-of-content objects
    fn visit_ber_endofcontent(&mut self, depth: usize) {}

    /// Called for BER enum objects
    fn visit_ber_enum(&mut self, e: u64, depth: usize) {}

    /// Called for BER generalstring objects
    fn visit_ber_generalstring(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER generalizedtime objects
    fn visit_ber_generalizedtime(&mut self, t: &'a ASN1DateTime, depth: usize) {}

    /// Called for BER graphicstring objects
    fn visit_ber_graphicstring(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER ia5string objects
    fn visit_ber_ia5string(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER integer objects
    fn visit_ber_integer(&mut self, raw_bytes: &'a [u8], depth: usize) {}

    /// Called for BER null objects
    fn visit_ber_null(&mut self, depth: usize) {}

    /// Called for BER numericstring objects
    fn visit_ber_numericstring(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER OID objects
    fn visit_ber_oid(&mut self, oid: &'a Oid, depth: usize) {}

    /// Called for BER object descriptor objects
    fn visit_ber_objectdescriptor(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER octetstring objects
    fn visit_ber_octetstring(&mut self, b: &'a [u8], depth: usize) {}

    /// Called for BER optional objects
    fn visit_ber_optional(&mut self, obj: Option<&'a BerObject<'a>>, depth: usize) {}

    /// Called for BER printablestring objects
    fn visit_ber_printablestring(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER relative OID objects
    fn visit_ber_relative_oid(&mut self, oid: &'a Oid, depth: usize) {}

    /// Called for BER sequence objects
    fn visit_ber_sequence(&mut self, ber: &'_ [BerObject<'a>], depth: usize) {}

    /// Called for BER set objects
    fn visit_ber_set(&mut self, ber: &'_ [BerObject<'a>], depth: usize) {}

    /// Called for BER teletexstring objects
    fn visit_ber_teletexstring(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER tagged objects
    fn visit_ber_tagged(&mut self, class: Class, tag: Tag, obj: &'_ BerObject<'a>, depth: usize) {}

    /// Called for BER generalizedtime objects
    fn visit_ber_utctime(&mut self, t: &'a ASN1DateTime, depth: usize) {}

    /// Called for BER utf8string objects
    fn visit_ber_utf8string(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER universalstring objects
    fn visit_ber_universalstring(&mut self, raw_bytes: &'a [u8], depth: usize) {}

    /// Called for BER videotexstring objects
    fn visit_ber_videotextstring(&mut self, raw_bytes: &'a str, depth: usize) {}

    /// Called for BER visiblestring objects
    fn visit_ber_visiblestring(&mut self, raw_bytes: &'a str, depth: usize) {}

    /// Called for BER unknown objects
    fn visit_ber_unknown(&mut self, ber: &'_ Any<'a>, depth: usize) {}

    /// Perform a BFS traversal of the BER object, calling the visitor functions during he traversal
    ///
    /// Usually, this method should not be redefined (unless implementing a custom traversal)
    fn run(&mut self, ber: &'a BerObject<'a>) {
        visit_ber_bfs(self, ber, 0)
    }

    /// Perform a BFS traversal of the BER object, calling the visitor functions during he traversal
    ///
    /// Start at specified depth.
    ///
    /// Usually, this method should not be redefined (unless implementing a custom traversal)
    fn run_at(&mut self, ber: &'a BerObject<'a>, depth: usize) {
        visit_ber_bfs(self, ber, depth)
    }
}

fn visit_ber_bfs<'a, V>(v: &mut V, ber: &'a BerObject<'a>, depth: usize)
where
    V: Visit<'a> + ?Sized,
{
    v.visit_ber(ber, depth);

    match ber.content {
        BerObjectContent::BitString(ignored, ref data) => {
            v.visit_ber_bitstring(ignored, data, depth);
        }
        BerObjectContent::BmpString(s) => v.visit_ber_bmpstring(s, depth),
        BerObjectContent::Boolean(b) => v.visit_ber_boolean(b, depth),
        BerObjectContent::EndOfContent => v.visit_ber_endofcontent(depth),
        BerObjectContent::Enum(val) => v.visit_ber_enum(val, depth),
        BerObjectContent::GeneralString(s) => v.visit_ber_generalstring(s, depth),
        BerObjectContent::GeneralizedTime(ref t) => v.visit_ber_generalizedtime(t, depth),
        BerObjectContent::GraphicString(s) => v.visit_ber_graphicstring(s, depth),
        BerObjectContent::IA5String(s) => v.visit_ber_ia5string(s, depth),
        BerObjectContent::Integer(s) => v.visit_ber_integer(s, depth),
        BerObjectContent::Null => v.visit_ber_null(depth),
        BerObjectContent::NumericString(s) => v.visit_ber_numericstring(s, depth),
        BerObjectContent::OID(ref oid) => v.visit_ber_oid(oid, depth),
        BerObjectContent::ObjectDescriptor(s) => v.visit_ber_objectdescriptor(s, depth),
        BerObjectContent::OctetString(b) => v.visit_ber_octetstring(b, depth),
        BerObjectContent::Optional(ref obj) => {
            let opt = obj.as_ref().map(|b| b.as_ref());
            v.visit_ber_optional(opt, depth)
        }
        BerObjectContent::PrintableString(s) => v.visit_ber_printablestring(s, depth),
        BerObjectContent::RelativeOID(ref oid) => v.visit_ber_relative_oid(oid, depth),
        BerObjectContent::Sequence(ref l) => {
            v.visit_ber_sequence(l, depth);
            for item in l.iter() {
                visit_ber_bfs(v, item, depth + 1);
            }
        }
        BerObjectContent::Set(ref l) => {
            v.visit_ber_set(l, depth);
            for item in l.iter() {
                visit_ber_bfs(v, item, depth + 1);
            }
        }
        BerObjectContent::T61String(s) => v.visit_ber_teletexstring(s, depth),
        BerObjectContent::Tagged(class, tag, ref obj) => {
            v.visit_ber_tagged(class, tag, obj.as_ref(), depth)
        }
        BerObjectContent::UTCTime(ref t) => v.visit_ber_utctime(t, depth),
        BerObjectContent::UTF8String(s) => v.visit_ber_utf8string(s, depth),
        BerObjectContent::UniversalString(b) => v.visit_ber_universalstring(b, depth),
        BerObjectContent::Unknown(ref inner) => v.visit_ber_unknown(inner, depth),
        BerObjectContent::VideotexString(s) => v.visit_ber_videotextstring(s, depth),
        BerObjectContent::VisibleString(s) => v.visit_ber_visiblestring(s, depth),
    }
}

#[cfg(test)]
mod tests {
    use super::Visit;
    use crate::ber::BerObject;

    #[derive(Debug)]
    struct BerObjectVisitor {}

    impl<'a> Visit<'a> for BerObjectVisitor {
        fn visit_ber(&mut self, ber: &'_ BerObject<'a>, depth: usize) {
            eprintln!("Depth {}: Object with tag {}", depth, ber.tag());
        }
    }
}
