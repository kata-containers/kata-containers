//! Convenient downcasting from Packets to Packet Bodies.

use crate::packet::{
    Packet,
    Unknown,
    Signature,
    OnePassSig,
    key::{
        PublicKey,
        PublicSubkey,
        SecretKey,
        SecretSubkey,
    },
    Marker,
    Trust,
    UserID,
    UserAttribute,
    Literal,
    CompressedData,
    PKESK,
    SKESK,
    SEIP,
    MDC,
    AED,
};

/// Convenient downcasting from Packets to Packet Bodies.
///
/// This trait offers functionality similar to [`std::any::Any`],
/// hence the name.
///
/// # Sealed trait
///
/// This trait is [sealed] and cannot be implemented for types outside
/// this crate.  Therefore it can be extended in a non-breaking way.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait Any<T>: crate::seal::Sealed {
    /// Attempts to downcast to `T`, returning the packet if it fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp::packet::prelude::*;
    /// let p: Packet = Marker::default().into();
    /// let m: Marker = p.downcast().unwrap();
    /// # let _ = m;
    /// ```
    fn downcast(self) -> std::result::Result<T, Packet>;

    /// Attempts to downcast to `&T`, returning `None` if it fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp::packet::prelude::*;
    /// let p: Packet = Marker::default().into();
    /// let m: &Marker = p.downcast_ref().unwrap();
    /// # let _ = m;
    /// ```
    fn downcast_ref(&self) -> Option<&T>;

    /// Attempts to downcast to `&mut T`, returning `None` if it fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp::packet::prelude::*;
    /// let mut p: Packet = Marker::default().into();
    /// let m: &mut Marker = p.downcast_mut().unwrap();
    /// # let _ = m;
    /// ```
    fn downcast_mut(&mut self) -> Option<&mut T>;
}

macro_rules! impl_downcast_for {
    ($typ: tt) => {
        impl Any<$typ> for Packet {
            fn downcast(self) -> std::result::Result<$typ, Packet> {
                match self {
                    Packet::$typ(v) => Ok(v),
                    p => Err(p),
                }
            }

            fn downcast_ref(&self) -> Option<&$typ> {
                match self {
                    Packet::$typ(v) => Some(v),
                    _ => None,
                }
            }

            fn downcast_mut(&mut self) -> Option<&mut $typ> {
                match self {
                    Packet::$typ(v) => Some(v),
                    _ => None,
                }
            }
        }
    };
}

macro_rules! impl_downcasts {
    ($($typ:ident, )*) => {
        $(impl_downcast_for!($typ);)*

        /// Checks that all packet types have implementations of `Any`.
        ///
        /// Not visible outside this module, isn't supposed to be
        /// called, this is a compile-time check.
        #[allow(unused)]
        fn check_exhaustion(p: Packet) {
            match p {
                $(Packet::$typ(_) => (),)*
            }
        }
    }
}

impl_downcasts!(
    Unknown,
    Signature,
    OnePassSig,
    PublicKey,
    PublicSubkey,
    SecretKey,
    SecretSubkey,
    Marker,
    Trust,
    UserID,
    UserAttribute,
    Literal,
    CompressedData,
    PKESK,
    SKESK,
    SEIP,
    MDC,
    AED,
);


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn downcast() {
        let p: Packet = Marker::default().into();
        let mut p = Any::<UserID>::downcast(p).unwrap_err();
        let r: Option<&UserID> = p.downcast_ref();
        assert!(r.is_none());
        let r: Option<&mut UserID> = p.downcast_mut();
        assert!(r.is_none());
        let _: &Marker = p.downcast_ref().unwrap();
        let _: &mut Marker = p.downcast_mut().unwrap();
        let _: Marker = p.downcast().unwrap();
    }
}
