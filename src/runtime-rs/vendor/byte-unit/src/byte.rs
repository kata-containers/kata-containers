use core::convert::TryFrom;
use core::str::FromStr;

#[cfg(feature = "serde")]
use alloc::string::String;

use core::fmt::{self, Display, Formatter};

use crate::{
    get_char_from_bytes, read_xib, AdjustedByte, ByteError, ByteUnit, ValueIncorrectError,
};

#[cfg(feature = "serde")]
use crate::serde::ser::{Serialize, Serializer};

#[cfg(feature = "serde")]
use crate::serde::de::{Deserialize, Deserializer, Error as DeError, Unexpected, Visitor};

#[cfg(feature = "u128")]
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, Default)]
/// Represent the n-bytes data. Use associated functions: `from_unit`, `from_bytes`, `from_str`, to create the instance.
pub struct Byte(u128);

#[cfg(not(feature = "u128"))]
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, Default)]
/// Represent the n-bytes data. Use associated functions: `from_unit`, `from_bytes`, `from_str`, to create the instance.
pub struct Byte(u64);

impl Byte {
    /// Create a new `Byte` object from a specified value and a unit. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_unit(1500f64, ByteUnit::KB).unwrap();
    ///
    /// assert_eq!(1500000, result.get_bytes());
    /// ```
    #[inline]
    pub fn from_unit(value: f64, unit: ByteUnit) -> Result<Byte, ByteError> {
        if value < 0f64 {
            return Err(ValueIncorrectError::Negative(value).into());
        }

        let bytes = get_bytes(value, unit);

        Ok(Byte(bytes))
    }

    /// Create a new `Byte` object from bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_bytes(1500000);
    ///
    /// assert_eq!(1500000, result.get_bytes());
    /// ```
    #[cfg(feature = "u128")]
    #[inline]
    pub const fn from_bytes(bytes: u128) -> Byte {
        Byte(bytes)
    }

    /// Create a new `Byte` object from bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_bytes(1500000);
    ///
    /// assert_eq!(1500000, result.get_bytes());
    /// ```
    #[cfg(not(feature = "u128"))]
    #[inline]
    pub const fn from_bytes(bytes: u64) -> Byte {
        Byte(bytes)
    }

    /// Create a new `Byte` object from string. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("123KiB").unwrap();
    ///
    /// assert_eq!(Byte::from_unit(123f64, ByteUnit::KiB).unwrap(), result);
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("50.84 MB").unwrap();
    ///
    /// assert_eq!(Byte::from_unit(50.84f64, ByteUnit::MB).unwrap(), result);
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 B").unwrap(); // 8 bytes
    ///
    /// assert_eq!(8, result.get_bytes());
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8").unwrap(); // 8 bytes
    ///
    /// assert_eq!(8, result.get_bytes());
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 b").unwrap(); // 8 bytes
    ///
    /// assert_eq!(8, result.get_bytes());
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 kb").unwrap(); // 8 kilobytes
    ///
    /// assert_eq!(8000, result.get_bytes());
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 kib").unwrap(); // 8 kibibytes
    ///
    /// assert_eq!(8192, result.get_bytes());
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 k").unwrap(); // 8 kilobytes
    ///
    /// assert_eq!(8000, result.get_bytes());
    /// ```
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str<S: AsRef<str>>(s: S) -> Result<Byte, ByteError> {
        let s = s.as_ref().trim();

        let mut bytes = s.bytes();

        let mut value = match bytes.next() {
            Some(e) => {
                match e {
                    b'0'..=b'9' => f64::from(e - b'0'),
                    _ => {
                        return Err(
                            ValueIncorrectError::NotNumber(get_char_from_bytes(e, bytes)).into()
                        );
                    }
                }
            }
            None => return Err(ValueIncorrectError::NoValue.into()),
        };

        let e = 'outer: loop {
            match bytes.next() {
                Some(e) => {
                    match e {
                        b'0'..=b'9' => {
                            value = value * 10.0 + f64::from(e - b'0');
                        }
                        b'.' => {
                            let mut i = 0.1;

                            loop {
                                match bytes.next() {
                                    Some(e) => {
                                        match e {
                                            b'0'..=b'9' => {
                                                value += f64::from(e - b'0') * i;

                                                i /= 10.0;
                                            }
                                            _ => {
                                                if (i * 10.0) as u8 == 1 {
                                                    return Err(ValueIncorrectError::NotNumber(
                                                        get_char_from_bytes(e, bytes),
                                                    )
                                                    .into());
                                                }

                                                match e {
                                                    b' ' => {
                                                        loop {
                                                            match bytes.next() {
                                                                Some(e) => {
                                                                    match e {
                                                                        b' ' => (),
                                                                        _ => break 'outer Some(e),
                                                                    }
                                                                }
                                                                None => break 'outer None,
                                                            }
                                                        }
                                                    }
                                                    _ => break 'outer Some(e),
                                                }
                                            }
                                        }
                                    }
                                    None => {
                                        if (i * 10.0) as u8 == 1 {
                                            return Err(ValueIncorrectError::NotNumber(
                                                get_char_from_bytes(e, bytes),
                                            )
                                            .into());
                                        }

                                        break 'outer None;
                                    }
                                }
                            }
                        }
                        b' ' => {
                            loop {
                                match bytes.next() {
                                    Some(e) => {
                                        match e {
                                            b' ' => (),
                                            _ => break 'outer Some(e),
                                        }
                                    }
                                    None => break 'outer None,
                                }
                            }
                        }
                        _ => break 'outer Some(e),
                    }
                }
                None => break None,
            }
        };

