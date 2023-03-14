extern crate rustc_serialize;
extern crate std;

use self::std::prelude::v1::*;
use self::rustc_serialize::{Encoder, Encodable, Decoder, Decodable};

use Uuid;

impl Encodable for Uuid {
    fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        e.emit_str(&self.hyphenated().to_string())
    }
}

impl Decodable for Uuid {
    fn decode<D: Decoder>(d: &mut D) -> Result<Uuid, D::Error> {
        let string = try!(d.read_str());
        string.parse::<Uuid>().map_err(|err| d.error(&err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::rustc_serialize::json;
    use Uuid;

    #[test]
    fn test_serialize_round_trip() {
        let u = Uuid::parse_str("F9168C5E-CEB2-4FAA-B6BF-329BF39FA1E4").unwrap();
        let s = json::encode(&u).unwrap();
        let u2 = json::decode(&s).unwrap();
        assert_eq!(u, u2);
    }
}
