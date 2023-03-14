extern crate serde;
extern crate std;

use self::std::fmt;
use self::std::prelude::v1::*;
use self::serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use Uuid;

impl Serialize for Uuid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hyphenated().to_string())
    }
}

impl Deserialize for Uuid {
    fn deserialize<D: Deserializer>(deserializer: D) -> Result<Self, D::Error> {
        struct UuidVisitor;

        impl de::Visitor for UuidVisitor {
            type Value = Uuid;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a UUID string")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Uuid, E> {
                value.parse::<Uuid>().map_err(|e| E::custom(e.to_string()))
            }
        }

        deserializer.deserialize_str(UuidVisitor)
    }
}
