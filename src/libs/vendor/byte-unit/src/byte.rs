use core::str::FromStr;

use alloc::fmt::{self, Display, Formatter};
use alloc::string::String;

use crate::{read_xib, AdjustedByte, ByteError, ByteUnit};

#[cfg(feature = "u128")]
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
/// Represent the n-bytes data. Use associated functions: `from_unit`, `from_bytes`, `from_str`, to create the instance.
pub struct Byte(u128);

#[cfg(not(feature = "u128"))]
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
/// Represent the n-bytes data. Use associated functions: `from_unit`, `from_bytes`, `from_str`, to create the instance.
pub struct Byte(u64);

impl Byte {
    /// Create a new `Byte` object from a specified value and a unit. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_unit(1500f64, ByteUnit::KB).unwrap();
    ///
    /// assert_eq!(1500000, result.get_bytes());
    /// ```
    #[inline]
    pub fn from_unit(value: f64, unit: ByteUnit) -> Result<Byte, ByteError> {
        if value < 0f64 {
            return Err(ByteError::ValueIncorrect(format!(
                "The value `{}` for creating a `Byte` instance is negative.",
                value
            )));
        }

        let bytes = get_bytes(value, unit);

        Ok(Byte(bytes))
    }

    /// Create a new `Byte` object from bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_bytes(1500000);
    ///
    /// assert_eq!(1500000, result.get_bytes());
    /// ```
    #[cfg(feature = "u128")]
    #[inline]
    pub fn from_bytes(bytes: u128) -> Byte {
        Byte(bytes)
    }

    /// Create a new `Byte` object from bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_bytes(1500000);
    ///
    /// assert_eq!(1500000, result.get_bytes());
    /// ```
    #[cfg(not(feature = "u128"))]
    #[inline]
    pub fn from_bytes(bytes: u64) -> Byte {
        Byte(bytes)
    }

    /// Create a new `Byte` object from string. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("123KiB").unwrap();
    ///
    /// assert_eq!(Byte::from_unit(123f64, ByteUnit::KiB).unwrap(), result);
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("50.84 MB").unwrap();
    ///
    /// assert_eq!(Byte::from_unit(50.84f64, ByteUnit::MB).unwrap(), result);
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 B").unwrap(); // 8 bytes
    ///
    /// assert_eq!(8, result.get_bytes());
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8").unwrap(); // 8 bytes
    ///
    /// assert_eq!(8, result.get_bytes());
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 b").unwrap(); // 8 bytes
    ///
    /// assert_eq!(8, result.get_bytes());
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 kb").unwrap(); // 8 kilobytes
    ///
    /// assert_eq!(8000, result.get_bytes());
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
    /// use byte_unit::{Byte, ByteUnit};
    ///
    /// let result = Byte::from_str("8 kib").unwrap(); // 8 kibibytes
    ///
    /// assert_eq!(8192, result.get_bytes());
    /// ```
    ///
    /// ```
    /// extern crate byte_unit;
    ///
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

        let mut chars = s.chars();

        let mut value = match chars.next() {
            Some(c) => {
                match c {
                    '0'..='9' => f64::from(c as u8 - b'0'),
                    _ => {
                        return Err(ByteError::ValueIncorrect(format!(
                            "The character {:?} is not a number.",
                            c
                        )));
                    }
                }
            }
            None => return Err(ByteError::ValueIncorrect(String::from("No value."))),
        };

        let c = 'outer: loop {
            match chars.next() {
                Some(c) => {
                    match c {
                        '0'..='9' => {
                            value = value * 10.0 + f64::from(c as u8 - b'0');
                        }
                        '.' => {
                            let mut i = 0.1;

                            loop {
                                match chars.next() {
                                    Some(c) => {
                                        if c >= '0' && c <= '9' {
                                            value += f64::from(c as u8 - b'0') * i;

                                            i /= 10.0;
                                        } else {
                                            if (i * 10.0) as u8 == 1 {
                                                return Err(ByteError::ValueIncorrect(format!(
                                                    "The character {:?} is not a number.",
                                                    c
                                                )));
                                            }

                                            match c {
                                                ' ' => {
                                                    loop {
                                                        match chars.next() {
                                                            Some(c) => {
                                                                match c {
                                                                    ' ' => (),
                                                                    _ => break 'outer Some(c),
                                                                }
                                                            }
                                                            None => break 'outer None,
                                                        }
                                                    }
                                                }
                                                _ => break 'outer Some(c),
                                            }
                                        }
                                    }
                                    None => {
                                        if (i * 10.0) as u8 == 1 {
                                            return Err(ByteError::ValueIncorrect(format!(
                                                "The character {:?} is not a number.",
                                                c
                                            )));
                                        }

                                        break 'outer None;
                                    }
                                }
                            }
                        }
                        ' ' => {
                            loop {
                                match chars.next() {
                                    Some(c) => {
                                        match c {
                                            ' ' => (),
                                            _ => break 'outer Some(c),
                                        }
                                    }
                                    None => break 'outer None,
                                }
                            }
                        }
                        _ => break 'outer Some(c),
                    }
                }
                None => break None,
            }
        };

        let unit = read_xib(c, chars)?;

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
    /// extern crate byte_unit;
    ///
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
    /// extern crate byte_unit;
    ///
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
    pub fn get_bytes(&self) -> u128 {
        self.0
    }

    /// Get bytes represented by a `Byte` object.
    ///
    /// ## Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
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
    /// extern crate byte_unit;
    ///
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
    pub fn get_bytes(&self) -> u64 {
        self.0
    }

    /// Adjust the unit and value for `Byte` object. **Accuracy** should be taken care of.
    ///
    /// ## Examples
    ///
    /// ```
    /// extern crate byte_unit;
    ///
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
    /// extern crate byte_unit;
    ///
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
    /// extern crate byte_unit;
    ///
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
    /// extern crate byte_unit;
    ///
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
impl Into<u128> for Byte {
    #[inline]
    fn into(self) -> u128 {
        self.0
    }
}

#[cfg(not(feature = "u128"))]
impl Into<u64> for Byte {
    #[inline]
    fn into(self) -> u64 {
        self.0
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
        ByteUnit::TiB => n_gib_bytes!(value, f64),
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
        ByteUnit::TiB => n_gib_bytes!(value, f64),
        ByteUnit::PB => n_pb_bytes!(value, f64),
        ByteUnit::PiB => n_pib_bytes!(value, f64),
    }
}
