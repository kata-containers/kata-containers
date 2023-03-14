use crate::*;
use alloc::format;
use core::convert::TryFrom;
use nom::Needed;

mod f32;
mod f64;
pub use self::f32::*;
pub use self::f64::*;

/// ASN.1 `REAL` type
///
/// # Limitations
///
/// When encoding binary values, only base 2 is supported
#[derive(Debug, PartialEq)]
pub enum Real {
    /// Non-special values
    Binary {
        mantissa: f64,
        base: u32,
        exponent: i32,
        enc_base: u8,
    },
    /// Infinity (∞).
    Infinity,
    /// Negative infinity (−∞).
    NegInfinity,
    /// Zero
    Zero,
}

impl Real {
    /// Create a new `REAL` from the `f64` value.
    pub fn new(f: f64) -> Self {
        if f.is_infinite() {
            if f.is_sign_positive() {
                Self::Infinity
            } else {
                Self::NegInfinity
            }
        } else if f.abs() == 0.0 {
            Self::Zero
        } else {
            let mut e = 0;
            let mut f = f;
            while f.fract() != 0.0 {
                f *= 10.0_f64;
                e -= 1;
            }
            Real::Binary {
                mantissa: f,
                base: 10,
                exponent: e,
                enc_base: 10,
            }
            .normalize_base10()
        }
    }

    pub const fn with_enc_base(self, enc_base: u8) -> Self {
        match self {
            Real::Binary {
                mantissa,
                base,
                exponent,
                ..
            } => Real::Binary {
                mantissa,
                base,
                exponent,
                enc_base,
            },
            e => e,
        }
    }

    fn normalize_base10(self) -> Self {
        match self {
            Real::Binary {
                mantissa,
                base: 10,
                exponent,
                enc_base: _enc_base,
            } => {
                let mut m = mantissa;
                let mut e = exponent;
                while m.abs() > f64::EPSILON && m.rem_euclid(10.0).abs() < f64::EPSILON {
                    m /= 10.0;
                    e += 1;
                }
                Real::Binary {
                    mantissa: m,
                    base: 10,
                    exponent: e,
                    enc_base: _enc_base,
                }
            }
            _ => self,
        }
    }

    /// Create a new binary `REAL`
    #[inline]
    pub const fn binary(mantissa: f64, base: u32, exponent: i32) -> Self {
        Self::Binary {
            mantissa,
            base,
            exponent,
            enc_base: 2,
        }
    }

    /// Returns `true` if this value is positive infinity or negative infinity, and
    /// `false` otherwise.
    #[inline]
    pub fn is_infinite(&self) -> bool {
        matches!(self, Real::Infinity | Real::NegInfinity)
    }

    /// Returns `true` if this number is not infinite.
    #[inline]
    pub fn is_finite(&self) -> bool {
        matches!(self, Real::Zero | Real::Binary { .. })
    }

    /// Returns the 'f64' value of this `REAL`.
    ///
    /// Returned value is a float, and may be infinite.
    pub fn f64(&self) -> f64 {
        match self {
            Real::Binary {
                mantissa,
                base,
                exponent,
                ..
            } => {
                let f = *mantissa as f64;
                let exp = (*base as f64).powi(*exponent);
                f * exp
            }
            Real::Zero => 0.0_f64,
            Real::Infinity => f64::INFINITY,
            Real::NegInfinity => f64::NEG_INFINITY,
        }
    }

    /// Returns the 'f32' value of this `REAL`.
    ///
    /// This functions casts the result of [`Real::f64`] to a `f32`, and loses precision.
    pub fn f32(&self) -> f32 {
        self.f64() as f32
    }
}

