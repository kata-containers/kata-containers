//! Module to serialize and enarmor a Cert and add informative headers.
use std::io;
use std::str;

use crate::armor;
use crate::cert::{Cert, amalgamation::ValidAmalgamation};
use crate::Result;
use crate::types::RevocationStatus;
use crate::seal;
use crate::serialize::{
    Marshal, MarshalInto,
    generic_serialize_into, generic_export_into,
    TSK,
};
use crate::policy::StandardPolicy as P;


/// Whether or not a character is printable.
pub(crate) fn is_printable(c: &char) -> bool {
    // c.is_ascii_alphanumeric || c.is_whitespace || c.is_ascii_punctuation
    // would exclude any utf8 character, so it seems that to obtain all
    // printable chars, it works just excluding the control chars.
    !c.is_control() && !c.is_ascii_control()
}

impl Cert {
    /// Creates descriptive armor headers.
    ///
    /// Returns armor headers that describe this Cert.  The Cert's
    /// primary fingerprint and valid userids (according to the
    /// default policy) are included as comments, so that it is easier
    /// to identify the Cert when looking at the armored data.
    pub fn armor_headers(&self) -> Vec<String> {
        let p = &P::default();

        let length_value = armor::LINE_LENGTH - "Comment: ".len();
        // Create a header per userid.
        let mut headers: Vec<String> = self.userids().with_policy(p, None)
            // Ignore revoked userids.
            .filter(|uidb| {
                !matches!(uidb.revocation_status(), RevocationStatus::Revoked(_))
            // Ignore userids with non-printable characters.
            }).filter_map(|uidb| {
                let value = str::from_utf8(uidb.userid().value()).ok()?;
                for c in value.chars().take(length_value) {
                    if !is_printable(&c){
                        return None;
                    }
                }
                // Make sure the line length does not exceed armor::LINE_LENGTH
                Some(value.chars().take(length_value).collect())
            }).collect();

        // Add the fingerprint to the front.
        headers.insert(0, self.fingerprint().to_spaced_hex());

        headers
    }

    /// Wraps this Cert in an armor structure when serialized.
    ///
    /// Derives an object from this `Cert` that adds an armor structure
    /// to the serialized `Cert` when it is serialized.  Additionally,
    /// the `Cert`'s User IDs are added as comments, so that it is easier
    /// to identify the Cert when looking at the armored data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::serialize::SerializeInto;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None, Some("Mr. Pink ☮☮☮"))
    ///     .generate()?;
    /// let armored = String::from_utf8(cert.armored().to_vec()?)?;
    ///
    /// assert!(armored.starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----"));
    /// assert!(armored.contains("Mr. Pink ☮☮☮"));
    /// # Ok(()) }
    /// ```
    pub fn armored(&self)
        -> impl crate::serialize::Serialize + crate::serialize::SerializeInto + '_
    {
        Encoder::new(self)
    }
}

impl<'a> TSK<'a> {
    /// Wraps this TSK in an armor structure when serialized.
    ///
    /// Derives an object from this `TSK` that adds an armor structure
    /// to the serialized `TSK` when it is serialized.  Additionally,
    /// the `TSK`'s User IDs are added as comments, so that it is easier
    /// to identify the `TSK` when looking at the armored data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::serialize::SerializeInto;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let (cert, _) =
    ///     CertBuilder::general_purpose(None, Some("Mr. Pink ☮☮☮"))
    ///     .generate()?;
    /// let armored = String::from_utf8(cert.as_tsk().armored().to_vec()?)?;
    ///
    /// assert!(armored.starts_with("-----BEGIN PGP PRIVATE KEY BLOCK-----"));
    /// assert!(armored.contains("Mr. Pink ☮☮☮"));
    /// # Ok(()) }
    /// ```
    pub fn armored(self)
        -> impl crate::serialize::Serialize + crate::serialize::SerializeInto + 'a
    {
        Encoder::new_tsk(self)
    }
}

