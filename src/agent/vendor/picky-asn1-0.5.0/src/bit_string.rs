use core::fmt;
use serde::{de, ser};

/// A bit string.
///
/// Rewrite based on [this implementation](https://melvinw.github.io/rust-asn1/asn1/struct.BitString.html) by Melvin Walls Jr.
/// licensed with
///
/// > The MIT License (MIT)
/// >
/// > Copyright (c) 2016 Melvin Walls Jr.
/// >
/// > Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
/// >
/// > The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
/// >
/// > THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
///
/// # Examples
///
/// ```
/// use picky_asn1::bit_string::BitString;
///
/// let mut b = BitString::with_len(60);
///
/// b.set(0, true);
/// assert_eq!(b.is_set(0), true);
///
/// b.set(59, true);
/// assert_eq!(b.is_set(59), true);
///
/// // because len is 60, attempts at setting anything greater than 59 won't change anything
/// b.set(63, true);
/// assert_eq!(b.is_set(63), false);
/// ```
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone)]
pub struct BitString {
    data: Vec<u8>,
}

impl BitString {
    fn h_number_of_unused_bits(data_size: usize, num_bits: usize) -> u8 {
        (data_size * 8 - num_bits) as u8
    }

    /// Construct a `BitString` of length `n` with all bits set to 0.
    pub fn with_len(num_bits: usize) -> BitString {
        let data_size = num_bits / 8 + if num_bits % 8 == 0 { 0 } else { 1 };
        let mut data = vec![0x00u8; data_size + 1];
        data[0] = Self::h_number_of_unused_bits(data_size, num_bits);
        BitString { data }
    }

    /// Construct a `BitString` of length `n` with initial values contained in `data`.
    ///
    /// # Examples
    ///
    /// ```
    /// use picky_asn1::bit_string::BitString;
    ///
    /// let v: Vec<u8> = vec![0x00, 0x02];
    /// let b = BitString::with_bytes_and_len(v, 15);
    /// assert_eq!(b.is_set(0), false);
    /// assert_eq!(b.is_set(14), true);
    ///
    /// // because len is 15, everything greater than 14 will returns false
    /// assert_eq!(b.is_set(15), false);
    /// assert_eq!(b.is_set(938), false);
    /// ```
    pub fn with_bytes_and_len<V>(data: V, num_bits: usize) -> BitString
    where
        V: Into<Vec<u8>>,
    {
        let mut data = data.into();
        let number_of_unused = Self::h_number_of_unused_bits(data.len(), num_bits);
        data.insert(0, number_of_unused);
        BitString { data }
    }

    /// Construct a `BitString` from initial values contained in `data`.
    /// Length is inferred fromthe size of `data`.
    ///
    /// # Examples
    ///
    /// ```
    /// use picky_asn1::bit_string::BitString;
    ///
    /// let v: Vec<u8> = vec![0x00, 0x02];
    /// let b = BitString::with_bytes(v);
    /// assert_eq!(b.is_set(0), false);
    /// assert_eq!(b.is_set(14), true);
    /// ```
    pub fn with_bytes<V>(data: V) -> BitString
    where
        V: Into<Vec<u8>>,
    {
        let mut data = data.into();
        data.insert(0, 0); // no unused bits
        BitString { data }
    }

    /// Get the number of available bits in the `BitString`
    pub fn get_num_bits(&self) -> usize {
        (self.data.len() - 1) * 8 - self.data[0] as usize
    }

    /// Set the length of a `BitString` with each additional slot filled with 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use picky_asn1::bit_string::BitString;
    ///
    /// let v: Vec<u8> = vec![0x01, 0x01];
    /// let mut b = BitString::with_bytes_and_len(v, 16);
    /// assert_eq!(b.is_set(7), true);
    /// assert_eq!(b.is_set(15), true);
    ///
    /// b.set_num_bits(8);
    /// assert_eq!(b.is_set(7), true);
    /// b.set(15, true); // attempts to set a value out of the bounds are ignored
    /// assert_eq!(b.is_set(15), false);
    ///
    /// b.set_num_bits(16);
    /// assert_eq!(b.is_set(7), true);
    /// assert_eq!(b.is_set(15), false);
    /// b.set(15, true);
    /// assert_eq!(b.is_set(15), true);
    /// ```
    pub fn set_num_bits(&mut self, num_bits: usize) {
        let new_size = num_bits / 8 + if num_bits % 8 == 0 { 0 } else { 1 };
        self.data[0] = Self::h_number_of_unused_bits(new_size, num_bits);
        self.data.resize(new_size + 1, 0);
    }

