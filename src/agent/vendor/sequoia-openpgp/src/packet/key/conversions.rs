//! Conversion functions for `Key` and associated types.

use std::convert::TryFrom;

use crate::Error;
use crate::cert::prelude::*;
use crate::packet::prelude::*;
use crate::packet::key::{
    KeyParts,
    KeyRole,
    PrimaryRole,
    PublicParts,
    SubordinateRole,
    SecretParts,
    UnspecifiedParts,
    UnspecifiedRole
};
use crate::Result;

macro_rules! convert {
    ( $x:ident ) => {
        // XXX: This is ugly, but how can we do better?
        unsafe { std::mem::transmute($x) }
    }
}

macro_rules! convert_ref {
    ( $x:ident ) => {
        // XXX: This is ugly, but how can we do better?
        #[allow(clippy::transmute_ptr_to_ptr)]
        unsafe { std::mem::transmute($x) }
    }
}

// Make it possible to go from an arbitrary Key<P, R> to an
// arbitrary Key<P', R'> (or &Key<P, R> to &Key<P', R'>) in a
// single .into().
//
// To allow the programmer to make the intent clearer, also
// provide explicit conversion function.

// In principle, this is as easy as the following:
//
//     impl<P, P2, R, R2> From<Key<P, R>> for Key<P2, R2>
//         where P: KeyParts, P2: KeyParts, R: KeyRole, R2: KeyRole
//     {
//         fn from(p: Key<P, R>) -> Self {
//             unimplemented!()
//         }
//     }
//
// But that results in:
//
//     error[E0119]: conflicting implementations of trait `std::convert::From<packet::Key<_, _>>` for type `packet::Key<_, _>`:
//     = note: conflicting implementation in crate `core`:
//             - impl<T> std::convert::From<T> for T;
//
// Unfortunately, it's not enough to make one type variable
// concrete, as the following errors demonstrate:
//
//     error[E0119]: conflicting implementations of trait `std::convert::From<packet::Key<packet::key::PublicParts, _>>` for type `packet::Key<packet::key::PublicParts, _>`:
//     ...
//         = note: conflicting implementation in crate `core`:
//                 - impl<T> std::convert::From<T> for T;
//
//     impl<P, R, R2> From<Key<P, R>> for Key<PublicParts, R2>
//         where P: KeyParts, R: KeyRole, R2: KeyRole
//     {
//         fn from(p: Key<P, R>) -> Self {
//             unimplemented!()
//         }
//     }
//
//   error[E0119]: conflicting implementations of trait `std::convert::From<packet::Key<packet::key::PublicParts, _>>` for type `packet::Key<packet::key::PublicParts, _>`:
//      --> openpgp/src/packet/key.rs:186:5
//   ...
//       = note: conflicting implementation in crate `core`:
//               - impl<T> std::convert::From<T> for T;
//   impl<P2, R, R2> From<Key<PublicParts, R>> for Key<P2, R2>
//       where P2: KeyParts, R: KeyRole, R2: KeyRole
//   {
//       fn from(p: Key<PublicParts, R>) -> Self {
//           unimplemented!()
//       }
//   }
//
// To solve this, we need at least one generic variable to be
// concrete on both sides of the `From`.