impl<'a> TryFrom<Any<'a>> for Real {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self> {
        any.tag().assert_eq(Self::TAG)?;
        any.header.assert_primitive()?;
        let data = &any.data;
        if data.is_empty() {
            return Ok(Real::Zero);
        }
        // code inspired from pyasn1
        let first = data[0];
        let rem = &data[1..];
        if first & 0x80 != 0 {
            // binary encoding (X.690 section 8.5.6)
            let rem = rem;
            // format of exponent
            let (n, rem) = match first & 0x03 {
                4 => {
                    let (b, rem) = rem
                        .split_first()
                        .ok_or_else(|| Error::Incomplete(Needed::new(1)))?;
                    (*b as usize, rem)
                }
                b => (b as usize + 1, rem),
            };
            if n >= rem.len() {
                return Err(any.tag().invalid_value("Invalid float value(exponent)"));
            }
            // n cannot be 0 (see the +1 above)
            let (eo, rem) = rem.split_at(n);
            // so 'eo' cannot be empty
            let mut e = if eo[0] & 0x80 != 0 { -1 } else { 0 };
            // safety check: 'eo' length must be <= container type for 'e'
            if eo.len() > 4 {
                return Err(any.tag().invalid_value("Exponent too large (REAL)"));
            }
            for b in eo {
                e = (e << 8) | (*b as i32);
            }
            // base bits
            let b = (first >> 4) & 0x03;
            let _enc_base = match b {
                0 => 2,
                1 => 8,
                2 => 16,
                _ => return Err(any.tag().invalid_value("Illegal REAL encoding base")),
            };
            let e = match b {
                // base 2
                0 => e,
                // base 8
                1 => e * 3,
                // base 16
                2 => e * 4,
                _ => return Err(any.tag().invalid_value("Illegal REAL base")),
            };
            if rem.len() > 8 {
                return Err(any.tag().invalid_value("Mantissa too large (REAL)"));
            }
            let mut p = 0;
            for b in rem {
                p = (p << 8) | (*b as i64);
            }
            // sign bit
            let p = if first & 0x40 != 0 { -p } else { p };
            // scale bits
            let sf = (first >> 2) & 0x03;
            let p = match sf {
                0 => p as f64,
                sf => {
                    // 2^sf: cannot overflow, sf is between 0 and 3
                    let scale = 2_f64.powi(sf as _);
                    (p as f64) * scale
                }
            };
            Ok(Real::Binary {
                mantissa: p,
                base: 2,
                exponent: e,
                enc_base: _enc_base,
            })
        } else if first & 0x40 != 0 {
            // special real value (X.690 section 8.5.8)
            // there shall be only one contents octet,
            if any.header.length != Length::Definite(1) {
                return Err(Error::InvalidLength);
            }
            // with values as follows
            match first {
                0x40 => Ok(Real::Infinity),
                0x41 => Ok(Real::NegInfinity),
                _ => Err(any.tag().invalid_value("Invalid float special value")),
            }
        } else {
            // decimal encoding (X.690 section 8.5.7)
            let s = alloc::str::from_utf8(rem)?;
            match first & 0x03 {
                0x1 => {
                    // NR1
                    match s.parse::<u32>() {
                        Err(_) => Err(any.tag().invalid_value("Invalid float string encoding")),
                        Ok(v) => Ok(Real::new(v.into())),
                    }
                }
                0x2 /* NR2 */ | 0x3 /* NR3 */=> {
                    match s.parse::<f64>() {
                        Err(_) => Err(any.tag().invalid_value("Invalid float string encoding")),
                        Ok(v) => Ok(Real::new(v)),
                    }
                        }
                c => {
                    return Err(any.tag().invalid_value(&format!("Invalid NR ({})", c)));
                }
            }
        }
    }
}

impl<'a> CheckDerConstraints for Real {
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.assert_primitive()?;
        any.header.length.assert_definite()?;
        // XXX more checks
        Ok(())
    }
}

impl DerAutoDerive for Real {}

impl Tagged for Real {
    const TAG: Tag = Tag::RealType;
}

