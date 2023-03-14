use std::fmt;
use std::cmp;
use std::convert::TryInto;
use std::time;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::types::{DataFormat, Timestamp};
use crate::Error;
use crate::packet;
use crate::Packet;
use crate::Result;

/// Holds a literal packet.
///
/// A literal packet contains unstructured data.  Since the size can
/// be very large, it is advised to process messages containing such
/// packets using a `PacketParser` or a `PacketPileParser` and process
/// the data in a streaming manner rather than the using the
/// `PacketPile::from_file` and related interfaces.
///
/// See [Section 5.9 of RFC 4880] for details.
///
///   [Section 5.9 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.9
// IMPORTANT: If you add fields to this struct, you need to explicitly
// IMPORTANT: implement PartialEq, Eq, and Hash.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Literal {
    /// CTB packet header fields.
    pub(crate) common: packet::Common,
    /// A one-octet field that describes how the data is formatted.
    format: DataFormat,
    /// filename is a string, but strings in Rust are valid UTF-8.
    /// There is no guarantee, however, that the filename is valid
    /// UTF-8.  Thus, we leave filename as a byte array.  It can be
    /// converted to a string using String::from_utf8() or
    /// String::from_utf8_lossy().
    filename: Option<Vec<u8>>,
    /// A four-octet number that indicates a date associated with the
    /// literal data.
    date: Option<Timestamp>,
    /// The literal data packet is a container packet, but cannot
    /// store packets.
    ///
    /// This is written when serialized, and set by the packet parser
    /// if `buffer_unread_content` is used.
    container: packet::Container,
}
assert_send_and_sync!(Literal);

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let filename = self
            .filename
            .as_ref()
            .map(|filename| String::from_utf8_lossy(filename));

        let threshold = 36;
        let body = self.body();
        let prefix = &body[..cmp::min(threshold, body.len())];
        let mut prefix_fmt = String::from_utf8_lossy(prefix).into_owned();
        if body.len() > threshold {
            prefix_fmt.push_str("...");
        }
        prefix_fmt.push_str(&format!(" ({} bytes)", body.len())[..]);

        f.debug_struct("Literal")
            .field("format", &self.format)
            .field("filename", &filename)
            .field("date", &self.date)
            .field("body", &prefix_fmt)
            .field("body_digest", &self.container.body_digest())
            .finish()
    }
}

impl Default for Literal {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl Literal {
    /// Returns a new `Literal` packet.
    pub fn new(format: DataFormat) -> Literal {
        Literal {
            common: Default::default(),
            format,
            filename: None,
            date: None,
            container: packet::Container::default_unprocessed(),
        }
    }

    /// Gets the Literal packet's content disposition.
    pub fn format(&self) -> DataFormat {
        self.format
    }

    /// Sets the Literal packet's content disposition.
    pub fn set_format(&mut self, format: DataFormat) -> DataFormat {
        ::std::mem::replace(&mut self.format, format)
    }

    /// Gets the literal packet's filename.
    ///
    /// Note: when a literal data packet is protected by a signature,
    /// only the literal data packet's body is protected, not the
    /// meta-data.  As such, this field should normally be ignored.
    pub fn filename(&self) -> Option<&[u8]> {
        self.filename.as_deref()
    }

    /// Sets the literal packet's filename field.
    ///
    /// The standard does not specify the encoding.  Filenames must
    /// not be longer than 255 bytes.
    ///
    /// Note: when a literal data packet is protected by a signature,
    /// only the literal data packet's body is protected, not the
    /// meta-data.  As such, this field should not be used.
    pub fn set_filename<F>(&mut self, filename: F)
                           -> Result<Option<Vec<u8>>>
        where F: AsRef<[u8]>
    {
        let filename = filename.as_ref();
        Ok(::std::mem::replace(&mut self.filename, match filename.len() {
            0 => None,
            1..=255 => Some(filename.to_vec()),
            n => return
                Err(Error::InvalidArgument(
                    format!("filename too long: {} bytes", n)).into()),
        }))
    }

    /// Gets the literal packet's date field.
    ///
    /// Note: when a literal data packet is protected by a signature,
    /// only the literal data packet's body is protected, not the
    /// meta-data.  As such, this field should normally be ignored.
    pub fn date(&self) -> Option<time::SystemTime> {
        self.date.map(|d| d.into())
    }

    /// Sets the literal packet's date field.
    ///
    /// Note: when a literal data packet is protected by a signature,
    /// only the literal data packet's body is protected, not the
    /// meta-data.  As such, this field should not be used.
    pub fn set_date<T>(&mut self, timestamp: T)
                       -> Result<Option<time::SystemTime>>
        where T: Into<Option<time::SystemTime>>
    {
        let date = if let Some(d) = timestamp.into() {
            let t = d.try_into()?;
            if u32::from(t) == 0 {
                None // RFC4880, section 5.9: 0 =^= "no specific time".
            } else {
                Some(t)
            }
        } else {
            None
        };
        Ok(std::mem::replace(&mut self.date, date).map(|d| d.into()))
    }
}

impl_body_forwards!(Literal);

impl From<Literal> for Packet {
    fn from(s: Literal) -> Self {
        Packet::Literal(s)
    }
}

#[cfg(test)]
impl Arbitrary for Literal {
    fn arbitrary(g: &mut Gen) -> Self {
        let mut l = Literal::new(DataFormat::arbitrary(g));
        l.set_body(Vec::<u8>::arbitrary(g));
        while let Err(_) = l.set_filename(&Vec::<u8>::arbitrary(g)) {
            // Too long, try again.
        }
        l.set_date(Some(Timestamp::arbitrary(g).into())).unwrap();
        l
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Parse;
    use crate::serialize::MarshalInto;

    quickcheck! {
        fn roundtrip(p: Literal) -> bool {
            let q = Literal::from_bytes(&p.to_vec().unwrap()).unwrap();
            assert_eq!(p, q);
            true
        }
    }

    /// Checks that partially read packets are still considered equal.
    #[test]
    fn partial_read_eq() -> Result<()> {
        use buffered_reader::BufferedReader;
        use crate::parse::PacketParserBuilder;

        let mut l0 = Literal::new(Default::default());
        l0.set_body(vec![0, 0]);
        let l0 = Packet::from(l0);
        let l0bin = l0.to_vec()?;
        // Sanity check.
        assert_eq!(l0, Packet::from_bytes(&l0bin)?);

        for &buffer_unread_content in &[false, true] {
            for read_n in 0..3 {
                eprintln!("buffer_unread_content: {:?}, read_n: {}",
                          buffer_unread_content, read_n);

                let mut b = PacketParserBuilder::from_bytes(&l0bin)?;
                if buffer_unread_content {
                    b = b.buffer_unread_content();
                }
                let mut pp = b.build()?.unwrap();
                let d = pp.steal(read_n)?;
                d.into_iter().for_each(|b| assert_eq!(b, 0));
                let l = pp.finish()?;
                assert_eq!(&l0, l);
                let l = pp.next()?.0;
                assert_eq!(l0, l);
            }
        }
        Ok(())
    }
}
