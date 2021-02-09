//! Base Endianness type module.

use byteorder::{BigEndian, ByteOrder, LittleEndian, NativeEndian, ReadBytesExt, WriteBytesExt};
use std::default::Default;
use std::io::{Read, Result as IoResult, Write};
use std::marker::PhantomData;

/// Trait for any type which has an opposite type. This is used to convert
/// immaterial types representing "little endian" into "big endian" and vice
/// versa.
pub trait HasOpposite: private::Sealed {
    type Opposite;
}

impl HasOpposite for LittleEndian {
    type Opposite = BigEndian;
}

impl HasOpposite for BigEndian {
    type Opposite = LittleEndian;
}

/// Trait for identifying whether a type is representative of the system's
/// native byte order.
pub trait StaticNative: private::Sealed {
    /// Checks whether this type represents the system's native endianness.
    fn is_native() -> bool;
}

impl StaticNative for NativeEndian {
    fn is_native() -> bool {
        true
    }
}

#[cfg(target_endian = "little")]
impl StaticNative for BigEndian {
    fn is_native() -> bool {
        false
    }
}

#[cfg(target_endian = "big")]
impl StaticNative for LittleEndian {
    fn is_native() -> bool {
        false
    }
}

/// General trait for types that can serialize and deserialize bytes in some
/// byte order. It roughly resembles [`byteorder::ByteOrder`], with the
/// exception that it is implemented for material types. This makes it possible
/// to embed byte order information to a reader or writer by composition
/// (which is done by [`ByteOrdered`]).
///
/// [`byteorder::ByteOrder`]: https://docs.rs/byteorder/*/byteorder/trait.ByteOrder.html
/// [`ByteOrdered`]: struct.ByteOrdered.html
pub trait Endian: private::Sealed {
    /// A type which can represent a byte order that is opposite to this one.
    type Opposite;

    /// Checks whether this value represents the system's native endianness.
    fn is_native(&self) -> bool;

    /// Converts the receiver into its opposite.
    fn into_opposite(self) -> Self::Opposite;

    /// Reads a signed 16 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_i16<R>(&self, reader: R) -> IoResult<i16>
    where
        R: Read;

    /// Reads an unsigned 16 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_u16<R>(&self, reader: R) -> IoResult<u16>
    where
        R: Read;

    /// Reads a signed 32 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_i32<R>(&self, reader: R) -> IoResult<i32>
    where
        R: Read;

    /// Reads an unsigned 32 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_u32<R>(&self, reader: R) -> IoResult<u32>
    where
        R: Read;

    /// Reads a signed 64 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_i64<R>(&self, reader: R) -> IoResult<i64>
    where
        R: Read;

    /// Reads an unsigned 16 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_u64<R>(&self, reader: R) -> IoResult<u64>
    where
        R: Read;

    /// Reads a signed 128 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_i128<R>(&self, reader: R) -> IoResult<i128>
    where
        R: Read;

    /// Reads an unsigned 16 bit integer from the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_u128<R>(&self, reader: R) -> IoResult<u128>
    where
        R: Read;

    /// Reads a IEEE754 single-precision (4 bytes) floating point number from
    /// the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_f32<R>(&self, reader: R) -> IoResult<f32>
    where
        R: Read;

    /// Reads a IEEE754 double-precision (8 bytes) floating point number from
    /// the given reader.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Read::read_exact`].
    ///
    /// [`Read::read_exact`]: https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    fn read_f64<R>(&self, reader: R) -> IoResult<f64>
    where
        R: Read;

    /// Writes a signed 16 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_i16<W>(&self, writer: W, v: i16) -> IoResult<()>
    where
        W: Write;

    /// Writes an unsigned 16 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_u16<W>(&self, writer: W, v: u16) -> IoResult<()>
    where
        W: Write;

    /// Writes a signed 32 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_i32<W>(&self, writer: W, v: i32) -> IoResult<()>
    where
        W: Write;

    /// Writes an unsigned 32 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_u32<W>(&self, writer: W, v: u32) -> IoResult<()>
    where
        W: Write;

    /// Writes a signed 64 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_i64<W>(&self, writer: W, v: i64) -> IoResult<()>
    where
        W: Write;

    /// Writes an unsigned 64 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_u64<W>(&self, writer: W, v: u64) -> IoResult<()>
    where
        W: Write;