macro_rules! create_part_conversions {
    ( $Key:ident<$( $l:lifetime ),*; $( $g:ident ),*>) => {
        create_part_conversions!($Key<$($l),*; $($g),*> where );
    };
    ( $Key:ident<$( $l:lifetime ),*; $( $g:ident ),*> where $( $w:ident: $c:path ),* ) => {
        // Convert between two KeyParts for a constant KeyRole.
        // Unfortunately, we can't let the KeyRole vary as otherwise we
        // get conflicting types when we do the same to convert between
        // two KeyRoles for a constant KeyParts. :(
        macro_rules! p {
            ( <$from_parts:ty> -> <$to_parts:ty> ) => {
                impl<$($l, )* $($g, )* > From<$Key<$($l, )* $from_parts, $($g, )* >> for $Key<$($l, )* $to_parts, $($g, )* >
                    where $($w: $c ),*
                {
                    fn from(p: $Key<$($l, )* $from_parts, $($g, )* >) -> Self {
                        convert!(p)
                    }
                }

                impl<$($l, )* $($g, )* > From<&$($l)* $Key<$($l, )* $from_parts, $($g, )* >> for &$($l)* $Key<$($l, )* $to_parts, $($g, )* >
                    where $($w: $c ),*
                {
                    fn from(p: &$($l)* $Key<$($l, )* $from_parts, $($g, )* >) -> Self {
                        convert_ref!(p)
                    }
                }
            }
        }

        // Likewise, but using TryFrom.
        macro_rules! p_try {
            ( <$from_parts:ty> -> <$to_parts:ty>) => {
                impl<$($l, )* $($g, )* > TryFrom<$Key<$($l, )* $from_parts, $($g, )* >> for $Key<$($l, )* $to_parts, $($g, )* >
                    where $($w: $c ),*
                {
                    type Error = anyhow::Error;
                    fn try_from(p: $Key<$($l, )* $from_parts, $($g, )* >) -> Result<Self> {
                        p.parts_into_secret()
                    }
                }

                impl<$($l, )* $($g, )* > TryFrom<&$($l)* $Key<$($l, )* $from_parts, $($g, )* >> for &$($l)* $Key<$($l, )* $to_parts, $($g, )* >
                    where $($w: $c ),*
                {
                    type Error = anyhow::Error;
                    fn try_from(p: &$($l)* $Key<$($l, )* $from_parts, $($g, )* >) -> Result<Self> {
                        if p.has_secret() {
                            Ok(convert_ref!(p))
                        } else {
                            Err(Error::InvalidArgument("No secret key".into())
                                .into())
                        }
                    }
                }
            }
        }


        p_try!(<PublicParts> -> <SecretParts>);
        p!(<PublicParts> -> <UnspecifiedParts>);

        p!(<SecretParts> -> <PublicParts>);
        p!(<SecretParts> -> <UnspecifiedParts>);

        p!(<UnspecifiedParts> -> <PublicParts>);
        p_try!(<UnspecifiedParts> -> <SecretParts>);


        impl<$($l, )* P, $($g, )*> $Key<$($l, )* P, $($g, )*> where P: KeyParts, $($w: $c ),*
        {
            /// Changes the key's parts tag to `PublicParts`.
            pub fn parts_into_public(self) -> $Key<$($l, )* PublicParts, $($g, )*> {
                // Ideally, we'd use self.into() to do the actually
                // conversion.  But, because P is not concrete, we get the
                // following error:
                //
                //     error[E0277]: the trait bound `packet::Key<packet::key::PublicParts, R>: std::convert::From<packet::Key<P, R>>` is not satisfied
                //        --> openpgp/src/packet/key.rs:401:18
                //         |
                //     401 |             self.into()
                //         |                  ^^^^ the trait `std::convert::From<packet::Key<P, R>>` is not implemented for `packet::Key<packet::key::PublicParts, R>`
                //         |
                //         = help: consider adding a `where packet::Key<packet::key::PublicParts, R>: std::convert::From<packet::Key<P, R>>` bound
                //         = note: required because of the requirements on the impl of `std::convert::Into<packet::Key<packet::key::PublicParts, R>>` for `packet::Key<P, R>`
                //
                // But we can't implement implement `From<Key<P, R>>` for
                // `Key<PublicParts, R>`, because that conflicts with a
                // standard conversion!  (See the comment for the `p`
                // macro above.)
                //
                // Adding the trait bound is annoying, because then we'd
                // have to add it everywhere that we use into.
                convert!(self)
            }

            /// Changes the key's parts tag to `PublicParts`.
            pub fn parts_as_public(&$($l)* self) -> &$($l)* $Key<$($l, )* PublicParts, $($g, )*> {
                convert_ref!(self)
            }

            /// Changes the key's parts tag to `SecretParts`.
            pub fn parts_into_secret(self) -> Result<$Key<$($l, )* SecretParts, $($g, )*>> {
                if self.has_secret() {
                    Ok(convert!(self))
                } else {
                    Err(Error::InvalidArgument("No secret key".into()).into())
                }
            }

            /// Changes the key's parts tag to `SecretParts`.
            pub fn parts_as_secret(&$($l)* self) -> Result<&$($l)* $Key<$($l, )* SecretParts, $($g, )*>>
            {
                if self.has_secret() {
                    Ok(convert_ref!(self))
                } else {
                    Err(Error::InvalidArgument("No secret key".into()).into())
                }
            }

            /// Changes the key's parts tag to `UnspecifiedParts`.
            pub fn parts_into_unspecified(self) -> $Key<$($l, )* UnspecifiedParts, $($g, )*> {
                convert!(self)
            }

            /// Changes the key's parts tag to `UnspecifiedParts`.
            pub fn parts_as_unspecified(&$($l)* self) -> &$Key<$($l, )* UnspecifiedParts, $($g, )*> {
                convert_ref!(self)
            }
        }
    }
}

