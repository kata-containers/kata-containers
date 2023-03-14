use crate::{DynTagged, Error, Result, Tag};
#[cfg(feature = "std")]
use crate::{SerializeResult, ToDer};
use core::ops;

/// BER Object Length
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Length {
    /// Definite form (X.690 8.1.3.3)
    Definite(usize),
    /// Indefinite form (X.690 8.1.3.6)
    Indefinite,
}

impl Length {
    /// Return true if length is definite and equal to 0
    #[inline]
    pub fn is_null(&self) -> bool {
        *self == Length::Definite(0)
    }

    /// Get length of primitive object
    #[inline]
    pub fn definite(&self) -> Result<usize> {
        match self {
            Length::Definite(sz) => Ok(*sz),
            Length::Indefinite => Err(Error::IndefiniteLengthUnexpected),
        }
    }

    /// Return true if length is definite
    #[inline]
    pub const fn is_definite(&self) -> bool {
        matches!(self, Length::Definite(_))
    }

    /// Return error if length is not definite
    #[inline]
    pub const fn assert_definite(&self) -> Result<()> {
        match self {
            Length::Definite(_) => Ok(()),
            Length::Indefinite => Err(Error::IndefiniteLengthUnexpected),
        }
    }
}

impl From<usize> for Length {
    fn from(l: usize) -> Self {
        Length::Definite(l)
    }
}

impl ops::Add<Length> for Length {
    type Output = Self;

    fn add(self, rhs: Length) -> Self::Output {
        match self {
            Length::Indefinite => self,
            Length::Definite(lhs) => match rhs {
                Length::Indefinite => rhs,
                Length::Definite(rhs) => Length::Definite(lhs + rhs),
            },
        }
    }
}

impl ops::Add<usize> for Length {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        match self {
            Length::Definite(lhs) => Length::Definite(lhs + rhs),
            Length::Indefinite => self,
        }
    }
}

impl ops::AddAssign<usize> for Length {
    fn add_assign(&mut self, rhs: usize) {
        match self {
            Length::Definite(ref mut lhs) => *lhs += rhs,
            Length::Indefinite => (),
        }
    }
}

impl DynTagged for Length {
    fn tag(&self) -> Tag {
        Tag(0)
    }
}

#[cfg(feature = "std")]
impl ToDer for Length {
    fn to_der_len(&self) -> Result<usize> {
        match self {
            Length::Indefinite => Ok(1),
            Length::Definite(l) => match l {
                0..=0x7f => Ok(1),
                0x80..=0xff => Ok(2),
                0x100..=0xffff => Ok(3),
                0x1_0000..=0xffff_ffff => Ok(4),
                _ => Err(Error::InvalidLength),
            },
        }
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        match *self {
            Length::Indefinite => {
                let sz = writer.write(&[0b1000_0000])?;
                Ok(sz)
            }
            Length::Definite(l) => {
                if l <= 127 {
                    // Short form
                    let sz = writer.write(&[l as u8])?;
                    Ok(sz)
                } else {
                    // Long form
                    let b = l.to_be_bytes();
                    // skip leading zeroes
                    // we do not have to test for length, l cannot be 0
                    let mut idx = 0;
                    while b[idx] == 0 {
                        idx += 1;
                    }
                    let b = &b[idx..];
                    // first byte: 0x80 + length of length
                    let b0 = 0x80 | (b.len() as u8);
                    let sz = writer.write(&[b0])?;
                    let sz = sz + writer.write(b)?;
                    Ok(sz)
                }
            }
        }
    }

    fn write_der_content(&self, _writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    /// Generic and coverage tests
    #[test]
    fn methods_length() {
        let l = Length::from(2);
        assert_eq!(l.definite(), Ok(2));
        assert!(l.assert_definite().is_ok());

        let l = Length::Indefinite;
        assert!(l.definite().is_err());
        assert!(l.assert_definite().is_err());

        let l = Length::from(2);
        assert_eq!(l + 2, Length::from(4));
        assert_eq!(l + Length::Indefinite, Length::Indefinite);

        let l = Length::Indefinite;
        assert_eq!(l + 2, Length::Indefinite);

        let l = Length::from(2);
        assert_eq!(l + Length::from(2), Length::from(4));

        let l = Length::Indefinite;
        assert_eq!(l + Length::from(2), Length::Indefinite);

        let mut l = Length::from(2);
        l += 2;
        assert_eq!(l.definite(), Ok(4));

        let mut l = Length::Indefinite;
        l += 2;
        assert_eq!(l, Length::Indefinite);
    }
}