/// A `Cert` or `TSK` to be armored and serialized.
#[allow(clippy::upper_case_acronyms)]
enum Encoder<'a> {
    Cert(&'a Cert),
    TSK(TSK<'a>),
}

impl<'a> Encoder<'a> {
    /// Returns a new Encoder to enarmor and serialize a `Cert`.
    fn new(cert: &'a Cert) -> Self {
        Encoder::Cert(cert)
    }

    /// Returns a new Encoder to enarmor and serialize a `TSK`.
    fn new_tsk(tsk: TSK<'a>) -> Self {
        Encoder::TSK(tsk)
    }

    fn serialize_common(&self, o: &mut dyn io::Write, export: bool)
                        -> Result<()> {
        let (prelude, headers) = match self {
            Encoder::Cert(cert) =>
                (armor::Kind::PublicKey, cert.armor_headers()),
            Encoder::TSK(ref tsk) =>
                (armor::Kind::SecretKey, tsk.cert.armor_headers()),
        };

        // Convert the Vec<String> into Vec<(&str, &str)>
        // `iter_into` can not be used here because will take ownership and
        // what is needed is the reference.
        let headers: Vec<_> = headers.iter()
            .map(|value| ("Comment", value.as_str()))
            .collect();

        let mut w =
            armor::Writer::with_headers(o, prelude, headers)?;
        if export {
            match self {
                Encoder::Cert(cert) => cert.export(&mut w)?,
                Encoder::TSK(ref tsk) => tsk.export(&mut w)?,
            }
        } else {
            match self {
                Encoder::Cert(cert) => cert.serialize(&mut w)?,
                Encoder::TSK(ref tsk) => tsk.serialize(&mut w)?,
            }
        }
        w.finalize()?;
        Ok(())
    }
}

impl<'a> crate::serialize::Serialize for Encoder<'a> {}
impl<'a> seal::Sealed for Encoder<'a> {}
impl<'a> Marshal for Encoder<'a> {
    fn serialize(&self, o: &mut dyn io::Write) -> Result<()> {
        self.serialize_common(o, false)
    }

    fn export(&self, o: &mut dyn io::Write) -> Result<()> {
        self.serialize_common(o, true)
    }
}

impl<'a> crate::serialize::SerializeInto for Encoder<'a> {}

impl<'a> MarshalInto for Encoder<'a> {
    fn serialized_len(&self) -> usize {
        let h = match self {
            Encoder::Cert(cert) => cert.armor_headers(),
            Encoder::TSK(ref tsk) => tsk.cert.armor_headers(),
        };
        let headers_len =
            ("Comment: ".len() + 1 /* NL */) * h.len()
            + h.iter().map(|c| c.len()).sum::<usize>();
        let body_len = (match self {
            Self::Cert(cert) => cert.serialized_len(),
            Self::TSK(ref tsk) => tsk.serialized_len(),
        } + 2) / 3 * 4; // base64

        let word = match self {
            Self::Cert(_) => "PUBLIC",
            Self::TSK(_) => "PRIVATE",
        }.len();

        "-----BEGIN PGP ".len() + word + " KEY BLOCK-----\n\n".len()
            + headers_len
            + body_len
            + (body_len + armor::LINE_LENGTH - 1) / armor::LINE_LENGTH // NLs
            + "=FUaG\n-----END PGP ".len() + word + " KEY BLOCK-----\n".len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, self.serialized_len(), buf)
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_export_into(self, self.serialized_len(), buf)
    }
}


#[cfg(test)]
mod tests {
    use crate::armor::{Kind, Reader, ReaderMode};
    use crate::cert::prelude::*;
    use crate::parse::Parse;

    use super::*;

    #[test]
    fn is_printable_succeed() {
        let chars: Vec<char> = vec![
            'a', 'z', 'A', 'Z', '1', '9', '0',
            '|', '!', '#', '$', '%', '^', '&', '*', '-', '+', '/',
            // The following unicode characters were taken from:
            // https://doc.rust-lang.org/std/primitive.char.html
            'é', 'ß', 'ℝ', '💣', '❤', '東', '京', '𝕊', '💝', 'δ',
            'Δ', '中', '越', '٣', '7', '৬', '¾', '①', 'K',
            'و', '藏', '山', 'I', 'ï', 'İ', 'i'
        ];
        for c in &chars {
            assert!(is_printable(c));
        }
    }

    #[test]
    fn is_printable_fail() {
        let chars: Vec<char> = vec![
            '\n', 0x1b_u8.into(),
            // U+009C, STRING TERMINATOR
            ''
        ];
        for c in &chars {
            assert!(!is_printable(c));
        }
    }

