use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};

use super::CapSet;

impl Serialize for CapSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.size()))?;
        for element in self.iter() {
            seq.serialize_element(&element)?;
        }
        seq.end()
    }
}

struct CapSetVisitor;

impl<'de> Visitor<'de> for CapSetVisitor {
    type Value = CapSet;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(formatter, "a set of Linux capability names")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut set = CapSet::empty();

        while let Some(cap) = seq.next_element()? {
            set.add(cap);
        }

        Ok(set)
    }
}

impl<'de> Deserialize<'de> for CapSet {
    fn deserialize<S>(deserializer: S) -> Result<Self, S::Error>
    where
        S: Deserializer<'de>,
    {
        deserializer.deserialize_seq(CapSetVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::Cap;

    use serde_test::{assert_de_tokens_error, assert_tokens, Token};

    #[test]
    fn test_ser_de_capset() {
        assert_tokens(
            &CapSet::empty(),
            &[Token::Seq { len: Some(0) }, Token::SeqEnd],
        );

        assert_tokens(
            &crate::capset!(Cap::CHOWN),
            &[
                Token::Seq { len: Some(1) },
                Token::UnitVariant {
                    name: "Cap",
                    variant: "CHOWN",
                },
                Token::SeqEnd,
            ],
        );

        assert_tokens(
            &crate::capset!(Cap::CHOWN, Cap::FOWNER),
            &[
                Token::Seq { len: Some(2) },
                Token::UnitVariant {
                    name: "Cap",
                    variant: "CHOWN",
                },
                Token::UnitVariant {
                    name: "Cap",
                    variant: "FOWNER",
                },
                Token::SeqEnd,
            ],
        );

        assert_de_tokens_error::<CapSet>(
            &[Token::String("CHOWN")],
            "invalid type: string \"CHOWN\", expected a set of Linux capability names",
        );
    }
}
