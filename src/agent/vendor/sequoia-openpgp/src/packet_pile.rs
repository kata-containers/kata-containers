use std::convert::TryFrom;
use std::fmt;
use std::vec;
use std::io;
use std::path::Path;
use std::iter::FromIterator;
use std::iter::IntoIterator;

use buffered_reader::BufferedReader;

use crate::Result;
use crate::Error;
use crate::Packet;
use crate::cert::Cert;
use crate::packet::{self, Container};
use crate::parse::PacketParserResult;
use crate::parse::PacketParserBuilder;
use crate::parse::Parse;
use crate::parse::Cookie;

/// An unstructured [packet] sequence.
///
/// To parse an OpenPGP packet stream into a `PacketPile`, you can use
/// [`PacketParser`], [`PacketPileParser`], or
/// [`PacketPile::from_file`] (or related routines).
///
///   [packet]: https://tools.ietf.org/html/rfc4880#section-4
///   [`PacketParser`]: crate::parse::PacketParser
///   [`PacketPileParser`]: crate::parse::PacketPileParser
///
/// You can also convert a [`Cert`] into a `PacketPile` using
/// `PacketPile::from`.  Unlike serializing a `Cert`, this does not
/// drop any secret key material.
///
/// Normally, you'll want to convert the `PacketPile` to a `Cert` or a
/// `Message`.
///
/// # Examples
///
/// This example shows how to modify packets in PacketPile using [`pathspec`]s.
///
///   [`pathspec`]: PacketPile::path_ref()
///
/// ```rust
/// # use sequoia_openpgp as openpgp;
/// use std::convert::TryFrom;
/// use openpgp::{Packet, PacketPile};
/// use openpgp::packet::signature::Signature4;
/// use openpgp::packet::Signature;
/// use openpgp::cert::prelude::*;
/// use openpgp::parse::Parse;
/// use openpgp::serialize::Serialize;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::crypto::mpi;
/// use openpgp::types::RevocationStatus::{Revoked, CouldBe};
///
/// # fn main() -> openpgp::Result<()> {
/// let (cert, revocation) = CertBuilder::new().generate()?;
///
/// let mut buffer = Vec::new();
/// cert.serialize(&mut buffer)?;
/// let packet: Packet = revocation.into();
/// packet.serialize(&mut buffer)?;
///
/// let policy = &StandardPolicy::new();
///
/// // Certificate is considered revoked because it is accompanied with its
/// // revocation signature
/// let pp: PacketPile = PacketPile::from_bytes(&buffer)?;
/// let cert = Cert::try_from(pp)?;
/// if let Revoked(_) = cert.revocation_status(policy, None) {
///     // cert is considered revoked
/// }
/// # else {
/// #     unreachable!();
/// # }
///
/// // Breaking the revocation signature changes certificate's status
/// let mut pp: PacketPile = PacketPile::from_bytes(&buffer)?;
/// if let Some(Packet::Signature(ref mut sig)) = pp.path_ref_mut(&[2]) {
///     *sig = Signature4::new(
///         sig.typ(),
///         sig.pk_algo(),
///         sig.hash_algo(),
///         sig.hashed_area().clone(),
///         sig.unhashed_area().clone(),
///         *sig.digest_prefix(),
///         // MPI is replaced with a dummy one
///         mpi::Signature::RSA {
///             s: mpi::MPI::from(vec![1, 2, 3])
///         }).into();
/// }
///
/// let cert = Cert::try_from(pp)?;
/// if let NotAsFarAsWeKnow = cert.revocation_status(policy, None) {
///     // revocation signature is broken and the cert is not revoked
///     assert_eq!(cert.bad_signatures().count(), 1);
/// }
/// # else {
/// #   unreachable!();
/// # }
/// #     Ok(())
/// # }
/// ```
#[derive(PartialEq, Clone, Default)]
pub struct PacketPile {
    /// At the top level, we have a sequence of packets, which may be
    /// containers.
    top_level: Container,
}

assert_send_and_sync!(PacketPile);

impl fmt::Debug for PacketPile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PacketPile")
            .field("packets", &self.top_level.children_ref())
            .finish()
    }
}

