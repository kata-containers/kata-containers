use crate::{Any, CheckDerConstraints, DerAutoDerive, Error, Real, Result, Tag, Tagged};
use core::convert::{TryFrom, TryInto};

impl<'a> TryFrom<Any<'a>> for f64 {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<f64> {
        any.tag().assert_eq(Self::TAG)?;
        any.header.assert_primitive()?;
        let real: Real = any.try_into()?;
        Ok(real.f64())
    }
}

impl<'a> CheckDerConstraints for f64 {
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.assert_primitive()?;
        any.header.length.assert_definite()?;
        Ok(())
    }
}

impl DerAutoDerive for f64 {}

impl Tagged for f64 {
    const TAG: Tag = Tag::RealType;
}
