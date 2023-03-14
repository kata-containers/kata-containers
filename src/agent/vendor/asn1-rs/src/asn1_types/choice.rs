use crate::{FromBer, FromDer, Tag, Tagged};

pub trait Choice {
    /// Is the provided [`Tag`] decodable as a variant of this `CHOICE`?
    fn can_decode(tag: Tag) -> bool;
}

/// This blanket impl allows any [`Tagged`] type to function as a [`Choice`]
/// with a single alternative.
impl<T> Choice for T
where
    T: Tagged,
{
    fn can_decode(tag: Tag) -> bool {
        T::TAG == tag
    }
}

pub trait BerChoice<'a>: Choice + FromBer<'a> {}

pub trait DerChoice<'a>: Choice + FromDer<'a> {}