macro_rules! create_role_conversions {
    ( $Key:ident<$( $l:lifetime ),*> ) => {
        // Convert between two KeyRoles for a constant KeyParts.  See
        // the comment for the p macro above.
        macro_rules! r {
            ( <$from_role:ty> -> <$to_role:ty>) => {
                impl<$($l, )* P> From<$Key<$($l, )* P, $from_role>> for $Key<$($l, )* P, $to_role>
                    where P: KeyParts
                {
                    fn from(p: $Key<$($l, )* P, $from_role>) -> Self {
                        convert!(p)
                    }
                }

                impl<$($l, )* P> From<&$($l)* $Key<$($l, )* P, $from_role>> for &$($l)* $Key<$($l, )* P, $to_role>
                    where P: KeyParts
                {
                    fn from(p: &$($l)* $Key<$($l, )* P, $from_role>) -> Self {
                        convert_ref!(p)
                    }
                }
            }
        }

        r!(<PrimaryRole> -> <SubordinateRole>);
        r!(<PrimaryRole> -> <UnspecifiedRole>);

        r!(<SubordinateRole> -> <PrimaryRole>);
        r!(<SubordinateRole> -> <UnspecifiedRole>);

        r!(<UnspecifiedRole> -> <PrimaryRole>);
        r!(<UnspecifiedRole> -> <SubordinateRole>);
    }
}

