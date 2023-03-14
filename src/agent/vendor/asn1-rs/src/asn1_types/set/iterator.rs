pub use crate::{Error, SequenceIterator};

/// An Iterator over binary data, parsing elements of type `T`
///
/// This helps parsing `SET OF` items of type `T`. The type of parser
/// (BER/DER) is specified using the generic parameter `F` of this struct.
///
/// Note: the iterator must start on the set *contents*, not the set itself.
///
/// # Examples
///
/// ```rust
/// use asn1_rs::{DerParser, Integer, SetIterator};
///
/// let data = &[0x30, 0x6, 0x2, 0x1, 0x1, 0x2, 0x1, 0x2];
/// for (idx, item) in SetIterator::<Integer, DerParser>::new(&data[2..]).enumerate() {
///     let item = item.unwrap(); // parsing could have failed
///     let i = item.as_u32().unwrap(); // integer can be negative, or too large to fit into u32
///     assert_eq!(i as usize, idx + 1);
/// }
/// ```
pub type SetIterator<'a, T, F, E = Error> = SequenceIterator<'a, T, F, E>;