    #[test]
    fn serialize_succeed() {
        let cert = Cert::from_bytes(crate::tests::key("neal.pgp")).unwrap();

        // Enarmor the Cert.
        let mut buffer = Vec::new();
        cert.armored()
            .serialize(&mut buffer)
            .unwrap();

        // Parse the armor.
        let mut cursor = io::Cursor::new(&buffer);
        let mut reader = Reader::from_reader(
            &mut cursor, ReaderMode::Tolerant(Some(Kind::PublicKey)));

        // Extract the headers.
        let mut headers: Vec<&str> = reader.headers()
            .unwrap()
            .into_iter()
            .map(|header| {
                assert_eq!(&header.0[..], "Comment");
                &header.1[..]})
            .collect();
        headers.sort();

        // Ensure the headers are correct
        let mut expected_headers = [
            "Neal H. Walfield <neal@walfield.org>",
            "Neal H. Walfield <neal@gnupg.org>",
            "Neal H. Walfield <neal@pep-project.org>",
            "Neal H. Walfield <neal@pep.foundation>",
            "Neal H. Walfield <neal@sequoia-pgp.org>",
            "8F17 7771 18A3 3DDA 9BA4  8E62 AACB 3243 6300 52D9"];
        expected_headers.sort();

        assert_eq!(&expected_headers[..], &headers[..]);
    }

    #[test]
    fn serialize_length_succeed() {
        let length_value = armor::LINE_LENGTH - "Comment: ".len();

        // Create userids one character longer than the size allowed in the
        // header and expect headers with the correct length.
        // 1 byte character
        // Can not use `to_string` here because not such method for
        //`std::vec::Vec<char>`
        let userid1: String = vec!['a'; length_value + 1].into_iter()
            .collect();
        let userid1_expected: String = vec!['a'; length_value].into_iter()
            .collect();
        // 2 bytes character.
        let userid2: String = vec!['ß'; length_value + 1].into_iter()
            .collect();
        let userid2_expected: String = vec!['ß'; length_value].into_iter()
            .collect();
        // 3 bytes character.
        let userid3: String = vec!['€'; length_value + 1].into_iter()
            .collect();
        let userid3_expected: String = vec!['€'; length_value].into_iter()
            .collect();
        // 4 bytes character.
        let userid4: String = vec!['𐍈'; length_value + 1].into_iter()
            .collect();
        let userid4_expected: String = vec!['𐍈'; length_value].into_iter()
            .collect();
        let mut userid5 = vec!['a'; length_value];
        userid5[length_value-1] = 'ß';
        let userid5: String = userid5.into_iter().collect();

        // Create a Cert with the userids.
        let (cert, _) = CertBuilder::general_purpose(None, Some(&userid1[..]))
            .add_userid(&userid2[..])
            .add_userid(&userid3[..])
            .add_userid(&userid4[..])
            .add_userid(&userid5[..])
            .generate()
            .unwrap();

        // Enarmor the Cert.
        let mut buffer = Vec::new();
        cert.armored()
            .serialize(&mut buffer)
            .unwrap();

        // Parse the armor.
        let mut cursor = io::Cursor::new(&buffer);
        let mut reader = Reader::from_reader(
            &mut cursor, ReaderMode::Tolerant(Some(Kind::PublicKey)));

        // Extract the headers.
        let mut headers: Vec<&str> = reader.headers()
            .unwrap()
            .into_iter()
            .map(|header| {
                assert_eq!(&header.0[..], "Comment");
                &header.1[..]})
            .skip(1) // Ignore the first header since it is the fingerprint
            .collect();
        // Cert canonicalization does not preserve the order of
        // userids.
        headers.sort();

        let mut headers_iter = headers.into_iter();
        assert_eq!(headers_iter.next().unwrap(), &userid1_expected);
        assert_eq!(headers_iter.next().unwrap(), &userid5);
        assert_eq!(headers_iter.next().unwrap(), &userid2_expected);
        assert_eq!(headers_iter.next().unwrap(), &userid3_expected);
        assert_eq!(headers_iter.next().unwrap(), &userid4_expected);
    }

    #[test]
    fn serialize_into() {
        let cert = Cert::from_bytes(crate::tests::key("neal.pgp")).unwrap();
        let mut v = Vec::new();
        cert.armored().serialize(&mut v).unwrap();
        let v_ = cert.armored().to_vec().unwrap();
        assert_eq!(v, v_);

        // Test truncation.
        let mut v = vec![0; cert.armored().serialized_len() - 1];
        let r = cert.armored().serialize_into(&mut v[..]);
        assert_match!(
            crate::Error::InvalidArgument(_) =
                r.unwrap_err().downcast().expect("not an openpgp::Error"));
    }
}