macro_rules! create_conversions {
    ( $Key:ident<$( $l:lifetime ),*> ) => {
        create_part_conversions!($Key<$($l ),* ; R> where R: KeyRole);
        create_role_conversions!($Key<$($l ),* >);

        // We now handle converting both the part and the role at the same
        // time.

        macro_rules! f {
            ( <$from_parts:ty, $from_role:ty> -> <$to_parts:ty, $to_role:ty> ) => {
                impl<$($l ),*> From<$Key<$($l, )* $from_parts, $from_role>> for $Key<$($l, )* $to_parts, $to_role>
                {
                    fn from(p: $Key<$($l, )* $from_parts, $from_role>) -> Self {
                        convert!(p)
                    }
                }

                impl<$($l ),*> From<&$($l)* $Key<$($l, )* $from_parts, $from_role>> for &$($l)* $Key<$($l, )* $to_parts, $to_role>
                {
                    fn from(p: &$($l)* $Key<$from_parts, $from_role>) -> Self {
                        convert_ref!(p)
                    }
                }
            }
        }

        // The calls that are comment out are the calls for the
        // combinations where either the KeyParts or the KeyRole does not
        // change.

        //f!(<PublicParts, PrimaryRole> -> <PublicParts, PrimaryRole>);
        //f!(<PublicParts, PrimaryRole> -> <PublicParts, SubordinateRole>);
        //f!(<PublicParts, PrimaryRole> -> <PublicParts, UnspecifiedRole>);
        //f!(<PublicParts, PrimaryRole> -> <SecretParts, PrimaryRole>);
        f!(<PublicParts, PrimaryRole> -> <SecretParts, SubordinateRole>);
        f!(<PublicParts, PrimaryRole> -> <SecretParts, UnspecifiedRole>);
        //f!(<PublicParts, PrimaryRole> -> <UnspecifiedParts, PrimaryRole>);
        f!(<PublicParts, PrimaryRole> -> <UnspecifiedParts, SubordinateRole>);
        f!(<PublicParts, PrimaryRole> -> <UnspecifiedParts, UnspecifiedRole>);

        //f!(<PublicParts, SubordinateRole> -> <PublicParts, PrimaryRole>);
        //f!(<PublicParts, SubordinateRole> -> <PublicParts, SubordinateRole>);
        //f!(<PublicParts, SubordinateRole> -> <PublicParts, UnspecifiedRole>);
        f!(<PublicParts, SubordinateRole> -> <SecretParts, PrimaryRole>);
        //f!(<PublicParts, SubordinateRole> -> <SecretParts, SubordinateRole>);
        f!(<PublicParts, SubordinateRole> -> <SecretParts, UnspecifiedRole>);
        f!(<PublicParts, SubordinateRole> -> <UnspecifiedParts, PrimaryRole>);
        //f!(<PublicParts, SubordinateRole> -> <UnspecifiedParts, SubordinateRole>);
        f!(<PublicParts, SubordinateRole> -> <UnspecifiedParts, UnspecifiedRole>);

        //f!(<PublicParts, UnspecifiedRole> -> <PublicParts, PrimaryRole>);
        //f!(<PublicParts, UnspecifiedRole> -> <PublicParts, SubordinateRole>);
        //f!(<PublicParts, UnspecifiedRole> -> <PublicParts, UnspecifiedRole>);
        f!(<PublicParts, UnspecifiedRole> -> <SecretParts, PrimaryRole>);
        f!(<PublicParts, UnspecifiedRole> -> <SecretParts, SubordinateRole>);
        //f!(<PublicParts, UnspecifiedRole> -> <SecretParts, UnspecifiedRole>);
        f!(<PublicParts, UnspecifiedRole> -> <UnspecifiedParts, PrimaryRole>);
        f!(<PublicParts, UnspecifiedRole> -> <UnspecifiedParts, SubordinateRole>);
        //f!(<PublicParts, UnspecifiedRole> -> <UnspecifiedParts, UnspecifiedRole>);

        //f!(<SecretParts, PrimaryRole> -> <PublicParts, PrimaryRole>);
        f!(<SecretParts, PrimaryRole> -> <PublicParts, SubordinateRole>);
        f!(<SecretParts, PrimaryRole> -> <PublicParts, UnspecifiedRole>);
        //f!(<SecretParts, PrimaryRole> -> <SecretParts, PrimaryRole>);
        //f!(<SecretParts, PrimaryRole> -> <SecretParts, SubordinateRole>);
        //f!(<SecretParts, PrimaryRole> -> <SecretParts, UnspecifiedRole>);
        //f!(<SecretParts, PrimaryRole> -> <UnspecifiedParts, PrimaryRole>);
        f!(<SecretParts, PrimaryRole> -> <UnspecifiedParts, SubordinateRole>);
        f!(<SecretParts, PrimaryRole> -> <UnspecifiedParts, UnspecifiedRole>);

        f!(<SecretParts, SubordinateRole> -> <PublicParts, PrimaryRole>);
        //f!(<SecretParts, SubordinateRole> -> <PublicParts, SubordinateRole>);
        f!(<SecretParts, SubordinateRole> -> <PublicParts, UnspecifiedRole>);
        //f!(<SecretParts, SubordinateRole> -> <SecretParts, PrimaryRole>);
        //f!(<SecretParts, SubordinateRole> -> <SecretParts, SubordinateRole>);
        //f!(<SecretParts, SubordinateRole> -> <SecretParts, UnspecifiedRole>);
        f!(<SecretParts, SubordinateRole> -> <UnspecifiedParts, PrimaryRole>);
        //f!(<SecretParts, SubordinateRole> -> <UnspecifiedParts, SubordinateRole>);
        f!(<SecretParts, SubordinateRole> -> <UnspecifiedParts, UnspecifiedRole>);

        f!(<SecretParts, UnspecifiedRole> -> <PublicParts, PrimaryRole>);
        f!(<SecretParts, UnspecifiedRole> -> <PublicParts, SubordinateRole>);
        //f!(<SecretParts, UnspecifiedRole> -> <PublicParts, UnspecifiedRole>);
        //f!(<SecretParts, UnspecifiedRole> -> <SecretParts, PrimaryRole>);
        //f!(<SecretParts, UnspecifiedRole> -> <SecretParts, SubordinateRole>);
        //f!(<SecretParts, UnspecifiedRole> -> <SecretParts, UnspecifiedRole>);
        f!(<SecretParts, UnspecifiedRole> -> <UnspecifiedParts, PrimaryRole>);
        f!(<SecretParts, UnspecifiedRole> -> <UnspecifiedParts, SubordinateRole>);
        //f!(<SecretParts, UnspecifiedRole> -> <UnspecifiedParts, UnspecifiedRole>);

        //f!(<UnspecifiedParts, PrimaryRole> -> <PublicParts, PrimaryRole>);
        f!(<UnspecifiedParts, PrimaryRole> -> <PublicParts, SubordinateRole>);
        f!(<UnspecifiedParts, PrimaryRole> -> <PublicParts, UnspecifiedRole>);
        //f!(<UnspecifiedParts, PrimaryRole> -> <SecretParts, PrimaryRole>);
        f!(<UnspecifiedParts, PrimaryRole> -> <SecretParts, SubordinateRole>);
        f!(<UnspecifiedParts, PrimaryRole> -> <SecretParts, UnspecifiedRole>);
        //f!(<UnspecifiedParts, PrimaryRole> -> <UnspecifiedParts, PrimaryRole>);
        //f!(<UnspecifiedParts, PrimaryRole> -> <UnspecifiedParts, SubordinateRole>);
        //f!(<UnspecifiedParts, PrimaryRole> -> <UnspecifiedParts, UnspecifiedRole>);

        f!(<UnspecifiedParts, SubordinateRole> -> <PublicParts, PrimaryRole>);
        //f!(<UnspecifiedParts, SubordinateRole> -> <PublicParts, SubordinateRole>);
        f!(<UnspecifiedParts, SubordinateRole> -> <PublicParts, UnspecifiedRole>);
        f!(<UnspecifiedParts, SubordinateRole> -> <SecretParts, PrimaryRole>);
        //f!(<UnspecifiedParts, SubordinateRole> -> <SecretParts, SubordinateRole>);
        f!(<UnspecifiedParts, SubordinateRole> -> <SecretParts, UnspecifiedRole>);
        //f!(<UnspecifiedParts, SubordinateRole> -> <UnspecifiedParts, PrimaryRole>);
        //f!(<UnspecifiedParts, SubordinateRole> -> <UnspecifiedParts, SubordinateRole>);
        //f!(<UnspecifiedParts, SubordinateRole> -> <UnspecifiedParts, UnspecifiedRole>);

        f!(<UnspecifiedParts, UnspecifiedRole> -> <PublicParts, PrimaryRole>);
        f!(<UnspecifiedParts, UnspecifiedRole> -> <PublicParts, SubordinateRole>);
        //f!(<UnspecifiedParts, UnspecifiedRole> -> <PublicParts, UnspecifiedRole>);
        f!(<UnspecifiedParts, UnspecifiedRole> -> <SecretParts, PrimaryRole>);
        f!(<UnspecifiedParts, UnspecifiedRole> -> <SecretParts, SubordinateRole>);
        //f!(<UnspecifiedParts, UnspecifiedRole> -> <SecretParts, UnspecifiedRole>);
        //f!(<UnspecifiedParts, UnspecifiedRole> -> <UnspecifiedParts, PrimaryRole>);
        //f!(<UnspecifiedParts, UnspecifiedRole> -> <UnspecifiedParts, SubordinateRole>);
        //f!(<UnspecifiedParts, UnspecifiedRole> -> <UnspecifiedParts, UnspecifiedRole>);


        impl<$($l, )* P, R> $Key<$($l, )* P, R> where P: KeyParts, R: KeyRole
        {
            /// Changes the key's role tag to `PrimaryRole`.
            pub fn role_into_primary(self) -> $Key<$($l, )* P, PrimaryRole> {
                convert!(self)
            }

            /// Changes the key's role tag to `PrimaryRole`.
            pub fn role_as_primary(&$($l)* self) -> &$($l)* $Key<$($l, )* P, PrimaryRole> {
                convert_ref!(self)
            }

            /// Changes the key's role tag to `SubordinateRole`.
            pub fn role_into_subordinate(self) -> $Key<$($l, )* P, SubordinateRole>
            {
                convert!(self)
            }

            /// Changes the key's role tag to `SubordinateRole`.
            pub fn role_as_subordinate(&$($l)* self) -> &$($l)* $Key<$($l, )* P, SubordinateRole>
            {
                convert_ref!(self)
            }

            /// Changes the key's role tag to `UnspecifiedRole`.
            pub fn role_into_unspecified(self) -> $Key<$($l, )* P, UnspecifiedRole>
            {
                convert!(self)
            }

            /// Changes the key's role tag to `UnspecifiedRole`.
            pub fn role_as_unspecified(&$($l)* self) -> &$($l)* $Key<$($l, )* P, UnspecifiedRole>
            {
                convert_ref!(self)
            }
        }
    }
}

create_conversions!(Key<>);
create_conversions!(Key4<>);
create_conversions!(KeyBundle<>);

// A hack, since the type has to be an ident, which means that we
// can't use <>.
type KeyComponentAmalgamation<'a, P, R> = ComponentAmalgamation<'a, Key<P, R>>;
create_conversions!(KeyComponentAmalgamation<'a>);

create_part_conversions!(PrimaryKeyAmalgamation<'a;>);
create_part_conversions!(SubordinateKeyAmalgamation<'a;>);
create_part_conversions!(ErasedKeyAmalgamation<'a;>);
create_part_conversions!(ValidPrimaryKeyAmalgamation<'a;>);
create_part_conversions!(ValidSubordinateKeyAmalgamation<'a;>);
create_part_conversions!(ValidErasedKeyAmalgamation<'a;>);
