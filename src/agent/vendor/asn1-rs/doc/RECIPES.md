# Documentation: BER/DER parsing recipes

## Builtin types

Most builtin types can be parsed by calling the `from_der` or `from_der` functions (see `FromBer` and `FromDer` traits for documentation).

For ex:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = <u32>::from_der(input)?;
# Ok(()) };
```

Note: this crates makes extensive use of types annotation and turbofish operator, for example `<Type>::from_der()` or `TaggedExplicit::<u32, Error, 0>::from_der()`.

See table B-3 in <https://doc.rust-lang.org/book/appendix-02-operators.html> for reference on syntax.

## `SEQUENCE` and `SET`

The `SEQUENCE` and `SET` types are handled very similarly, so recipes will be given for `SEQUENCE`, but can be adapted to `SET` by replacing words.

### Parsing `SEQUENCE`

Usually, the sequence envelope does not need to be stored, so it just needs to be parsed to get the sequence content and parse it.
The methods [`from_ber_and_then`](crate::Sequence::from_ber_and_then()) and [`from_der_and_then`](crate::Sequence::from_der_and_then()) provide helpers for that:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = Sequence::from_ber_and_then(input, |i| {
    // first item is INTEGER
    let (rem, a) = u32::from_der(input)?;
    // second item is OCTET STRING
    let (rem, b) = <&[u8]>::from_der(input)?;
    Ok((rem, (a, b)))
})?;
// result has type (u32, &[u8])
assert_eq!(result.0, 0);
assert_eq!(result.1, b"\x00\x01");
# Ok(()) };
```

### Automatically deriving sequence parsers

The [`BerSequence`](crate::BerSequence) and [`DerSequence`](crate::DerSequence)
custom derive provide attributes to automatically derive a parser for a sequence.

For ex:

```rust
# use asn1_rs::*;
#[derive(DerSequence)]
pub struct S {
    a: u32,
    b: u16,
    c: u16,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_der(input)?;
# Ok(()) };
```

This will work for any field type that implements [`FromBer`](crate::FromBer) or [`FromDer`](crate::FromDer), respectively.

See [`derive`](mod@derive) documentation for more examples and documentation.

### Parsing `SEQUENCE OF`

`SEQUENCE OF T` can be parsed using either type `SequenceOf<T>` or `Vec<T>`:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = SequenceOf::<u32>::from_der(input)?;
# Ok(()) };
```

or

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = <Vec<u32>>::from_der(input)?;
# Ok(()) };
```

`SET OF T` can be parsed using either `SetOf<T>`, `BTreeSet<T>` or `HashSet<T>`.

## `EXPLICIT` tagged values

### Parsing `EXPLICIT`, expecting a known tag

If you expect only a specific tag, use `TaggedExplicit`.

For ex, to parse a `[3] EXPLICIT INTEGER`:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = TaggedExplicit::<u32, Error, 0>::from_der(input)?;
// result has type TaggedValue. Use `.as_ref()` or `.into_inner()` 
// to access content
let tag = result.tag();
let class = result.class();
assert_eq!(result.as_ref(), &0);
# Ok(()) };
```

### Specifying the class

`TaggedExplicit` does not check the class, and accepts any class. It expects you to check the class after reading the value.


To specify the class in the parser, use `TaggedValue`:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
// Note: the strange notation (using braces) is required by the compiler to use
// a constant instead of the numeric value.
let (rem, result) = TaggedValue::<u32, Error, Explicit, {Class::CONTEXT_SPECIFIC}, 0>::from_der(input)?;
# Ok(()) };
```

Note that `TaggedExplicit` is a type alias to `TaggedValue`, so the objects are the same.

### Accepting any `EXPLICIT` tag

To parse a value, accepting any class or tag, use `TaggedParser`.

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = TaggedParser::<Explicit, u32>::from_der(input)?;
// result has type TaggedParser. Use `.as_ref()` or `.into_inner()` 
// to access content
let tag = result.tag();
let class = result.class();
assert_eq!(result.as_ref(), &0);
# Ok(()) };
```

### Optional tagged values

To parse optional tagged values, `Option<TaggedExplicit<...>>` can be used:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = Option::<TaggedExplicit::<u32, Error, 0>>::from_der(input)?;
# Ok(()) };
```

The type `OptTaggedExplicit` is also provided as an alias:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = OptTaggedExplicit::<u32, Error, 0>::from_der(input)?;
# Ok(()) };
```

## `IMPLICIT` tagged values

### Parsing `IMPLICIT`, expecting a known tag

If you expect only a specific tag, use `TaggedImplicit`.

For ex, to parse a `[3] EXPLICIT INTEGER`:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = TaggedExplicit::<u32, Error, 0>::from_der(input)?;
// result has type TaggedValue. Use `.as_ref()` or `.into_inner()` 
// to access content
let tag = result.tag();
let class = result.class();
assert_eq!(result.as_ref(), &0);
# Ok(()) };
```

### Specifying the class

`TaggedImplicit` does not check the class, and accepts any class. It expects you to check the class after reading the value.


To specify the class in the parser, use `TaggedValue`:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
// Note: the strange notation (using braces) is required by the compiler to use
// a constant instead of the numeric value.
let (rem, result) = TaggedValue::<u32, Error, Implicit, { Class::CONTEXT_SPECIFIC }, 1>::from_der(input)?;
# Ok(()) };
```

Note that `TaggedImplicit` is a type alias to `TaggedValue`, so the objects are the same.

### Accepting any `IMPLICIT` tag

To parse a value, accepting any class or tag, use `TaggedParser`.

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = TaggedParser::<Implicit, u32>::from_der(input)?;
// result has type TaggedParser. Use `.as_ref()` or `.into_inner()` 
// to access content
let tag = result.tag();
let class = result.class();
assert_eq!(result.as_ref(), &0);
# Ok(()) };
```

### Optional tagged values

To parse optional tagged values, `Option<TaggedImplicit<...>>` can be used:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = Option::<TaggedImplicit::<u32, Error, 0>>::from_der(input)?;
# Ok(()) };
```

The type `OptTaggedImplicit` is also provided as an alias:

```rust
# use asn1_rs::*;
# let parser = |input| -> Result<(), Error> {
let (rem, result) = OptTaggedImplicit::<u32, Error, 0>::from_der(input)?;
# Ok(()) };
```