    /// Writes a signed 128 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_i128<W>(&self, writer: W, v: i128) -> IoResult<()>
    where
        W: Write;

    /// Writes an unsigned 128 bit integer to the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_u128<W>(&self, writer: W, v: u128) -> IoResult<()>
    where
        W: Write;

    /// Writes a IEEE754 single-precision (4 bytes) floating point number to
    /// the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_f32<W>(&self, writer: W, v: f32) -> IoResult<()>
    where
        W: Write;

    /// Writes a IEEE754 double-precision (8 bytes) floating point number to
    /// the given writer.
    ///
    /// # Errors
    ///
    /// This method returns the same errors as [`Write::write_all`].
    ///
    /// [`Write::write_all`]: https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all
    fn write_f64<W>(&self, writer: W, v: f64) -> IoResult<()>
    where
        W: Write;
}

/// A data type representing a byte order known in compile time.
/// Unlike the types provided in `byteorder`, this type can be constructed.
///
/// The parameter type `E` can be one of either [`byteorder::BigEndian`][be]
/// or [`byteorder::LittleEndian`][le].
///
/// [be]: https://docs.rs/byteorder/*/byteorder/enum.BigEndian.html
/// [le]: https://docs.rs/byteorder/*/byteorder/enum.LittleEndian.html
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StaticEndianness<E>(PhantomData<E>);

impl<E> Default for StaticEndianness<E> {
    fn default() -> Self {
        StaticEndianness::new()
    }
}

impl<E> StaticEndianness<E> {
    /// Constructor for a static endianness.
    pub fn new() -> Self {
        StaticEndianness(PhantomData)
    }
}

impl StaticEndianness<NativeEndian> {
    /// Constructor for native endianness.
    pub fn native() -> Self {
        StaticEndianness::new()
    }
}

impl PartialEq<StaticEndianness<LittleEndian>> for StaticEndianness<BigEndian> {
    fn eq(&self, _: &StaticEndianness<LittleEndian>) -> bool {
        false
    }
}

impl PartialEq<StaticEndianness<BigEndian>> for StaticEndianness<LittleEndian> {
    fn eq(&self, _: &StaticEndianness<BigEndian>) -> bool {
        false
    }
}

impl PartialEq<Endianness> for StaticEndianness<BigEndian> {
    fn eq(&self, e: &Endianness) -> bool {
        *e == Endianness::Big
    }
}

impl PartialEq<Endianness> for StaticEndianness<LittleEndian> {
    fn eq(&self, e: &Endianness) -> bool {
        *e == Endianness::Little
    }
}

impl<E> HasOpposite for StaticEndianness<E>
where
    E: HasOpposite,
{
    type Opposite = StaticEndianness<E::Opposite>;
}

macro_rules! fn_static_endianness_read {
    ($method:ident, $e:ty, $out:ty) => {
        fn $method<S>(&self, mut src: S) -> IoResult<$out>
        where
            S: Read,
        {
            src.$method::< $e >()
        }
    };
}

macro_rules! fn_static_endianness_write {
    ($method:ident, $e:ty, $out:ty) => {
        fn $method<W>(&self, mut src: W, x: $out) -> IoResult<()>
        where
            W: Write,
        {
            src.$method::< $e >(x)
        }
    };
}

impl<E> Endian for StaticEndianness<E>
where
    E: HasOpposite,
    E: StaticNative,
    E: ByteOrder,
{
    type Opposite = StaticEndianness<E::Opposite>;

    fn into_opposite(self) -> Self::Opposite {
        StaticEndianness(PhantomData)
    }

    fn is_native(&self) -> bool {
        E::is_native()
    }

    fn_static_endianness_read!(read_i16, E, i16);
    fn_static_endianness_read!(read_u16, E, u16);
    fn_static_endianness_read!(read_i32, E, i32);
    fn_static_endianness_read!(read_u32, E, u32);
    fn_static_endianness_read!(read_i64, E, i64);
    fn_static_endianness_read!(read_u64, E, u64);
    fn_static_endianness_read!(read_i128, E, i128);
    fn_static_endianness_read!(read_u128, E, u128);
    fn_static_endianness_read!(read_f32, E, f32);
    fn_static_endianness_read!(read_f64, E, f64);

    fn_static_endianness_write!(write_i16, E, i16);
    fn_static_endianness_write!(write_u16, E, u16);
    fn_static_endianness_write!(write_i32, E, i32);
    fn_static_endianness_write!(write_u32, E, u32);
    fn_static_endianness_write!(write_i64, E, i64);
    fn_static_endianness_write!(write_u64, E, u64);
    fn_static_endianness_write!(write_i128, E, i128);
    fn_static_endianness_write!(write_u128, E, u128);
    fn_static_endianness_write!(write_f32, E, f32);
    fn_static_endianness_write!(write_f64, E, f64);
}