impl<'a> Parse<'a, PacketPile> for PacketPile {
    /// Deserializes the OpenPGP message stored in a `std::io::Read`
    /// object.
    ///
    /// Although this method is easier to use to parse a sequence of
    /// OpenPGP packets than a [`PacketParser`] or a
    /// [`PacketPileParser`], this interface buffers the whole message
    /// in memory.  Thus, the caller must be certain that the
    /// *deserialized* message is not too large.
    ///
    /// Note: this interface *does* buffer the contents of packets.
    ///
    ///   [`PacketParser`]: crate::parse::PacketParser
    ///   [`PacketPileParser`]: crate::parse::PacketPileParser
    fn from_reader<R: 'a + io::Read + Send + Sync>(reader: R) -> Result<PacketPile> {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        PacketPile::from_buffered_reader(Box::new(bio))
    }

    /// Deserializes the OpenPGP message stored in the file named by
    /// `path`.
    ///
    /// See `from_reader` for more details and caveats.
    fn from_file<P: AsRef<Path>>(path: P) -> Result<PacketPile> {
        PacketPile::from_buffered_reader(
            Box::new(buffered_reader::File::with_cookie(path, Cookie::default())?))
    }

    /// Deserializes the OpenPGP message stored in the provided buffer.
    ///
    /// See `from_reader` for more details and caveats.
    fn from_bytes<D: AsRef<[u8]> + ?Sized>(data: &'a D) -> Result<PacketPile> {
        let bio = buffered_reader::Memory::with_cookie(
            data.as_ref(), Cookie::default());
        PacketPile::from_buffered_reader(Box::new(bio))
    }
}

impl std::str::FromStr for PacketPile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_bytes(s.as_bytes())
    }
}

impl From<Vec<Packet>> for PacketPile {
    fn from(p: Vec<Packet>) -> Self {
        PacketPile { top_level: Container::from(p) }
    }
}

impl From<Packet> for PacketPile {
    fn from(p: Packet) -> Self {
        Self::from(vec![p])
    }
}

impl FromIterator<Packet> for PacketPile {
    fn from_iter<I: IntoIterator<Item=Packet>>(iter: I) -> Self {
        Self::from(Vec::from_iter(iter))
    }
}

impl From<PacketPile> for Vec<Packet> {
    fn from(pp: PacketPile) -> Self {
        pp.into_children().collect()
    }
}

impl PacketPile {
    /// Accessor for PacketPileParser.
    pub(crate) fn top_level_mut(&mut self) -> &mut Container {
        &mut self.top_level
    }

    /// Returns an error if operating on a non-container packet.
    fn error() -> crate::Error {
        crate::Error::InvalidOperation("Not a container packet".into())
    }

    /// Pretty prints the message to stderr.
    ///
    /// This function is primarily intended for debugging purposes.
    pub fn pretty_print(&self) {
        self.top_level.pretty_print(0);
    }

    /// Returns a reference to the packet at the location described by
    /// `pathspec`.
    ///
    /// `pathspec` is a slice of the form `[0, 1, 2]`.  Each element
    /// is the index of packet in a container.  Thus, the previous
    /// path specification means: return the third child of the second
    /// child of the first top-level packet.  In other words, the
    /// starred packet in the following tree:
    ///
    /// ```text
    ///         PacketPile
    ///        /     |     \
    ///       0      1      2  ...
    ///     /   \
    ///    /     \
    ///  0         1  ...
    ///        /   |   \  ...
    ///       0    1    2
    ///                 *
    /// ```
    ///
    /// And, `[10]` means return the 11th top-level packet.
    ///
    /// Note: there is no packet at the root.  Thus, the path `[]`
    /// returns None.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::{Result, types::{CompressionAlgorithm, DataFormat},
    /// #     Packet, PacketPile, packet::Literal, packet::CompressedData};
    /// # fn main() -> Result<()> {
    /// # let mut lit = Literal::new(DataFormat::Text);
    /// # lit.set_body(b"test".to_vec());
    /// # let packets = vec![lit.into()];
    /// let pile = PacketPile::from(packets);
    ///
    /// if let Some(packet) = pile.path_ref(&[0]) {
    ///     // There is a packet at this path.
    /// }
    /// # else {
    /// #     unreachable!();
    /// # }
    ///
    /// if let None = pile.path_ref(&[0, 1, 2]) {
    ///     // But none here.
    /// }
    /// # else {
    /// #     unreachable!();
    /// # }
    /// # Ok(())
    /// # }
    /// ```
    pub fn path_ref(&self, pathspec: &[usize]) -> Option<&Packet> {
        let mut packet : Option<&Packet> = None;

        let mut cont = Some(&self.top_level);
        for i in pathspec {
            if let Some(c) = cont.take() {
                if let Some(children) = c.children_ref() {
                    if *i < children.len() {
                        let p = &children[*i];
                        packet = Some(p);
                        cont = p.container_ref();
                        continue;
                    }
                }
            }

            return None;
        }
        packet
    }

