use core::cmp::Ordering;

use alloc::fmt::{self, Display, Formatter};
use alloc::string::String;

use crate::{get_bytes, Byte, ByteUnit};

#[derive(Debug, Clone, Copy)]
/// Generated from the `get_appropriate_unit` and `get_adjusted_unit` methods of a `Byte` object.
pub struct AdjustedByte {
    pub(crate) value: f64,
    pub(crate) unit: ByteUnit,
}

impl AdjustedByte {
    /// Format the `AdjustedByte` object to string.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte = Byte::from_unit(1555f64, ByteUnit::KB).unwrap();
    ///
    /// let result = byte.get_appropriate_unit(false).format(3);
    ///
    /// assert_eq!("1.555 MB", result);
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte = Byte::from_unit(1555.2f64, ByteUnit::B).unwrap();
    ///
    /// let result = byte.get_adjusted_unit(ByteUnit::B).format(3);
    ///
    /// assert_eq!("1555 B", result);
    /// ```
    #[inline]
    pub fn format(&self, fractional_digits: usize) -> String {
        if self.unit == ByteUnit::B {
            format!("{:.0} B", self.value)
        } else {
            format!("{:.*} {}", fractional_digits, self.value, self.unit)
        }
    }

    #[inline]
    pub fn get_value(&self) -> f64 {
        self.value
    }

    #[inline]
    pub fn get_unit(&self) -> ByteUnit {
        self.unit
    }

    /// Create a new `Byte` object from this `AdjustedByte` object. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte = Byte::from_str("123456789123456").unwrap();
    /// let adjusted_byte = byte.get_adjusted_unit(ByteUnit::GB);
    ///
    /// assert_eq!(123456.789123456, adjusted_byte.get_value());
    ///
    /// let byte = adjusted_byte.get_byte();
    /// let adjusted_byte = byte.get_adjusted_unit(ByteUnit::GB);
    ///
    /// assert_eq!(123456.789123, adjusted_byte.get_value());
    /// ```
    #[inline]
    pub fn get_byte(&self) -> Byte {
        let bytes = get_bytes(self.value, self.unit);

        Byte::from_bytes(bytes)
    }
}

impl Display for AdjustedByte {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        if self.unit == ByteUnit::B {
            f.write_fmt(format_args!("{:.0} B", self.value))
        } else {
            f.write_fmt(format_args!("{:.2} ", self.value))?;

            Display::fmt(&self.unit, f)
        }
    }
}

impl PartialEq for AdjustedByte {
    /// Deal with the logical numeric equivalent.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte1 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    /// let byte2 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    ///
    /// assert_eq!(byte1.get_appropriate_unit(false), byte2.get_appropriate_unit(true));
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte1 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    /// let byte2 = Byte::from_unit(1f64, ByteUnit::MiB).unwrap();
    ///
    /// assert_eq!(byte1.get_appropriate_unit(true), byte2.get_appropriate_unit(false));
    /// ```
    #[inline]
    fn eq(&self, other: &AdjustedByte) -> bool {
        let s = self.get_byte();
        let o = other.get_byte();

        s.eq(&o)
    }
}

impl Eq for AdjustedByte {}

impl PartialOrd for AdjustedByte {
    #[inline]
    fn partial_cmp(&self, other: &AdjustedByte) -> Option<Ordering> {
        let s = self.get_byte();
        let o = other.get_byte();

        s.partial_cmp(&o)
    }
}

impl Ord for AdjustedByte {
    /// Deal with the logical numeric comparation.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte1 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    /// let byte2 = Byte::from_unit(1025f64, ByteUnit::KiB).unwrap();
    ///
    /// assert!(byte1.get_appropriate_unit(false) < byte2.get_appropriate_unit(true));
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte1 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    /// let byte2 = Byte::from_unit(1.01f64, ByteUnit::MiB).unwrap();
    ///
    /// assert!(byte1.get_appropriate_unit(true) < byte2.get_appropriate_unit(false));
    /// ```
    #[inline]
    fn cmp(&self, other: &AdjustedByte) -> Ordering {
        let s = self.get_byte();
        let o = other.get_byte();

        s.cmp(&o)
    }
}