/// Enumerate for materializing the two kinds of machine byte order supported
/// by Rust.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, PartialOrd, Ord)]
pub enum Endianness {
    /// Little Endian
    Little,
    /// Big Endian
    Big,
}

impl From<StaticEndianness<LittleEndian>> for Endianness {
    fn from(_: StaticEndianness<LittleEndian>) -> Self {
        Endianness::Little
    }
}

impl From<StaticEndianness<BigEndian>> for Endianness {
    fn from(_: StaticEndianness<BigEndian>) -> Self {
        Endianness::Big
    }
}

impl PartialEq<StaticEndianness<BigEndian>> for Endianness {
    fn eq(&self, _: &StaticEndianness<BigEndian>) -> bool {
        *self == Endianness::Big
    }
}

impl PartialEq<StaticEndianness<LittleEndian>> for Endianness {
    fn eq(&self, _: &StaticEndianness<LittleEndian>) -> bool {
        *self == Endianness::Little
    }
}

macro_rules! fn_runtime_endianness_read {
    ($method:ident, $out:ty) => {
        fn $method<S>(&self, mut src: S) -> IoResult<$out>
        where
            S: Read,
        {
            match *self {
                Endianness::Little => src.$method::<LittleEndian>(),
                Endianness::Big => src.$method::<BigEndian>(),
            }
        }
    };
}

macro_rules! fn_runtime_endianness_write {
    ($method:ident, $i:ty) => {
        fn $method<S>(&self, mut src: S, v: $i) -> IoResult<()>
        where
            S: Write,
        {
            match *self {
                Endianness::Little => src.$method::<LittleEndian>(v),
                Endianness::Big => src.$method::<BigEndian>(v),
            }
        }
    };
}

impl HasOpposite for Endianness {
    type Opposite = Self;
}

impl Endian for Endianness {
    type Opposite = Self;

    fn into_opposite(self) -> Self::Opposite {
        self.to_opposite()
    }

    fn is_native(&self) -> bool {
        *self == Endianness::native()
    }

    fn_runtime_endianness_read!(read_i16, i16);
    fn_runtime_endianness_read!(read_u16, u16);
    fn_runtime_endianness_read!(read_i32, i32);
    fn_runtime_endianness_read!(read_u32, u32);
    fn_runtime_endianness_read!(read_i64, i64);
    fn_runtime_endianness_read!(read_u64, u64);
    fn_runtime_endianness_read!(read_f32, f32);
    fn_runtime_endianness_read!(read_f64, f64);
    fn_runtime_endianness_read!(read_i128, i128);
    fn_runtime_endianness_read!(read_u128, u128);

    fn_runtime_endianness_write!(write_i16, i16);
    fn_runtime_endianness_write!(write_u16, u16);
    fn_runtime_endianness_write!(write_i32, i32);
    fn_runtime_endianness_write!(write_u32, u32);
    fn_runtime_endianness_write!(write_i64, i64);
    fn_runtime_endianness_write!(write_u64, u64);
    fn_runtime_endianness_write!(write_f32, f32);
    fn_runtime_endianness_write!(write_f64, f64);
    fn_runtime_endianness_write!(write_i128, i128);
    fn_runtime_endianness_write!(write_u128, u128);
}

impl Endianness {
    /// Obtains this system's native endianness.
    ///
    /// On this platform, the function returns `Endianness::Little`.
    #[cfg(target_endian = "little")]
    #[inline]
    pub fn native() -> Self {
        Endianness::Little
    }

    /// Obtains this system's native endianness.
    ///
    /// On this platform, the function returns `Endianness::Big`.
    #[cfg(target_endian = "big")]
    #[inline]
    pub fn native() -> Self {
        Endianness::Big
    }