    /// Returns a mutable reference to the packet at the location
    /// described by `pathspec`.
    ///
    /// See the description of the `path_spec` for more details.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::{Result, types::{CompressionAlgorithm, DataFormat},
    /// #     Packet, PacketPile, packet::Literal, packet::CompressedData};
    /// # fn main() -> Result<()> {
    /// # let mut lit = Literal::new(DataFormat::Text);
    /// # lit.set_body(b"test".to_vec());
    /// # let packets = vec![lit.into()];
    /// let mut pile = PacketPile::from(packets);
    ///
    /// if let Some(ref packet) = pile.path_ref_mut(&[0]) {
    ///     // There is a packet at this path.
    /// }
    /// # else {
    /// #     unreachable!();
    /// # }
    ///
    /// if let None = pile.path_ref_mut(&[0, 1, 2]) {
    ///     // But none here.
    /// }
    /// # else {
    /// #     unreachable!();
    /// # }
    /// # Ok(())
    /// # }
    /// ```
    pub fn path_ref_mut(&mut self, pathspec: &[usize]) -> Option<&mut Packet> {
        let mut container = &mut self.top_level;

        for (level, &i) in pathspec.iter().enumerate() {
            let tmp = container;

            let p = tmp.children_mut().and_then(|c| c.get_mut(i))?;

            if level == pathspec.len() - 1 {
                return Some(p)
            }

            container = p.container_mut().unwrap();
        }

        None
    }

    /// Replaces the specified packets at the location described by
    /// `pathspec` with `packets`.
    ///
    /// If a packet is a container, the sub-tree rooted at the
    /// container is removed.
    ///
    /// Note: the number of packets to remove need not match the
    /// number of packets to insert.
    ///
    /// The removed packets are returned.
    ///
    /// If the path was invalid, then `Error::IndexOutOfRange` is
    /// returned instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::{Result, types::{CompressionAlgorithm, DataFormat},
    /// #     Packet, PacketPile, packet::Literal, packet::CompressedData};
    /// # fn main() -> Result<()> {
    /// // A compressed data packet that contains a literal data packet.
    /// let mut literal = Literal::new(DataFormat::Text);
    /// literal.set_body(b"old".to_vec());
    /// let mut compressed =
    ///     CompressedData::new(CompressionAlgorithm::Uncompressed);
    /// compressed.children_mut().unwrap().push(literal.into());
    /// let mut pile = PacketPile::from(Packet::from(compressed));
    ///
    /// // Replace the literal data packet.
    /// let mut literal = Literal::new(DataFormat::Text);
    /// literal.set_body(b"new".to_vec());
    /// pile.replace(
    ///     &[0, 0], 1,
    ///     [literal.into()].to_vec())?;
    /// # if let Some(Packet::Literal(lit)) = pile.path_ref(&[0, 0]) {
    /// #     assert_eq!(lit.body(), &b"new"[..], "{:#?}", lit);
    /// # } else {
    /// #     panic!("Unexpected packet!");
    /// # }
    /// #     Ok(())
    /// # }
    /// ```
    pub fn replace(&mut self, pathspec: &[usize], count: usize,
                   mut packets: Vec<Packet>)
        -> Result<Vec<Packet>>
    {
        let mut container = &mut self.top_level;

        for (level, &i) in pathspec.iter().enumerate() {
            let tmp = container;

            if level == pathspec.len() - 1 {
                if tmp.children_ref().map(|c| i + count > c.len())
                    .unwrap_or(true)
                {
                    return Err(Error::IndexOutOfRange.into());
                }

                // Out with the old...
                let old = tmp.children_mut()
                    .expect("checked above")
                    .drain(i..i + count)
                    .collect::<Vec<Packet>>();
                assert_eq!(old.len(), count);

                // In with the new...

                let mut tail = tmp.children_mut()
                    .expect("checked above")
                    .drain(i..)
                    .collect::<Vec<Packet>>();

                tmp.children_mut().expect("checked above").append(&mut packets);
                tmp.children_mut().expect("checked above").append(&mut tail);

                return Ok(old)
            }

            if tmp.children_ref().map(|c| i >= c.len()).unwrap_or(true) {
                return Err(Error::IndexOutOfRange.into());
            }

            match tmp.children_ref().expect("checked above")[i] {
                // The structured container types.
                Packet::CompressedData(_)
                    | Packet::SEIP(_)
                    | Packet::AED(_)
                    => (), // Ok.
                _ => return Err(Error::IndexOutOfRange.into()),
            }
            container =
                tmp.children_mut().expect("checked above")[i].container_mut()
                .expect("The above packets are structured containers");
        }

        Err(Error::IndexOutOfRange.into())
    }

