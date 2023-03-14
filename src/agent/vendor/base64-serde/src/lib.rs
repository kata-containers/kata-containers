extern crate base64;
extern crate serde;

#[doc(hidden)]
pub use base64::{decode_config, encode_config};
#[doc(hidden)]
pub use serde::{de, Deserializer, Serializer};

/// Create a type with appropriate `serialize` and `deserialize` functions to use with
/// serde when specifying how to serialize a particular field.
///
/// If you wanted to use the `URL_SAFE_NO_PAD` configuration, for instance, then you might have
/// `base64_serde_type!(Base64UrlSafeNoPad, URL_SAFE_NO_PAD)` in your code to declare the type, and
/// then use `#[serde(with = "Base64UrlSafeNoPad")]` on a `Vec<u8>` field that you wished to
/// serialize to base64 or deserialize from base64.
///
/// If you want the resulting type to be public, prefix the desired type name with `pub`, as in:
///
/// `base64_serde_type!(pub IWillBeAPublicType, URL_SAFE_NO_PAD)`
#[macro_export]
macro_rules! base64_serde_type {
    ($typename:ident, $config:expr) => {
        enum $typename {}
        base64_serde_type!(impl_only, $typename, $config);
    };
    (pub $typename:ident, $config:expr) => {
        pub enum $typename {}
        base64_serde_type!(impl_only, $typename, $config);
    };
    (impl_only, $typename:ident, $config:expr) => {
        impl $typename {
            pub fn serialize<S, Input>(
                bytes: Input,
                serializer: S,
            ) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: $crate::Serializer,
                Input: AsRef<[u8]>,
            {
                serializer.serialize_str(&$crate::encode_config(bytes.as_ref(), $config))
            }

            pub fn deserialize<'de, D, Output>(
                deserializer: D,
            ) -> ::std::result::Result<Output, D::Error>
            where
                D: $crate::Deserializer<'de>,
                Output: From<Vec<u8>>,
            {
                struct Base64Visitor;

                impl<'de> $crate::de::Visitor<'de> for Base64Visitor {
                    type Value = Vec<u8>;

                    fn expecting(
                        &self,
                        formatter: &mut ::std::fmt::Formatter,
                    ) -> ::std::fmt::Result {
                        write!(formatter, "base64 ASCII text")
                    }

                    fn visit_str<E>(self, v: &str) -> ::std::result::Result<Self::Value, E>
                    where
                        E: $crate::de::Error,
                    {
                        $crate::decode_config(v, $config).map_err($crate::de::Error::custom)
                    }
                }

                deserializer
                    .deserialize_str(Base64Visitor)
                    .map(|vec| Output::from(vec))
            }
        }
    };
}