    /// Obtains _Little Endian_ if and only if the given value is `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use byteordered::{Endian, Endianness};
    /// let data: &[u8] = &[4, 1];
    /// let e = Endianness::le_iff(2 + 2 == 4);
    /// assert_eq!(e.read_u16(data).unwrap(), 260);
    ///
    /// let e = Endianness::le_iff(2 + 2 >= 5);
    /// assert_eq!(e.read_u16(data).unwrap(), 1025);
    /// ```
    #[inline]
    pub fn le_iff(e: bool) -> Self {
        if e {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    /// Obtains _Big Endian_ if and only if the given value is `true`.
    ///
    /// Examples
    ///
    /// ```
    /// # use byteordered::Endianness;
    /// assert_eq!(Endianness::be_iff(2 + 2 == 4), Endianness::Big);
    /// assert_eq!(Endianness::be_iff(2 + 2 >= 5), Endianness::Little);
    /// ```
    #[inline]
    pub fn be_iff(e: bool) -> Self {
        if e {
            Endianness::Big
        } else {
            Endianness::Little
        }
    }

    /// Obtains the opposite endianness: Little Endian returns Big Endian and vice versa.
    #[inline]
    pub fn to_opposite(self) -> Self {
        if self == Endianness::Little {
            Endianness::Big
        } else {
            Endianness::Little
        }
    }
}

mod private {
    use super::{Endianness, StaticEndianness};
    use byteorder::{BigEndian, LittleEndian};
    pub trait Sealed {}

    impl Sealed for LittleEndian {}
    impl Sealed for BigEndian {}
    impl<T> Sealed for StaticEndianness<T> {}
    impl Sealed for Endianness {}
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_BYTES: &'static [u8] = &[0x12, 0x34, 0x56, 0x78, 0x21, 0x43, 0x65, 0x87];

    static TEST_U64DATA_LE: &'static [u64] = &[0x87654321_78563412];
    static TEST_U64DATA_BE: &'static [u64] = &[0x12345678_21436587];

    #[test]
    fn test_read_u64() {
        let mut data = TEST_BYTES;
        let e = Endianness::Little;
        let words = [e.read_u64(&mut data).unwrap()];
        assert_eq!(words, TEST_U64DATA_LE);

        let mut data = TEST_BYTES;
        let e = Endianness::Big;
        let words = [e.read_u64(&mut data).unwrap()];
        assert_eq!(words, TEST_U64DATA_BE);
    }

    static TEST_U32DATA_LE: &'static [u32] = &[0x7856_3412, 0x8765_4321];
    static TEST_U32DATA_BE: &'static [u32] = &[0x1234_5678, 0x2143_6587];

    #[test]
    fn test_read_u32() {
        let mut data = TEST_BYTES;
        let e = Endianness::Little;
        let words = [
            e.read_u32(&mut data).unwrap(),
            e.read_u32(&mut data).unwrap(),
        ];
        assert_eq!(words, TEST_U32DATA_LE);

        let mut data = TEST_BYTES;
        let e = Endianness::Big;
        let words = [
            e.read_u32(&mut data).unwrap(),
            e.read_u32(&mut data).unwrap(),
        ];
        assert_eq!(words, TEST_U32DATA_BE);
    }

    static TEST_U16DATA_LE: &'static [u16] = &[0x3412, 0x7856, 0x4321, 0x8765];
    static TEST_U16DATA_BE: &'static [u16] = &[0x1234, 0x5678, 0x2143, 0x6587];

    #[test]
    fn test_read_u16() {
        let mut data = TEST_BYTES;
        let e = Endianness::Little;
        let words = [
            e.read_u16(&mut data).unwrap(),
            e.read_u16(&mut data).unwrap(),
            e.read_u16(&mut data).unwrap(),
            e.read_u16(&mut data).unwrap(),
        ];
        assert_eq!(words, TEST_U16DATA_LE);

        let mut data = TEST_BYTES;
        let e = Endianness::Big;
        let words = [
            e.read_u16(&mut data).unwrap(),
            e.read_u16(&mut data).unwrap(),
            e.read_u16(&mut data).unwrap(),
            e.read_u16(&mut data).unwrap(),
        ];
        assert_eq!(words, TEST_U16DATA_BE);
    }

    #[test]
    fn test_native_is_le() {
        if cfg!(target_endian = "little") {
            assert_eq!(Endianness::native(), Endianness::Little);
        } else if cfg!(target_endian = "big") {
            assert_eq!(Endianness::native(), Endianness::Big);
        } else {
            unreachable!();
        }
    }

    // TODO test writing
}