    /// Returns an iterator over all of the packet's descendants, in
    /// depth-first order.
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::{Result, types::{CompressionAlgorithm, DataFormat},
    /// #     Packet, PacketPile, packet::Literal, packet::Tag};
    /// # use std::iter::Iterator;
    /// # fn main() -> Result<()> {
    /// let mut lit = Literal::new(DataFormat::Text);
    /// lit.set_body(b"test".to_vec());
    ///
    /// let pile = PacketPile::from(vec![lit.into()]);
    ///
    /// for packet in pile.descendants() {
    ///     assert_eq!(packet.tag(), Tag::Literal);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn descendants(&self) -> packet::Iter {
        self.top_level.descendants().expect("toplevel is a container")
    }

    /// Returns an iterator over the top-level packets.
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::{Result, types::{CompressionAlgorithm, DataFormat},
    /// #     Packet, PacketPile, packet::Literal, packet::CompressedData};
    /// # fn main() -> Result<()> {
    /// let mut lit = Literal::new(DataFormat::Text);
    /// lit.set_body(b"test".to_vec());
    ///
    /// let pile = PacketPile::from(vec![lit.into()]);
    ///
    /// assert_eq!(pile.children().len(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn children(&self)
        -> impl Iterator<Item=&Packet> + ExactSizeIterator + Send + Sync
    {
        self.top_level.children().expect("toplevel is a container")
    }

    /// Returns an `IntoIter` over the top-level packets.
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::{Result, types::{CompressionAlgorithm, DataFormat},
    /// #     Packet, PacketPile, packet::Literal, packet::Tag};
    /// # fn main() -> Result<()> {
    /// let mut lit = Literal::new(DataFormat::Text);
    /// lit.set_body(b"test".to_vec());
    ///
    /// let pile = PacketPile::from(vec![lit.into()]);
    ///
    /// for packet in pile.into_children() {
    ///     assert_eq!(packet.tag(), Tag::Literal);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn into_children(self)
        -> impl Iterator<Item=Packet> + ExactSizeIterator + Send + Sync
    {
        self.top_level.into_children().expect("toplevel is a container")
    }


    pub(crate) fn from_buffered_reader<'a>(bio: Box<dyn BufferedReader<Cookie> + 'a>)
            -> Result<PacketPile> {
        PacketParserBuilder::from_buffered_reader(bio)?
            .buffer_unread_content()
            .into_packet_pile()
    }
}

impl From<Cert> for PacketPile {
    /// Converts the `Cert` into a `PacketPile`.
    ///
    /// If any packets include secret key material, that secret key
    /// material is not dropped, as it is when serializing a `Cert`.
    fn from(cert: Cert) -> PacketPile {
        PacketPile::from(cert.into_packets().collect::<Vec<Packet>>())
    }
}

impl<'a> TryFrom<PacketParserResult<'a>> for PacketPile {
    type Error = anyhow::Error;