    /// Check if bit `i` is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use picky_asn1::bit_string::BitString;
    ///
    /// let mut b = BitString::with_len(10);
    /// assert_eq!(b.is_set(7), false);
    /// b.set(7, true);
    /// assert_eq!(b.is_set(7), true);
    /// ```
    pub fn is_set(&self, i: usize) -> bool {
        if i > self.get_num_bits() {
            return false;
        }

        let bucket = i / 8;
        let pos = i - bucket * 8;
        let mask = (1 << (7 - pos)) as u8;
        self.data[bucket + 1] & mask != 0
    }

    /// Set bit `i` to `val`.
    pub fn set(&mut self, i: usize, val: bool) {
        if i > self.get_num_bits() {
            return;
        }

        let bucket = i / 8;
        let pos = i - bucket * 8;
        let mask = (1 << (7 - pos)) as u8;

        if val {
            self.data[bucket + 1] |= mask;
        } else {
            self.data[bucket + 1] &= !mask;
        }
    }

    pub fn get_num_unused_bits(&self) -> u8 {
        self.data[0]
    }

    pub fn get_num_buckets(&self) -> usize {
        self.data.len() - 1
    }

    pub fn get_bucket(&self, i: usize) -> u8 {
        self.data[i + 1]
    }

    pub fn get_bucket_mut(&mut self, i: usize) -> &mut u8 {
        &mut self.data[i + 1]
    }

    pub fn set_bucket(&mut self, i: usize, value: u8) {
        self.data[i + 1] = value
    }

    /// Returns an immutabe view on the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// use picky_asn1::bit_string::BitString;
    ///
    /// let v: Vec<u8> = vec![0x01, 0x00];
    /// let mut b = BitString::with_bytes_and_len(v, 15);
    /// b.set(14, true);
    /// let payload = b.payload_view();
    /// assert_eq!(payload, &[0x01, 0x02]);
    /// ```
    pub fn payload_view(&self) -> &[u8] {
        &self.data[1..]
    }

    /// Returns a mutabe view on the payload.
    ///
    /// # Examples
    ///
    /// ```
    /// use picky_asn1::bit_string::BitString;
    ///
    /// let v: Vec<u8> = vec![0x01, 0x00];
    /// let mut b = BitString::with_bytes_and_len(v, 15);
    /// b.set(14, true);
    /// let payload = b.payload_view_mut();
    /// payload[0] = 0x20;
    /// assert_eq!(payload, &[0x20, 0x02]);
    /// ```
    pub fn payload_view_mut(&mut self) -> &mut [u8] {
        &mut self.data[1..]
    }
}

impl From<BitString> for Vec<u8> {
    /// Strips 'unused bits count' byte and returns payload.
    ///
    /// # Examples
    ///
    /// ```
    /// use picky_asn1::bit_string::BitString;
    ///
    /// let v: Vec<u8> = vec![0x01, 0x00];
    /// let mut b = BitString::with_bytes_and_len(v, 15);
    /// b.set(14, true);
    /// let payload: Vec<u8> = b.into();
    /// assert_eq!(payload, vec![0x01, 0x02]);
    /// ```
    fn from(mut bs: BitString) -> Self {
        bs.data.drain(1..).collect()
    }
}

impl<'de> de::Deserialize<'de> for BitString {
    fn deserialize<D>(deserializer: D) -> Result<BitString, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = BitString;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid buffer representing a bit string")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_byte_buf(v.to_vec())
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(BitString { data: v })
            }
        }

        deserializer.deserialize_byte_buf(Visitor)
    }
}

impl ser::Serialize for BitString {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_bytes(&self.data)
    }
}

impl Default for BitString {
    fn default() -> Self {
        BitString::with_len(0)
    }
}
