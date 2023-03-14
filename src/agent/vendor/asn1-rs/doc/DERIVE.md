# BER/DER Custom Derive Attributes

## BER/DER Sequence parsers

### `BER`

To derive a BER `SEQUENCE` parser, add the [`BerSequence`] derive attribute to an existing struct. Parsers will be derived automatically for all fields, which must implement the [`FromBer`] trait.

For ex:

```rust
# use asn1_rs::*;
#[derive(Debug, PartialEq, BerSequence)]
pub struct S {
    a: u32,
    b: u16,
    c: u16,
}

# let parser = |input| -> Result<(), Error> {
let (rest, result) = S::from_ber(input)?;
# Ok(()) };
```

After parsing b, any bytes that were leftover and not used to fill val will be returned in `rest`.

When parsing a `SEQUENCE` into a struct, any trailing elements of the `SEQUENCE` that do
not have matching fields in val will not be included in `rest`, as these are considered
valid elements of the `SEQUENCE` and not trailing data.

### `DER`

To derive a `DER` parser, use the [`DerSequence`] custom attribute.

*Note: the `DerSequence` attributes derive both `BER` and `DER` parsers.*

## Tagged values

### `EXPLICIT`

There are several ways of parsing tagged values: either using types like [`TaggedExplicit`], or using custom annotations.

Using `TaggedExplicit` works as usual. The only drawback is that the type is visible in the structure, so accessing the value must be done using `.as_ref()` or `.into_inner()`:

```rust
# use asn1_rs::*;
#[derive(Debug, PartialEq, DerSequence)]
pub struct S2 {
    a: u16,
}

// test with EXPLICIT Vec
#[derive(Debug, PartialEq, DerSequence)]
pub struct S {
    // a INTEGER
    a: u32,
    // b INTEGER
    b: u16,
    // c [0] EXPLICIT SEQUENCE OF S2
    c: TaggedExplicit<Vec<S2>, Error, 0>,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_ber(input)?;

// Get a reference on c (type is &Vec<S2>)
let ref_c = result.c.as_ref();
# Ok(()) };
```

*Note: tags are context-specific by default. To specify other kind of tags (like `APPLICATION`) use [`TaggedValue`].*

### `tag_explicit`

To "hide" the tag from the parser, the `tag_explicit` attribute is provided. This attribute must specify the tag value (as an integer), and will automatically wrap reading the value with the specified tag.

```rust
# use asn1_rs::*;
#[derive(Debug, PartialEq, DerSequence)]
pub struct S {
    // a [0] EXPLICIT INTEGER
    #[tag_explicit(0)]
    a: u16,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_ber(input)?;
# Ok(()) };
```

This method handles transparently the encapsulation and the read of the tagged value.

*Note: tags are context-specific by default. To specify other kind of tags (like `APPLICATION`) add the tag class before the value in the `tag_explicit` attribute.*
For ex: `tag_explicit(APPLICATION 0)` or `tag_explicit(PRIVATE 2)`.

### Tagged optional values

The `optional` custom attribute can be used in addition of `tag_explicit` to specify that the value is `OPTIONAL`.

The type of the annotated field member must be resolvable to `Option`.

```rust
# use asn1_rs::*;
#[derive(Debug, PartialEq, DerSequence)]
pub struct S {
    // a [0] EXPLICIT INTEGER OPTIONAL
    #[tag_explicit(0)]
    #[optional]
    a: Option<u16>,
    // b INTEGER
    b: u16,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_ber(input)?;
# Ok(()) };
```

### `IMPLICIT`

Tagged `IMPLICIT` values are handled similarly as for `EXPLICIT`, and can be parsed either using the [`TaggedImplicit`] type, or using the `tag_implicit` custom attribute.

For ex:
```rust
# use asn1_rs::*;
#[derive(Debug, PartialEq, DerSequence)]
pub struct S {
    // a [0] IMPLICIT INTEGER OPTIONAL
    #[tag_implicit(0)]
    #[optional]
    a: Option<u16>,
    // b INTEGER
    b: u16,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_ber(input)?;
# Ok(()) };
```

## `OPTIONAL` values (not tagged)

The `optional` custom attribute can be specified to indicate the value is `OPTIONAL`.

```rust
# use asn1_rs::*;
#[derive(Debug, PartialEq, DerSequence)]
pub struct S {
    // a INTEGER
    a: u16,
    // b INTEGER OPTIONAL
    #[optional]
    b: Option<u16>,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_ber(input)?;
# Ok(()) };
```

**Important**: there are several limitations to this attribute.