    /// Reads all of the packets from a `PacketParser`, and turns them
    /// into a message.
    ///
    /// Note: this assumes that `ppr` points to a top-level packet.
    fn try_from(ppr: PacketParserResult<'a>)
        -> Result<PacketPile>
    {
        // Things are not going to work out if we don't start with a
        // top-level packet.  We should only pop until
        // ppo.recursion_depth and leave the rest of the message, but
        // it is hard to imagine that that is what the caller wants.
        // Instead of hiding that error, fail fast.
        if let PacketParserResult::Some(ref pp) = ppr {
            if pp.recursion_depth() != 0 {
                return Err(Error::InvalidOperation(
                    format!("Expected top-level packet, \
                             but the parser is at level {}",
                            pp.recursion_depth())).into());
            }
        }

        // Create a top-level container.
        let mut top_level = Container::default();

        let mut last_position = 0;

        if ppr.is_eof() {
            // Empty message.
            return Ok(PacketPile::from(Vec::new()));
        }
        let mut pp = ppr.unwrap();

        'outer: loop {
            let recursion_depth = pp.recursion_depth();
            let (mut packet, mut ppr) = pp.recurse()?;
            let mut position = recursion_depth as isize;

            let mut relative_position : isize = position - last_position;
            assert!(relative_position <= 1);

            // Find the right container for `packet`.
            let mut container = &mut top_level;
            // If we recurse, don't create the new container here.
            for _ in 0..(position - if relative_position > 0 { 1 } else { 0 }) {
                // Do a little dance to prevent container from
                // being reborrowed and preventing us from
                // assigning to it.
                let tmp = container;
                let packets_len =
                    tmp.children_ref().ok_or_else(Self::error)?.len();
                let p = &mut tmp.children_mut()
                    .ok_or_else(Self::error)?
                    [packets_len - 1];

                container = p.container_mut().unwrap();
            }

            if relative_position < 0 {
                relative_position = 0;
            }

            // If next packet will be inserted in the same container
            // or the current container's child, we don't need to walk
            // the tree from the root.
            loop {
                if relative_position == 1 {
                    // Create a new container.
                    let tmp = container;
                    let i =
                        tmp.children_ref().ok_or_else(Self::error)?.len() - 1;
                    container = tmp.children_mut()
                        .ok_or_else(Self::error)?
                        [i].container_mut().unwrap();
                }

                container.children_mut().unwrap().push(packet);

                if ppr.is_eof() {
                    break 'outer;
                }

                pp = ppr.unwrap();

                last_position = position;
                position = pp.recursion_depth() as isize;
                relative_position = position - last_position;
                if position < last_position {
                    // There was a pop, we need to restart from the
                    // root.
                    break;
                }

                let recursion_depth = pp.recursion_depth();
                let (packet_, ppr_) = pp.recurse()?;
                packet = packet_;
                ppr = ppr_;
                assert_eq!(position, recursion_depth as isize);
            }
        }

        Ok(PacketPile { top_level })
    }
}

impl<'a> PacketParserBuilder<'a> {
    /// Finishes configuring the `PacketParser` and returns a fully
    /// parsed message.
    ///
    /// Note: calling this function does not change the default
    /// settings.  Thus, by default, the content of packets will *not*
    /// be buffered.
    ///
    /// Note: to avoid denial of service attacks, the `PacketParser`
    /// interface should be preferred unless the size of the message
    /// is known to fit in memory.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::PacketPile;
    /// # use openpgp::parse::{Parse, PacketParser, PacketParserBuilder};
    /// # f(include_bytes!("../tests/data/keys/public-key.gpg"));
    /// #
    /// # fn f(message_data: &[u8]) -> Result<PacketPile> {
    /// let message = PacketParserBuilder::from_bytes(message_data)?
    ///     .buffer_unread_content()
    ///     .into_packet_pile()?;
    /// # return Ok(message);
    /// # }
    /// ```
    pub fn into_packet_pile(self) -> Result<PacketPile> {
        PacketPile::try_from(self.build()?)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::types::CompressionAlgorithm;
    use crate::types::DataFormat::Text;
    use crate::packet::Literal;
    use crate::packet::CompressedData;
    use crate::packet::seip::SEIP1;
    use crate::packet::Tag;
    use crate::parse::Parse;

    #[test]
    fn deserialize_test_1 () {
        // XXX: This test should be more thorough.  Right now, we mostly
        // just rely on the fact that an assertion is not thrown.

        // A flat message.
        let pile = PacketPile::from_bytes(crate::tests::key("public-key.gpg"))
            .unwrap();
        eprintln!("PacketPile has {} top-level packets.",
                  pile.children().len());
        eprintln!("PacketPile: {:?}", pile);

        let mut count = 0;
        for (i, p) in pile.descendants().enumerate() {
            eprintln!("{}: {:?}", i, p);
            count += 1;
        }

        assert_eq!(count, 61);
    }

    #[cfg(feature = "compression-deflate")]
    #[test]
    fn deserialize_test_2 () {
        // A message containing a compressed packet that contains a
        // literal packet.
        let pile = PacketPile::from_bytes(
            crate::tests::message("compressed-data-algo-1.gpg")).unwrap();
        eprintln!("PacketPile has {} top-level packets.",
                  pile.children().len());
        eprintln!("PacketPile: {:?}", pile);

        let mut count = 0;
        for (i, p) in pile.descendants().enumerate() {
            eprintln!("{}: {:?}", i, p);
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[cfg(feature = "compression-deflate")]
    #[test]
    fn deserialize_test_3 () {
        let pile =
            PacketPile::from_bytes(crate::tests::message("signed.gpg")).unwrap();
        eprintln!("PacketPile has {} top-level packets.",
                  pile.children().len());
        eprintln!("PacketPile: {:?}", pile);

        let mut count = 0;
        for (i, p) in pile.descendants().enumerate() {
            count += 1;
            eprintln!("{}: {:?}", i, p);
        }
        // We expect 6 packets.
        assert_eq!(count, 6);
    }

    // dkg's key contains packets from different OpenPGP
    // implementations.  And, it even includes some v3 signatures.
    //
    // lutz's key is a v3 key.
    #[test]
    fn torture() {
        use std::convert::TryInto;
        use crate::parse::PacketPileParser;

        let data = crate::tests::key("dkg.gpg");
        let mut ppp: PacketPileParser =
            PacketParserBuilder::from_bytes(data).unwrap()
            //.trace()
            .buffer_unread_content()
            .try_into().unwrap();

        while ppp.is_some() {
            ppp.recurse().unwrap();
        }
        let pile = ppp.finish();
        //pile.pretty_print();
        assert_eq!(pile.children().len(), 1450);

        let data = crate::tests::key("lutz.gpg");
        let mut ppp: PacketPileParser =
            PacketParserBuilder::from_bytes(data).unwrap()
            //.trace()
            .buffer_unread_content()
            .try_into().unwrap();

        while let Ok(pp) = ppp.as_ref() {
            eprintln!("{:?}", pp);
            ppp.recurse().unwrap();
        }
        let pile = ppp.finish();
        pile.pretty_print();
        assert_eq!(pile.children().len(), 77);
    }

    #[cfg(feature = "compression-deflate")]
    #[test]
    fn compression_quine_test_1 () {
        // Use the PacketPile::from_file interface to parse an OpenPGP
        // quine.
        let max_recursion_depth = 128;
        let pile = PacketParserBuilder::from_bytes(
            crate::tests::message("compression-quine.gpg")).unwrap()
            .max_recursion_depth(max_recursion_depth)
            .into_packet_pile().unwrap();

        let mut count = 0;
        for (i, p) in pile.descendants().enumerate() {
            count += 1;
            if false {
                eprintln!("{}: p: {:?}", i, p);
            }
        }

        assert_eq!(count, 1 + max_recursion_depth);
    }

    #[cfg(feature = "compression-deflate")]
    #[test]
    fn compression_quine_test_2 () {
        // Use the iterator interface to parse an OpenPGP quine.
        let max_recursion_depth = 255;
        let mut ppr : PacketParserResult
            = PacketParserBuilder::from_bytes(
                crate::tests::message("compression-quine.gpg")).unwrap()
                .max_recursion_depth(max_recursion_depth)
                .build().unwrap();

        let mut count = 0;
        loop {
            if let PacketParserResult::Some(pp2) = ppr {
                count += 1;

                let packet_depth = pp2.recursion_depth();
                let pp2 = pp2.recurse().unwrap().1;
                assert_eq!(packet_depth, count - 1);
                if pp2.is_some() {
                    assert_eq!(pp2.as_ref().unwrap().recursion_depth(), count);
                }
                ppr = pp2;
            } else {
                break;
            }
        }
        assert_eq!(count, 1 + max_recursion_depth as isize);
    }

    #[cfg(feature = "compression-deflate")]
    #[test]
    fn consume_content_1 () {
        use std::io::Read;
        use crate::parse::PacketParser;
        // A message containing a compressed packet that contains a
        // literal packet.  When we read some of the compressed
        // packet, we expect recurse() to not recurse.

        let ppr = PacketParserBuilder::from_bytes(
                crate::tests::message("compressed-data-algo-1.gpg")).unwrap()
            .buffer_unread_content()
            .build().unwrap();

        let mut pp = ppr.unwrap();
        if let Packet::CompressedData(_) = pp.packet {
        } else {
            panic!("Expected a compressed packet!");
        }

        // Read some of the body of the compressed packet.
        let mut data = [0u8; 1];
        let amount = pp.read(&mut data).unwrap();
        assert_eq!(amount, 1);

        // recurse should now not recurse.  Since there is nothing
        // following the compressed packet, ppr should be EOF.
        let (packet, ppr) = pp.next().unwrap();
        assert!(ppr.is_eof());

        // Get the rest of the content and put the initial byte that
        // we stole back.
        let mut content = packet.processed_body().unwrap().to_vec();
        content.insert(0, data[0]);

        let ppr = PacketParser::from_bytes(&content).unwrap();
        let pp = ppr.unwrap();
        if let Packet::Literal(_) = pp.packet {
        } else {
            panic!("Expected a literal packet!");
        }

        // And we're done...
        let ppr = pp.next().unwrap().1;
        assert!(ppr.is_eof());
    }

    #[test]
    fn path_ref() {
        // 0: SEIP
        //  0: CompressedData
        //   0: Literal("one")
        //   1: Literal("two")
        //   2: Literal("three")
        //   3: Literal("four")
        let mut packets : Vec<Packet> = Vec::new();

        let text = [ &b"one"[..], &b"two"[..],
                      &b"three"[..], &b"four"[..] ].to_vec();

        let mut cd = CompressedData::new(CompressionAlgorithm::Uncompressed);
        for t in text.iter() {
            let mut lit = Literal::new(Text);
            lit.set_body(t.to_vec());
            cd = cd.push(lit.into())
        }

        let mut seip = SEIP1::new();
        seip.children_mut().unwrap().push(cd.into());
        packets.push(seip.into());

        eprintln!("{:#?}", packets);

        let mut pile = PacketPile::from(packets);

        assert_eq!(pile.path_ref(&[ 0 ]).unwrap().tag(), Tag::SEIP);
        assert_eq!(pile.path_ref_mut(&[ 0 ]).unwrap().tag(), Tag::SEIP);
        assert_eq!(pile.path_ref(&[ 0, 0 ]).unwrap().tag(),
                   Tag::CompressedData);
        assert_eq!(pile.path_ref_mut(&[ 0, 0 ]).unwrap().tag(),
                   Tag::CompressedData);

        for (i, t) in text.into_iter().enumerate() {
            assert_eq!(pile.path_ref(&[ 0, 0, i ]).unwrap().tag(),
                       Tag::Literal);
            assert_eq!(pile.path_ref_mut(&[ 0, 0, i ]).unwrap().tag(),
                       Tag::Literal);

            let packet = pile.path_ref(&[ 0, 0, i ]).unwrap();
            if let Packet::Literal(l) = packet {
                assert_eq!(l.body(), t);
            } else {
                panic!("Expected literal, got: {:?}", packet);
            }
            let packet = pile.path_ref_mut(&[ 0, 0, i ]).unwrap();
            if let Packet::Literal(l) = packet {
                assert_eq!(l.body(), t);
            } else {
                panic!("Expected literal, got: {:?}", packet);
            }
        }

        // Try a few out of bounds accesses.
        assert!(pile.path_ref(&[ 0, 0, 4 ]).is_none());
        assert!(pile.path_ref_mut(&[ 0, 0, 4 ]).is_none());

        assert!(pile.path_ref(&[ 0, 0, 5 ]).is_none());
        assert!(pile.path_ref_mut(&[ 0, 0, 5 ]).is_none());

        assert!(pile.path_ref(&[ 0, 1 ]).is_none());
        assert!(pile.path_ref_mut(&[ 0, 1 ]).is_none());

        assert!(pile.path_ref(&[ 0, 2 ]).is_none());
        assert!(pile.path_ref_mut(&[ 0, 2 ]).is_none());

        assert!(pile.path_ref(&[ 1 ]).is_none());
        assert!(pile.path_ref_mut(&[ 1 ]).is_none());

        assert!(pile.path_ref(&[ 2 ]).is_none());
        assert!(pile.path_ref_mut(&[ 2 ]).is_none());

        assert!(pile.path_ref(&[ 0, 1, 0 ]).is_none());
        assert!(pile.path_ref_mut(&[ 0, 1, 0 ]).is_none());

        assert!(pile.path_ref(&[ 0, 2, 0 ]).is_none());
        assert!(pile.path_ref_mut(&[ 0, 2, 0 ]).is_none());
    }

    #[test]
    fn replace() {
        // 0: Literal("one")
        // =>
        // 0: Literal("two")
        let mut one = Literal::new(Text);
        one.set_body(b"one".to_vec());
        let mut two = Literal::new(Text);
        two.set_body(b"two".to_vec());
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(one.into());

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::Literal ]);

        let mut pile = PacketPile::from(packets.clone());
        pile.replace(
            &[ 0 ], 1,
            [ two.into()
            ].to_vec()).unwrap();

        let children = pile.into_children().collect::<Vec<Packet>>();
        assert_eq!(children.len(), 1, "{:#?}", children);
        if let Packet::Literal(ref literal) = children[0] {
            assert_eq!(literal.body(), &b"two"[..], "{:#?}", literal);
        } else {
            panic!("WTF");
        }

        // We start with four packets, and replace some of them with
        // up to 3 packets.
        let initial
            = [ &b"one"[..], &b"two"[..], &b"three"[..], &b"four"[..] ].to_vec();
        let inserted
            = [ &b"a"[..], &b"b"[..], &b"c"[..] ].to_vec();

        let mut packets : Vec<Packet> = Vec::new();
        for text in initial.iter() {
            let mut lit = Literal::new(Text);
            lit.set_body(text.to_vec());
            packets.push(lit.into())
        }

        for start in 0..initial.len() + 1 {
            for delete in 0..initial.len() - start + 1 {
                for insert in 0..inserted.len() + 1 {
                    let mut pile = PacketPile::from(packets.clone());

                    let mut replacement : Vec<Packet> = Vec::new();
                    for &text in inserted[0..insert].iter() {
                        let mut lit = Literal::new(Text);
                        lit.set_body(text.to_vec());
                        replacement.push(lit.into());
                    }

                    pile.replace(&[ start ], delete, replacement).unwrap();

                    let values = pile
                        .children()
                        .map(|p| {
                            if let Packet::Literal(ref literal) = p {
                                literal.body()
                            } else {
                                panic!("Expected a literal packet, got: {:?}", p);
                            }
                        })
                        .collect::<Vec<&[u8]>>();

                    assert_eq!(values.len(), initial.len() - delete + insert);

                    assert_eq!(values[..start],
                               initial[..start]);
                    assert_eq!(values[start..start + insert],
                               inserted[..insert]);
                    assert_eq!(values[start + insert..],
                               initial[start + delete..]);
                }
            }
        }


        // Like above, but the packets to replace are not at the
        // top-level, but in a compressed data packet.

        let initial
            = [ &b"one"[..], &b"two"[..], &b"three"[..], &b"four"[..] ].to_vec();
        let inserted
            = [ &b"a"[..], &b"b"[..], &b"c"[..] ].to_vec();

        let mut cd = CompressedData::new(CompressionAlgorithm::Uncompressed);
        for l in initial.iter() {
            let mut lit = Literal::new(Text);
            lit.set_body(l.to_vec());
            cd = cd.push(lit.into());
        }

        for start in 0..initial.len() + 1 {
            for delete in 0..initial.len() - start + 1 {
                for insert in 0..inserted.len() + 1 {
                    let mut pile = PacketPile::from(
                        vec![ cd.clone().into() ]);

                    let mut replacement : Vec<Packet> = Vec::new();
                    for &text in inserted[0..insert].iter() {
                        let mut lit = Literal::new(Text);
                        lit.set_body(text.to_vec());
                        replacement.push(lit.into());
                    }

                    pile.replace(&[ 0, start ], delete, replacement).unwrap();

                    let top_level = pile.children().collect::<Vec<&Packet>>();
                    assert_eq!(top_level.len(), 1);

                    let values = top_level[0]
                        .children().unwrap()
                        .map(|p| {
                            if let Packet::Literal(ref literal) = p {
                                literal.body()
                            } else {
                                panic!("Expected a literal packet, got: {:?}", p);
                            }
                        })
                        .collect::<Vec<&[u8]>>();

                    assert_eq!(values.len(), initial.len() - delete + insert);

                    assert_eq!(values[..start],
                               initial[..start]);
                    assert_eq!(values[start..start + insert],
                               inserted[..insert]);
                    assert_eq!(values[start + insert..],
                               initial[start + delete..]);
                }
            }
        }

        // Make sure out-of-range accesses error out.
        let mut one = Literal::new(Text);
        one.set_body(b"one".to_vec());
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(one.into());
        let mut pile = PacketPile::from(packets.clone());

        assert!(pile.replace(&[ 1 ], 0, Vec::new()).is_ok());
        assert!(pile.replace(&[ 2 ], 0, Vec::new()).is_err());
        assert!(pile.replace(&[ 0 ], 2, Vec::new()).is_err());
        assert!(pile.replace(&[ 0, 0 ], 0, Vec::new()).is_err());
        assert!(pile.replace(&[ 0, 1 ], 0, Vec::new()).is_err());

        // Try the same thing, but with a container.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                     .into());
        let mut pile = PacketPile::from(packets.clone());

        assert!(pile.replace(&[ 1 ], 0, Vec::new()).is_ok());
        assert!(pile.replace(&[ 2 ], 0, Vec::new()).is_err());
        assert!(pile.replace(&[ 0 ], 2, Vec::new()).is_err());
        // Since this is a container, this should be okay.
        assert!(pile.replace(&[ 0, 0 ], 0, Vec::new()).is_ok());
        assert!(pile.replace(&[ 0, 1 ], 0, Vec::new()).is_err());
    }
}