        let unit = read_xib(e, bytes)?;

        let bytes = get_bytes(value, unit);

        Ok(Byte(bytes))
    }
}

impl Byte {
    /// Get bytes represented by a `Byte` object.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::Byte;
    ///
    /// let byte = Byte::from_str("123KiB").unwrap();
    ///
    /// let result = byte.get_bytes();
    ///
    /// assert_eq!(125952, result);
    /// ```
    ///
    /// ```
    /// use byte_unit::Byte;
    ///
    /// let byte = Byte::from_str("50.84 MB").unwrap();
    ///
    /// let result = byte.get_bytes();
    ///
    /// assert_eq!(50840000, result);
    /// ```
    #[cfg(feature = "u128")]
    #[inline]
    pub const fn get_bytes(&self) -> u128 {
        self.0
    }

    /// Get bytes represented by a `Byte` object.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::Byte;
    ///
    /// let byte = Byte::from_str("123KiB").unwrap();
    ///
    /// let result = byte.get_bytes();
    ///
    /// assert_eq!(125952, result);
    /// ```
    ///
    /// ```
    /// use byte_unit::Byte;
    ///
    /// let byte = Byte::from_str("50.84 MB").unwrap();
    ///
    /// let result = byte.get_bytes();
    ///
    /// assert_eq!(50840000, result);
    /// ```
    #[cfg(not(feature = "u128"))]
    #[inline]
    pub const fn get_bytes(&self) -> u64 {
        self.0
    }

    /// Adjust the unit and value for `Byte` object. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte = Byte::from_str("123KiB").unwrap();
    ///
    /// let adjusted_byte = byte.get_adjusted_unit(ByteUnit::KB);
    ///
    /// assert_eq!("125.95 KB", adjusted_byte.to_string());
    /// ```
    ///
    /// ```
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let byte = Byte::from_str("50.84 MB").unwrap();
    ///
    /// let adjusted_byte = byte.get_adjusted_unit(ByteUnit::MiB);
    ///
    /// assert_eq!("48.48 MiB", adjusted_byte.to_string());
    /// ```
    #[inline]
    pub fn get_adjusted_unit(&self, unit: ByteUnit) -> AdjustedByte {
        let bytes_f64 = self.0 as f64;

        let value = bytes_f64 / unit.get_unit_bytes() as f64;

        AdjustedByte {
            value,
            unit,
        }
    }

