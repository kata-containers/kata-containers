use super::{BerObject, BerObjectContent, BitStringObject};
use alloc::vec::Vec;
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
/// Entry point: use the [`VisitMut::run`] or [`VisitMut::run_at`] methods.
///
/// Visitor functions
#[allow(unused_variables)]
pub trait VisitMut<'a> {
    /// Called for every BER object
    fn visit_ber_mut(&mut self, ber: &'_ mut BerObject<'a>, depth: usize) {}

    /// Called for BER bitstring objects
    fn visit_ber_bitstring_mut(
        &mut self,
        ignored: &mut u8,
        data: &'a mut BitStringObject,
        depth: usize,
    ) {
    }

    /// Called for BER bmpstring objects
    fn visit_ber_bmpstring_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER boolean objects
    fn visit_ber_boolean_mut(&mut self, b: &'a mut bool, depth: usize) {}

    /// Called for BER end-of-content objects
    fn visit_ber_endofcontent_mut(&mut self, depth: usize) {}

    /// Called for BER enum objects
    fn visit_ber_enum_mut(&mut self, e: &'a mut u64, depth: usize) {}

    /// Called for BER generalstring objects
    fn visit_ber_generalstring_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER generalizedtime objects
    fn visit_ber_generalizedtime_mut(&mut self, t: &'a ASN1DateTime, depth: usize) {}

    /// Called for BER graphicstring objects
    fn visit_ber_graphicstring_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER ia5string objects
    fn visit_ber_ia5string_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER integer objects
    fn visit_ber_integer_mut(&mut self, raw_bytes: &'a mut &'_ [u8], depth: usize) {}

    /// Called for BER null objects
    fn visit_ber_null_mut(&mut self, depth: usize) {}

    /// Called for BER numericstring objects
    fn visit_ber_numericstring_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER OID objects
    fn visit_ber_oid_mut(&mut self, oid: &'a mut Oid, depth: usize) {}

    /// Called for BER object descriptor objects
    fn visit_ber_objectdescriptor_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER octetstring objects
    fn visit_ber_octetstring_mut(&mut self, b: &'a [u8], depth: usize) {}

    /// Called for BER optional objects
    fn visit_ber_optional_mut(&mut self, obj: Option<&'a mut BerObject<'a>>, depth: usize) {}

    /// Called for BER printablestring objects
    fn visit_ber_printablestring_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER relative OID objects
    fn visit_ber_relative_oid_mut(&mut self, oid: &'a mut Oid, depth: usize) {}

    /// Called for BER sequence objects
    fn visit_ber_sequence_mut(&mut self, l: &'_ mut Vec<BerObject<'a>>, depth: usize) {}

    /// Called for BER set objects
    fn visit_ber_set_mut(&mut self, ber: &'_ mut Vec<BerObject<'a>>, depth: usize) {}

    /// Called for BER teletexstring objects
    fn visit_ber_teletexstring_mut(&mut self, s: &'a mut &'_ str, depth: usize) {}

    /// Called for BER tagged objects
    fn visit_ber_tagged_mut(
        &mut self,
        class: &'a mut Class,
        tag: &'a mut Tag,
        obj: &'a mut BerObject<'a>,
        depth: usize,
    ) {
    }

    /// Called for BER generalizedtime objects
    fn visit_ber_utctime_mut(&mut self, t: &'a ASN1DateTime, depth: usize) {}

    /// Called for BER utf8string objects
    fn visit_ber_utf8string_mut(&mut self, s: &'a str, depth: usize) {}

    /// Called for BER universalstring objects
    fn visit_ber_universalstring_mut(&mut self, raw_bytes: &'a mut &'_ [u8], depth: usize) {}

    /// Called for BER videotexstring objects
    fn visit_ber_videotextstring_mut(&mut self, raw_bytes: &'a mut &'_ str, depth: usize) {}

    /// Called for BER visiblestring objects
    fn visit_ber_visiblestring_mut(&mut self, raw_bytes: &'a mut &'_ str, depth: usize) {}

    /// Called for BER unknown objects
    fn visit_ber_unknown_mut(&mut self, ber: &'_ mut Any<'a>, depth: usize) {}

    /// Perform a BFS traversal of the BER object, calling the visitor functions during he traversal
    ///
    /// Usually, this method should not be redefined (unless implementing a custom traversal)
    fn run(&mut self, ber: &'a mut BerObject<'a>) {
        visit_ber_bfs_mut(self, ber, 0)
    }

    /// Perform a BFS traversal of the BER object, calling the visitor functions during he traversal
    ///
    /// Start at specified depth.
    ///
    /// Usually, this method should not be redefined (unless implementing a custom traversal)
    fn run_at(&mut self, ber: &'a mut BerObject<'a>, depth: usize) {
        visit_ber_bfs_mut(self, ber, depth)
    }
}

fn visit_ber_bfs_mut<'a, V>(v: &mut V, ber: &'a mut BerObject<'a>, depth: usize)
where
    V: VisitMut<'a> + ?Sized,
{
    v.visit_ber_mut(ber, depth);

    match ber.content {
        BerObjectContent::BitString(ref mut ignored, ref mut data) => {
            v.visit_ber_bitstring_mut(ignored, data, depth);
        }
        BerObjectContent::BmpString(ref mut s) => v.visit_ber_bmpstring_mut(s, depth),
        BerObjectContent::Boolean(ref mut b) => v.visit_ber_boolean_mut(b, depth),
        BerObjectContent::EndOfContent => v.visit_ber_endofcontent_mut(depth),
        BerObjectContent::Enum(ref mut val) => v.visit_ber_enum_mut(val, depth),
        BerObjectContent::GeneralString(ref mut s) => v.visit_ber_generalstring_mut(s, depth),
        BerObjectContent::GeneralizedTime(ref t) => v.visit_ber_generalizedtime_mut(t, depth),
        BerObjectContent::GraphicString(ref mut s) => v.visit_ber_graphicstring_mut(s, depth),
        BerObjectContent::IA5String(ref mut s) => v.visit_ber_ia5string_mut(s, depth),
        BerObjectContent::Integer(ref mut s) => v.visit_ber_integer_mut(s, depth),
        BerObjectContent::Null => v.visit_ber_null_mut(depth),
        BerObjectContent::NumericString(ref mut s) => v.visit_ber_numericstring_mut(s, depth),
        BerObjectContent::OID(ref mut oid) => v.visit_ber_oid_mut(oid, depth),
        BerObjectContent::ObjectDescriptor(ref mut s) => v.visit_ber_objectdescriptor_mut(s, depth),
        BerObjectContent::OctetString(ref mut b) => v.visit_ber_octetstring_mut(b, depth),
        BerObjectContent::Optional(ref mut obj) => {
            let opt = obj.as_mut().map(|b| b.as_mut());
            v.visit_ber_optional_mut(opt, depth)
        }
        BerObjectContent::PrintableString(ref mut s) => v.visit_ber_printablestring_mut(s, depth),
        BerObjectContent::RelativeOID(ref mut oid) => v.visit_ber_relative_oid_mut(oid, depth),
        BerObjectContent::Sequence(ref mut l) => {
            v.visit_ber_sequence_mut(l, depth);
            for item in l.iter_mut() {
                visit_ber_bfs_mut(v, item, depth + 1);
            }
        }
        BerObjectContent::Set(ref mut l) => {
            v.visit_ber_set_mut(l, depth);
            for item in l.iter_mut() {
                visit_ber_bfs_mut(v, item, depth + 1);
            }
        }
        BerObjectContent::T61String(ref mut s) => v.visit_ber_teletexstring_mut(s, depth),
        BerObjectContent::Tagged(ref mut class, ref mut tag, ref mut obj) => {
            v.visit_ber_tagged_mut(class, tag, obj.as_mut(), depth)
        }
        BerObjectContent::UTCTime(ref t) => v.visit_ber_utctime_mut(t, depth),
        BerObjectContent::UTF8String(ref mut s) => v.visit_ber_utf8string_mut(s, depth),
        BerObjectContent::UniversalString(ref mut b) => v.visit_ber_universalstring_mut(b, depth),
        BerObjectContent::Unknown(ref mut inner) => v.visit_ber_unknown_mut(inner, depth),
        BerObjectContent::VideotexString(ref mut s) => v.visit_ber_videotextstring_mut(s, depth),
        BerObjectContent::VisibleString(ref mut s) => v.visit_ber_visiblestring_mut(s, depth),
    }
}

#[cfg(test)]
mod tests {
    use super::VisitMut;
    use crate::ber::BerObject;

    #[derive(Debug)]
    struct BerObjectVisitor {}

    impl<'a> VisitMut<'a> for BerObjectVisitor {
        fn visit_ber_mut(&mut self, ber: &'_ mut BerObject<'a>, depth: usize) {
            eprintln!("Depth {}: Object with tag {}", depth, ber.tag());
        }
    }
}
