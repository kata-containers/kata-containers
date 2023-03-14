use core::cmp::Ordering;

use core::fmt::{self, Display, Formatter};

#[cfg(feature = "alloc")]
use alloc::string::String;

use crate::{get_bytes, Byte, ByteUnit};

#[cfg(feature = "serde")]
use crate::serde::ser::{Serialize, Serializer};

#[cfg(feature = "serde")]
use crate::serde::de::{Deserialize, Deserializer, Error as DeError, Unexpected, Visitor};

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
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte = Byte::from_unit(1555.2f64, ByteUnit::B).unwrap();
    ///
    /// let result = byte.get_adjusted_unit(ByteUnit::B).format(3);
    ///
    /// assert_eq!("1555 B", result);
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
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
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte1 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    /// let byte2 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    ///
    /// assert_eq!(byte1.get_appropriate_unit(false), byte2.get_appropriate_unit(true));
    /// ```
    ///
    /// ```
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
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte1 = Byte::from_unit(1024f64, ByteUnit::KiB).unwrap();
    /// let byte2 = Byte::from_unit(1025f64, ByteUnit::KiB).unwrap();
    ///
    /// assert!(byte1.get_appropriate_unit(false) < byte2.get_appropriate_unit(true));
    /// ```
    ///
    /// ```
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

impl From<AdjustedByte> for Byte {
    #[inline]
    fn from(other: AdjustedByte) -> Byte {
        other.get_byte()
    }
}

#[cfg(feature = "serde")]
impl Serialize for AdjustedByte {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        serializer.serialize_str(self.format(2).as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for AdjustedByte {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>, {
        struct AdjustedByteVisitor;

        impl<'de> Visitor<'de> for AdjustedByteVisitor {
            type Value = AdjustedByte;

            serde_if_integer128! {
                #[inline]
                fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
                    where
                        E: DeError,
                {
                    if v < 0 {
                        Err(DeError::invalid_value(Unexpected::Other(format!("integer `{}`", v).as_str()), &self))
                    } else {
                        #[cfg(feature = "u128")]
                            {
                                Ok(Byte::from_bytes(v as u128).get_appropriate_unit(false))
                            }

                        #[cfg(not(feature = "u128"))]
                            {
                                if v > u64::MAX as i128 {
                                    Err(DeError::invalid_value(Unexpected::Other(format!("integer `{}`", v).as_str()), &self))
                                } else {
                                    Ok(Byte::from_bytes(v as u64).get_appropriate_unit(false))
                                }
                            }
                    }
                }

                #[inline]
                fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
                    where
                        E: DeError,
                {
                    #[cfg(feature = "u128")]
                        {
                            Ok(Byte::from_bytes(v).get_appropriate_unit(false))
                        }

                    #[cfg(not(feature = "u128"))]
                        {
                            if v > u64::MAX as u128 {
                                Err(DeError::invalid_value(Unexpected::Other(format!("integer `{}`", v).as_str()), &self))
                            } else {
                                Ok(Byte::from_bytes(v as u64).get_appropriate_unit(false))
                            }
                        }
                }
            }

            #[inline]
            fn expecting(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
                f.write_str("a byte such as 123, \"123\", \"123KiB\" or \"50.84 MB\"")
            }

            #[inline]
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError, {
                Byte::from_str(v).map(|b| b.get_appropriate_unit(false)).map_err(DeError::custom)
            }

            #[inline]
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: DeError, {
                Byte::from_str(v.as_str())
                    .map(|b| b.get_appropriate_unit(false))
                    .map_err(DeError::custom)
            }

            #[inline]
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: DeError, {
                if v < 0 {
                    Err(DeError::invalid_value(Unexpected::Signed(v), &self))
                } else {
                    #[cfg(feature = "u128")]
                    {
                        Ok(Byte::from_bytes(v as u128).get_appropriate_unit(false))
                    }

                    #[cfg(not(feature = "u128"))]
                    {
                        Ok(Byte::from_bytes(v as u64).get_appropriate_unit(false))
                    }
                }
            }

            #[inline]
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: DeError, {
                #[cfg(feature = "u128")]
                {
                    Ok(Byte::from_bytes(v as u128).get_appropriate_unit(false))
                }

                #[cfg(not(feature = "u128"))]
                {
                    Ok(Byte::from_bytes(v).get_appropriate_unit(false))
                }
            }

            #[inline]
            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: DeError, {
                Byte::from_unit(v, ByteUnit::B)
                    .map(|b| b.get_appropriate_unit(false))
                    .map_err(DeError::custom)
            }
        }

        deserializer.deserialize_any(AdjustedByteVisitor)
    }
}