    /// Find the appropriate unit and value for `Byte` object. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// use byte_unit::Byte;
    ///
    /// let byte = Byte::from_str("123KiB").unwrap();
    ///
    /// let adjusted_byte = byte.get_appropriate_unit(false);
    ///
    /// assert_eq!("125.95 KB", adjusted_byte.to_string());
    /// ```
    ///
    /// ```
    /// use byte_unit::Byte;
    ///
    /// let byte = Byte::from_str("50.84 MB").unwrap();
    ///
    /// let adjusted_byte = byte.get_appropriate_unit(true);
    ///
    /// assert_eq!("48.48 MiB", adjusted_byte.to_string());
    /// ```
    #[allow(clippy::collapsible_if)]
    pub fn get_appropriate_unit(&self, binary_multiples: bool) -> AdjustedByte {
        let bytes = self.0;

        if binary_multiples {
            #[cfg(feature = "u128")]
            {
                if bytes > n_zib_bytes!() {
                    return self.get_adjusted_unit(ByteUnit::ZiB);
                } else if bytes > n_eib_bytes!() {
                    return self.get_adjusted_unit(ByteUnit::EiB);
                }
            }

            if bytes > n_pib_bytes!() {
                self.get_adjusted_unit(ByteUnit::PiB)
            } else if bytes > n_tib_bytes!() {
                self.get_adjusted_unit(ByteUnit::TiB)
            } else if bytes > n_gib_bytes!() {
                self.get_adjusted_unit(ByteUnit::GiB)
            } else if bytes > n_mib_bytes!() {
                self.get_adjusted_unit(ByteUnit::MiB)
            } else if bytes > n_kib_bytes!() {
                self.get_adjusted_unit(ByteUnit::KiB)
            } else {
                self.get_adjusted_unit(ByteUnit::B)
            }
        } else {
            #[cfg(feature = "u128")]
            {
                if bytes > n_zb_bytes!() {
                    return self.get_adjusted_unit(ByteUnit::ZB);
                } else if bytes > n_eb_bytes!() {
                    return self.get_adjusted_unit(ByteUnit::EB);
                }
            }

            if bytes > n_pb_bytes!() {
                self.get_adjusted_unit(ByteUnit::PB)
            } else if bytes > n_tb_bytes!() {
                self.get_adjusted_unit(ByteUnit::TB)
            } else if bytes > n_gb_bytes!() {
                self.get_adjusted_unit(ByteUnit::GB)
            } else if bytes > n_mb_bytes!() {
                self.get_adjusted_unit(ByteUnit::MB)
            } else if bytes > n_kb_bytes!() {
                self.get_adjusted_unit(ByteUnit::KB)
            } else {
                self.get_adjusted_unit(ByteUnit::B)
            }
        }
    }
}

impl Display for Byte {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.write_fmt(format_args!("{}", self.0))
    }
}

#[cfg(feature = "u128")]
impl From<Byte> for u128 {
    #[inline]
    fn from(b: Byte) -> u128 {
        b.0
    }
}

#[cfg(not(feature = "u128"))]
impl From<Byte> for u64 {
    #[inline]
    fn from(b: Byte) -> u64 {
        b.0
    }
}

#[cfg(feature = "u128")]
impl From<u128> for Byte {
    #[inline]
    fn from(u: u128) -> Self {
        Byte::from_bytes(u)
    }
}

#[cfg(feature = "u128")]
impl From<usize> for Byte {
    #[inline]
    fn from(u: usize) -> Self {
        Byte::from_bytes(u as u128)
    }
}

#[cfg(not(feature = "u128"))]
impl From<usize> for Byte {
    #[inline]
    fn from(u: usize) -> Self {
        Byte::from_bytes(u as u64)
    }
}

#[cfg(feature = "u128")]
impl From<u64> for Byte {
    #[inline]
    fn from(u: u64) -> Self {
        Byte::from_bytes(u as u128)
    }
}

#[cfg(not(feature = "u128"))]
impl From<u64> for Byte {
    #[inline]
    fn from(u: u64) -> Self {
        Byte::from_bytes(u)
    }
}

#[cfg(feature = "u128")]
impl From<u32> for Byte {
    #[inline]
    fn from(u: u32) -> Self {
        Byte::from_bytes(u as u128)
    }
}

#[cfg(not(feature = "u128"))]
impl From<u32> for Byte {
    #[inline]
    fn from(u: u32) -> Self {
        Byte::from_bytes(u as u64)
    }
}

#[cfg(feature = "u128")]
impl From<u16> for Byte {
    #[inline]
    fn from(u: u16) -> Self {
        Byte::from_bytes(u as u128)
    }
}

#[cfg(not(feature = "u128"))]
impl From<u16> for Byte {
    #[inline]
    fn from(u: u16) -> Self {
        Byte::from_bytes(u as u64)
    }
}

#[cfg(feature = "u128")]
impl From<u8> for Byte {
    #[inline]
    fn from(u: u8) -> Self {
        Byte::from_bytes(u as u128)
    }
}

#[cfg(not(feature = "u128"))]
impl From<u8> for Byte {
    #[inline]
    fn from(u: u8) -> Self {
        Byte::from_bytes(u as u64)
    }
}