In particular, the parser is eager: when an `OPTIONAL` value of some type is followed by another value (not `OPTIONAL`) of the same type, this can create problem.
If only one value is present, the parser will affect it to the first field, and then raise an error because the second is absent.

Note that this does not concern tagged optional values (unless they have the same tag).

## `DEFAULT`

The `default` custom attribute can be specified to indicate the value has a `DEFAULT` attribute. The value can also be marked as
`OPTIONAL`, but this can be omitted.

Since the value can always be obtained, the type should not be `Option<T>`, but should use `T` directly.

```rust
# use asn1_rs::*;
#[derive(Debug, PartialEq, DerSequence)]
#[debug_derive]
pub struct S {
    // a INTEGER
    a: u16,
    // b INTEGER DEFAULT 0
    #[default(0_u16)]
    b: u16,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_ber(input)?;
# Ok(()) };
```

Limitations are the same as for `OPTIONAL` attribute.

## Debugging

To help debugging the generated code, the `#[debug_derive]` attribute has been added.

When this attribute is specified, the generated code will be printed to `stderr` during compilation.

Example:
```rust
use asn1_rs::*;

#[derive(BerSequence)]
#[debug_derive]
struct S {
  a: u32,
}
```

## BER/DER Set parsers

Parsing BER/DER `SET` objects is very similar to `SEQUENCE`. Use the [`BerSet`] and [`DerSet`] custom derive attributes on the structure, and everything else is exactly the same as for sequences (see above for documentation).

Example:
```rust
# use asn1_rs::*;
use std::collections::BTreeSet;

// `Ord` is needed because we will parse as a `BTreeSet` later
#[derive(Debug, DerSet, PartialEq, Eq, PartialOrd, Ord)]
pub struct S2 {
    a: u16,
}

// test with EXPLICIT Vec
#[derive(Debug, PartialEq, DerSet)]
pub struct S {
    // a INTEGER
    a: u32,
    // b INTEGER
    b: u16,
    // c [0] EXPLICIT SET OF S2
    c: TaggedExplicit<BTreeSet<S2>, Error, 0>,
}

# let parser = |input| -> Result<(), Error> {
let (rem, result) = S::from_ber(input)?;

// Get a reference on c (type is &BTreeSet<S2>)
let ref_c = result.c.as_ref();
# Ok(()) };
```

# Advanced

## Custom errors

Derived parsers can use the `error` attribute to specify the error type of the parser.

The custom error type must implement `From<Error>`, so the derived parsers will transparently convert errors using the [`Into`] trait.


Example:
```rust
# use asn1_rs::*;
#
#[derive(Debug, PartialEq)]
pub enum MyError {
    NotYetImplemented,
}

impl From<asn1_rs::Error> for MyError {
    fn from(_: asn1_rs::Error) -> Self {
        MyError::NotYetImplemented
    }
}

#[derive(DerSequence)]
#[error(MyError)]
pub struct T2 {
    pub a: u32,
}
```

## Mapping errors

Sometimes, it is necessary to map the returned error to another type, for example when a subparser returns a different error type than the parser's, and the [`Into`] trait cannot be implemented. This is often used in combination with the `error` attribute, but can also be used alone.

The `map_err` attribute can be used to specify a function or closure to map errors. The function signature is `fn (e1: E1) -> E2`.

Example:
```rust
# use asn1_rs::*;
#
#[derive(Debug, PartialEq)]
pub enum MyError {
    NotYetImplemented,
}

impl From<asn1_rs::Error> for MyError {
    fn from(_: asn1_rs::Error) -> Self {
        MyError::NotYetImplemented
    }
}

#[derive(DerSequence)]
#[error(MyError)]
pub struct T2 {
    pub a: u32,
}

// subparser returns an error of type MyError,
// which is mapped to `Error`
#[derive(DerSequence)]
pub struct T4 {
    #[map_err(|_| Error::BerTypeError)]
    pub a: T2,
}
```

*Note*: when deriving BER and DER parsers, errors paths are different (`TryFrom` returns the error type, while [`FromDer`] returns a [`ParseResult`]). Some code will be inserted by the `map_err` attribute to handle this transparently and keep the same function signature.

[`FromBer`]: crate::FromBer
[`FromDer`]: crate::FromDer
[`BerSequence`]: crate::BerSequence
[`DerSequence`]: crate::DerSequence
[`BerSet`]: crate::BerSet
[`DerSet`]: crate::DerSet
[`ParseResult`]: crate::ParseResult
[`TaggedExplicit`]: crate::TaggedExplicit
[`TaggedImplicit`]: crate::TaggedImplicit
[`TaggedValue`]: crate::TaggedValue
