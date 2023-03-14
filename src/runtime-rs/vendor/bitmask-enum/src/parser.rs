
use syn::{Ident, ItemEnum};
use proc_macro::{TokenStream, Span};

pub fn parse(attr: TokenStream, item: ItemEnum) -> TokenStream {
    let typ = parse_typ(attr);

    let vis = item.vis;
    let attr = item.attrs;
    let ident = item.ident;

    let mut has_vals: Option<bool> = None;

    let attrs = item.variants.iter().map(|v| &v.attrs);
    let idents = item.variants.iter().map(|v| &v.ident);
    let exprs = item.variants.iter().enumerate().map(|(i, v)| {
        let hv = has_vals.unwrap_or_else(|| {
            let hv = v.discriminant.is_some();
            has_vals = Some(hv);
            hv
        });

        if hv != v.discriminant.is_some() {
            panic!("the bitmask can either have assigned or default values, not both.")
        }

        if hv {
            let (_, ref expr) = v.discriminant.as_ref().expect("unreachable");
            quote::quote!(Self { bits: #expr })
        } else {
            quote::quote!(Self { bits: 1 << #i })
        }
    });

    TokenStream::from(quote::quote! {
        #(#attr)*
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #vis struct #ident {
            bits: #typ,
        }

        #[allow(non_upper_case_globals)]
        impl #ident {
            #(
                #(#attrs)*
                #vis const #idents: #ident = #exprs;
            )*

            /// returns the underlying bits
            #[inline]
            #vis const fn bits(&self) -> #typ {
                self.bits
            }

            /// contains all values
            #[inline]
            #vis const fn all() -> Self {
                Self { bits: !0 }
            }

            /// if self contains all values
            #[inline]
            #vis const fn is_all(&self) -> bool {
                self.bits == !0
            }

            /// contains no value
            #[inline]
            #vis const fn none() -> Self {
                Self { bits: 0 }
            }

            /// if self contains no value
            #[inline]
            #vis const fn is_none(&self) -> bool {
                self.bits == 0
            }

            /// self intersects one of the other
            /// `(self & other) != 0 || other == 0`
            #[inline]
            #vis const fn intersects(&self, other: Self) -> bool {
                (self.bits & other.bits) != 0 || other.bits == 0
            }

            /// self contains all of the other
            /// `(self & other) == other`
            #[inline]
            #vis const fn contains(&self, other: Self) -> bool {
                (self.bits & other.bits) == other.bits
            }

            /// constant bitwise not
            #[inline]
            #vis const fn not(self) -> Self {
                Self { bits: !self.bits }
            }

            /// constant bitwise and
            #[inline]
            #vis const fn and(self, other: Self) -> Self {
                Self { bits: self.bits & other.bits }
            }

            /// constant bitwise or
            #[inline]
            #vis const fn or(self, other: Self) -> Self {
                Self { bits: self.bits | other.bits }
            }

            /// constant bitwise xor
            #[inline]
            #vis const fn xor(self, other: Self) -> Self {
                Self { bits: self.bits ^ other.bits }
            }
        }

        impl core::ops::Not for #ident {
            type Output = Self;
            #[inline]
            fn not(self) -> Self::Output {
                Self { bits: self.bits.not() }
            }
        }

        impl core::ops::BitAnd for #ident {
            type Output = Self;
            #[inline]
            fn bitand(self, rhs: Self) -> Self::Output {
                Self { bits: self.bits.bitand(rhs.bits) }
            }
        }

        impl core::ops::BitAndAssign for #ident {
            #[inline]
            fn bitand_assign(&mut self, rhs: Self){
                self.bits.bitand_assign(rhs.bits)
            }
        }

        impl core::ops::BitOr for #ident {
            type Output = Self;
            #[inline]
            fn bitor(self, rhs: Self) -> Self::Output {
                Self { bits: self.bits.bitor(rhs.bits) }
            }
        }

        impl core::ops::BitOrAssign for #ident {
            #[inline]
            fn bitor_assign(&mut self, rhs: Self){
                self.bits.bitor_assign(rhs.bits)
            }
        }

        impl core::ops::BitXor for #ident {
            type Output = Self;
            #[inline]
            fn bitxor(self, rhs: Self) -> Self::Output {
                Self { bits: self.bits.bitxor(rhs.bits) }
            }
        }

        impl core::ops::BitXorAssign for #ident {
            #[inline]
            fn bitxor_assign(&mut self, rhs: Self){
                self.bits.bitxor_assign(rhs.bits)
            }
        }

        impl From<#typ> for #ident {
            #[inline]
            fn from(val: #typ) -> Self {
                Self { bits: val }
            }
        }

        impl From<#ident> for #typ {
            #[inline]
            fn from(val: #ident) -> #typ {
                val.bits
            }
        }

        impl PartialEq<#typ> for #ident {
            #[inline]
            fn eq(&self, other: &#typ) -> bool {
                self.bits == *other
            }
        }

        impl core::fmt::Binary for #ident {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.bits.fmt(f)
            }
        }

        impl core::fmt::LowerHex for #ident {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.bits.fmt(f)
            }
        }

        impl core::fmt::UpperHex for #ident {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.bits.fmt(f)
            }
        }

        impl core::fmt::Octal for #ident {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.bits.fmt(f)
            }
        }
    })
}

fn parse_typ(attr: TokenStream) -> Ident {
    if attr.is_empty() {
        Ident::new("usize", Span::call_site().into())
    } else {
        match syn::parse::<Ident>(attr) {
            Ok(ident) => {
                match ident.to_string().as_str() {
                    "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => (),
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => (),
                    _ => panic!("type can only be an (un)signed integer."),
                }
                ident
            }
            Err(_) => panic!("type can only be an (un)signed integer."),
        }
    }
}