impl TryFrom<&str> for Byte {
    type Error = ByteError;

    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Byte::from_str(s)
    }
}

impl FromStr for Byte {
    type Err = ByteError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Byte::from_str(s)
    }
}

#[cfg(feature = "u128")]
#[inline]
pub(crate) fn get_bytes(value: f64, unit: ByteUnit) -> u128 {
    match unit {
        ByteUnit::B => value as u128,
        ByteUnit::KB => n_kb_bytes!(value, f64),
        ByteUnit::KiB => n_kib_bytes!(value, f64),
        ByteUnit::MB => n_mb_bytes!(value, f64),
        ByteUnit::MiB => n_mib_bytes!(value, f64),
        ByteUnit::GB => n_gb_bytes!(value, f64),
        ByteUnit::GiB => n_gib_bytes!(value, f64),
        ByteUnit::TB => n_tb_bytes!(value, f64),
        ByteUnit::TiB => n_tib_bytes!(value, f64),
        ByteUnit::PB => n_pb_bytes!(value, f64),
        ByteUnit::PiB => n_pib_bytes!(value, f64),
        ByteUnit::EB => n_eb_bytes!(value, f64),
        ByteUnit::EiB => n_eib_bytes!(value, f64),
        ByteUnit::ZB => n_zb_bytes!(value, f64),
        ByteUnit::ZiB => n_zib_bytes!(value, f64),
    }
}

#[cfg(not(feature = "u128"))]
#[inline]
pub(crate) fn get_bytes(value: f64, unit: ByteUnit) -> u64 {
    match unit {
        ByteUnit::B => value as u64,
        ByteUnit::KB => n_kb_bytes!(value, f64),
        ByteUnit::KiB => n_kib_bytes!(value, f64),
        ByteUnit::MB => n_mb_bytes!(value, f64),
        ByteUnit::MiB => n_mib_bytes!(value, f64),
        ByteUnit::GB => n_gb_bytes!(value, f64),
        ByteUnit::GiB => n_gib_bytes!(value, f64),
        ByteUnit::TB => n_tb_bytes!(value, f64),
        ByteUnit::TiB => n_tib_bytes!(value, f64),
        ByteUnit::PB => n_pb_bytes!(value, f64),
        ByteUnit::PiB => n_pib_bytes!(value, f64),
    }
}

#[cfg(feature = "serde")]
impl Serialize for Byte {
    #[allow(unreachable_code)]
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        #[cfg(feature = "u128")]
        {
            serde_if_integer128! {
                return serializer.serialize_u128(self.get_bytes());
            }

            unreachable!("the `integer128` feature of the `serde` crate needs to be enabled")
        }

        #[cfg(not(feature = "u128"))]
        {
            serializer.serialize_u64(self.get_bytes())
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Byte {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>, {
        struct ByteVisitor;

        impl<'de> Visitor<'de> for ByteVisitor {
            type Value = Byte;

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
                                Ok(Byte::from_bytes(v as u128))
                            }

                        #[cfg(not(feature = "u128"))]
                            {
                                if v > u64::MAX as i128 {
                                    Err(DeError::invalid_value(Unexpected::Other(format!("integer `{}`", v).as_str()), &self))
                                } else {
                                    Ok(Byte::from_bytes(v as u64))
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
                            Ok(Byte::from_bytes(v))
                        }

                    #[cfg(not(feature = "u128"))]
                        {
                            if v > u64::MAX as u128 {
                                Err(DeError::invalid_value(Unexpected::Other(format!("integer `{}`", v).as_str()), &self))
                            } else {
                                Ok(Byte::from_bytes(v as u64))
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
                Byte::from_str(v).map_err(DeError::custom)
            }

            #[inline]
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: DeError, {
                Byte::from_str(v.as_str()).map_err(DeError::custom)
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
                        Ok(Byte::from_bytes(v as u128))
                    }

                    #[cfg(not(feature = "u128"))]
                    {
                        Ok(Byte::from_bytes(v as u64))
                    }
                }
            }

            #[inline]
            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: DeError, {
                #[cfg(feature = "u128")]
                {
                    Ok(Byte::from_bytes(v as u128))
                }

                #[cfg(not(feature = "u128"))]
                {
                    Ok(Byte::from_bytes(v))
                }
            }

            #[inline]
            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: DeError, {
                Byte::from_unit(v, ByteUnit::B).map_err(DeError::custom)
            }
        }

        deserializer.deserialize_any(ByteVisitor)
    }
}