#[cfg(feature = "std")]
impl ToDer for Real {
    fn to_der_len(&self) -> Result<usize> {
        match self {
            Real::Zero => Ok(0),
            Real::Infinity | Real::NegInfinity => Ok(1),
            Real::Binary { .. } => {
                let mut sink = std::io::sink();
                let n = self
                    .write_der_content(&mut sink)
                    .map_err(|_| Self::TAG.invalid_value("Serialization of REAL failed"))?;
                Ok(n)
            }
        }
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let header = Header::new(
            Class::Universal,
            false,
            Self::TAG,
            Length::Definite(self.to_der_len()?),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        match self {
            Real::Zero => Ok(0),
            Real::Infinity => writer.write(&[0x40]).map_err(Into::into),
            Real::NegInfinity => writer.write(&[0x41]).map_err(Into::into),
            Real::Binary {
                mantissa,
                base,
                exponent,
                enc_base: _enc_base,
            } => {
                if *base == 10 {
                    // using character form
                    let sign = if *exponent == 0 { "+" } else { "" };
                    let s = format!("\x03{}E{}{}", mantissa, sign, exponent);
                    return writer.write(s.as_bytes()).map_err(Into::into);
                }
                if *base != 2 {
                    return Err(Self::TAG.invalid_value("Invalid base for REAL").into());
                }
                let mut first: u8 = 0x80;
                // choose encoding base
                let enc_base = *_enc_base;
                let (ms, mut m, enc_base, mut e) =
                    drop_floating_point(*mantissa, enc_base, *exponent);
                assert!(m != 0);
                if ms < 0 {
                    first |= 0x40
                };
                // exponent & mantissa normalization
                match enc_base {
                    2 => {
                        while m & 0x1 == 0 {
                            m >>= 1;
                            e += 1;
                        }
                    }
                    8 => {
                        while m & 0x7 == 0 {
                            m >>= 3;
                            e += 1;
                        }
                        first |= 0x10;
                    }
                    _ /* 16 */ => {
                        while m & 0xf == 0 {
                            m >>= 4;
                            e += 1;
                        }
                        first |= 0x20;
                    }
                }
                // scale factor
                // XXX in DER, sf is always 0 (11.3.1)
                let mut sf = 0;
                while m & 0x1 == 0 && sf < 4 {
                    m >>= 1;
                    sf += 1;
                }
                first |= sf << 2;
                // exponent length and bytes
                let len_e = match e.abs() {
                    0..=0xff => 1,
                    0x100..=0xffff => 2,
                    0x1_0000..=0xff_ffff => 3,
                    // e is an `i32` so it can't be longer than 4 bytes
                    // use 4, so `first` is ORed with 3
                    _ => 4,
                };
                first |= (len_e - 1) & 0x3;
                // write first byte
                let mut n = writer.write(&[first])?;
                // write exponent
                // special case: number of bytes from exponent is > 3 and cannot fit in 2 bits
                #[allow(clippy::identity_op)]
                if len_e == 4 {
                    let b = len_e & 0xff;
                    n += writer.write(&[b])?;
                }
                // we only need to write e.len() bytes
                let bytes = e.to_be_bytes();
                n += writer.write(&bytes[(4 - len_e) as usize..])?;
                // write mantissa
                let bytes = m.to_be_bytes();
                let mut idx = 0;
                for &b in bytes.iter() {
                    if b != 0 {
                        break;
                    }
                    idx += 1;
                }
                n += writer.write(&bytes[idx..])?;
                Ok(n)
            }
        }
    }
}

impl From<f32> for Real {
    fn from(f: f32) -> Self {
        Real::new(f.into())
    }
}

impl From<f64> for Real {
    fn from(f: f64) -> Self {
        Real::new(f)
    }
}

impl From<Real> for f32 {
    fn from(r: Real) -> Self {
        r.f32()
    }
}

impl From<Real> for f64 {
    fn from(r: Real) -> Self {
        r.f64()
    }
}

#[cfg(feature = "std")]
fn drop_floating_point(m: f64, b: u8, e: i32) -> (i8, u64, u8, i32) {
    let ms = if m.is_sign_positive() { 1 } else { -1 };
    let es = if e.is_positive() { 1 } else { -1 };
    let mut m = m.abs();
    let mut e = e;
    //
    if b == 8 {
        m *= 2_f64.powi((e.abs() / 3) * es);
        e = (e.abs() / 3) * es;
    } else if b == 16 {
        m *= 2_f64.powi((e.abs() / 4) * es);
        e = (e.abs() / 4) * es;
    }
    //
    while m.abs() > f64::EPSILON {
        if m.fract() != 0.0 {
            m *= b as f64;
            e -= 1;
        } else {
            break;
        }
    }
    (ms, m as u64, b, e)
}
