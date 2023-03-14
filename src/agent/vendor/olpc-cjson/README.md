`olpc-cjson` provides a [`serde_json::Formatter`] to serialize data as [canonical JSON], as defined by OLPC and used in [TUF]. It is developed as part of [tough], a Rust library for using TUF repositories.

[`serde_json::Formatter`]: ../serde_json/ser/trait.Formatter.html
[canonical JSON]: http://wiki.laptop.org/go/Canonical_JSON
[TUF]: https://theupdateframework.github.io/
[tough]: https://github.com/awslabs/tough

OLPC's canonical JSON specification is subtly different from other "canonical JSON" specifications, and is also not a strict subset of JSON (specifically, ASCII control characters 0x00&ndash;0x1f are printed literally, which is not valid JSON). Therefore, `serde_json` cannot necessarily deserialize JSON produced by this formatter.

This crate is not developed or endorsed by OLPC; use of the term is solely to distinguish this specification of canonical JSON from [other specifications of canonical JSON][xkcd].

[xkcd]: https://xkcd.com/927/

```rust
use olpc_cjson::CanonicalFormatter;
use serde::Serialize;
use serde_json::json;

let value = json!({"b": 12, "a": "qwerty"});
let mut buf = Vec::new();
let mut ser = serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
value.serialize(&mut ser).unwrap();
assert_eq!(buf, br#"{"a":"qwerty","b":12}"#);
```
