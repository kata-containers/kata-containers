/// # BerSequence custom derive
///
/// `BerSequence` is a custom derive attribute, to derive a BER [`Sequence`](super::Sequence) parser automatically from the structure definition.
/// This attribute will automatically derive implementations for the following traits:
///   - [`TryFrom<Any>`](super::Any), also providing [`FromBer`](super::FromBer)
///   - [`Tagged`](super::Tagged)
///
/// `DerSequence` implies `BerSequence`, and will conflict with this custom derive. Use `BerSequence` when you only want the
/// above traits derived.
///
/// Parsers will be automatically derived from struct fields. Every field type must implement the [`FromBer`](super::FromBer) trait.
///
/// See [`derive`](crate::doc::derive) documentation for more examples and documentation.
///
/// ## Examples
///
/// To parse the following ASN.1 structure:
/// <pre>
/// S ::= SEQUENCE {
///     a INTEGER(0..2^32),
///     b INTEGER(0..2^16),
///     c INTEGER(0..2^16),
/// }
/// </pre>
///
/// Define a structure and add the `BerSequence` derive:
///
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(BerSequence)]
/// struct S {
///   a: u32,
///   b: u16,
///   c: u16
/// }
/// ```
///
/// ## Debugging
///
/// To help debugging the generated code, the `#[debug_derive]` attribute has been added.
///
/// When this attribute is specified, the generated code will be printed to `stderr` during compilation.
///
/// Example:
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(BerSequence)]
/// #[debug_derive]
/// struct S {
///   a: u32,
/// }
/// ```
pub use asn1_rs_derive::BerSequence;

/// # DerSequence custom derive
///
/// `DerSequence` is a custom derive attribute, to derive both BER and DER [`Sequence`](super::Sequence) parsers automatically from the structure definition.
/// This attribute will automatically derive implementations for the following traits:
///   - [`TryFrom<Any>`](super::Any), also providing [`FromBer`](super::FromBer)
///   - [`Tagged`](super::Tagged)
///   - [`CheckDerConstraints`](super::CheckDerConstraints)
///   - [`FromDer`](super::FromDer)
///
/// `DerSequence` implies `BerSequence`, and will conflict with this custom derive.
///
/// Parsers will be automatically derived from struct fields. Every field type must implement the [`FromDer`](super::FromDer) trait.
///
/// See [`derive`](crate::doc::derive) documentation for more examples and documentation.
///
/// ## Examples
///
/// To parse the following ASN.1 structure:
/// <pre>
/// S ::= SEQUENCE {
///     a INTEGER(0..2^32),
///     b INTEGER(0..2^16),
///     c INTEGER(0..2^16),
/// }
/// </pre>
///
/// Define a structure and add the `DerSequence` derive:
///
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(DerSequence)]
/// struct S {
///   a: u32,
///   b: u16,
///   c: u16
/// }
/// ```
///
/// ## Debugging
///
/// To help debugging the generated code, the `#[debug_derive]` attribute has been added.
///
/// When this attribute is specified, the generated code will be printed to `stderr` during compilation.
///
/// Example:
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(DerSequence)]
/// #[debug_derive]
/// struct S {
///   a: u32,
/// }
/// ```
pub use asn1_rs_derive::DerSequence;

/// # BerSet custom derive
///
/// `BerSet` is a custom derive attribute, to derive a BER [`Set`](super::Set) parser automatically from the structure definition.
/// This attribute will automatically derive implementations for the following traits:
///   - [`TryFrom<Any>`](super::Any), also providing [`FromBer`](super::FromBer)
///   - [`Tagged`](super::Tagged)
///
/// `DerSet` implies `BerSet`, and will conflict with this custom derive. Use `BerSet` when you only want the
/// above traits derived.
///
/// Parsers will be automatically derived from struct fields. Every field type must implement the [`FromBer`](super::FromBer) trait.
///
/// See [`derive`](crate::doc::derive) documentation for more examples and documentation.
///
/// ## Examples
///
/// To parse the following ASN.1 structure:
/// <pre>
/// S ::= SET {
///     a INTEGER(0..2^32),
///     b INTEGER(0..2^16),
///     c INTEGER(0..2^16),
/// }
/// </pre>
///
/// Define a structure and add the `BerSet` derive:
///
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(BerSet)]
/// struct S {
///   a: u32,
///   b: u16,
///   c: u16
/// }
/// ```
///
/// ## Debugging
///
/// To help debugging the generated code, the `#[debug_derive]` attribute has been added.
///
/// When this attribute is specified, the generated code will be printed to `stderr` during compilation.
///
/// Example:
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(BerSet)]
/// #[debug_derive]
/// struct S {
///   a: u32,
/// }
/// ```
pub use asn1_rs_derive::BerSet;

/// # DerSet custom derive
///
/// `DerSet` is a custom derive attribute, to derive both BER and DER [`Set`](super::Set) parsers automatically from the structure definition.
/// This attribute will automatically derive implementations for the following traits:
///   - [`TryFrom<Any>`](super::Any), also providing [`FromBer`](super::FromBer)
///   - [`Tagged`](super::Tagged)
///   - [`CheckDerConstraints`](super::CheckDerConstraints)
///   - [`FromDer`](super::FromDer)
///
/// `DerSet` implies `BerSet`, and will conflict with this custom derive.
///
/// Parsers will be automatically derived from struct fields. Every field type must implement the [`FromDer`](super::FromDer) trait.
///
/// See [`derive`](crate::doc::derive) documentation for more examples and documentation.
///
/// ## Examples
///
/// To parse the following ASN.1 structure:
/// <pre>
/// S ::= SEt {
///     a INTEGER(0..2^32),
///     b INTEGER(0..2^16),
///     c INTEGER(0..2^16),
/// }
/// </pre>
///
/// Define a structure and add the `DerSet` derive:
///
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(DerSet)]
/// struct S {
///   a: u32,
///   b: u16,
///   c: u16
/// }
/// ```
///
/// ## Debugging
///
/// To help debugging the generated code, the `#[debug_derive]` attribute has been added.
///
/// When this attribute is specified, the generated code will be printed to `stderr` during compilation.
///
/// Example:
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(DerSet)]
/// #[debug_derive]
/// struct S {
///   a: u32,
/// }
/// ```
pub use asn1_rs_derive::DerSet;

/// # BerAlias custom derive
///
/// `BerAlias` is a custom derive attribute, to derive a BER object parser automatically from the structure definition.
/// This attribute will automatically derive implementations for the following traits:
///   - [`TryFrom<Any>`](super::Any), also providing [`FromBer`](super::FromBer)
///   - [`Tagged`](super::Tagged)
///   - [`CheckDerConstraints`](super::CheckDerConstraints)
///   - [`FromDer`](super::FromDer)
///
/// `DerAlias` implies `BerAlias`, and will conflict with this custom derive. Use `BerAlias` when you only want the
/// above traits derived.
///
/// When defining alias, only unnamed (tuple) structs with one field are supported.
///
/// Parser will be automatically derived from the struct field. This field type must implement the `TryFrom<Any>` trait.
///
/// See [`derive`](crate::doc::derive) documentation for more examples and documentation.
///
/// ## Examples
///
/// To parse the following ASN.1 object:
/// <pre>
/// MyInt ::= INTEGER(0..2^32)
/// </pre>
///
/// Define a structure and add the `BerAlias` derive:
///
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(BerAlias)]
/// struct S(pub u32);
/// ```
///
/// ## Debugging
///
/// To help debugging the generated code, the `#[debug_derive]` attribute has been added.
///
/// When this attribute is specified, the generated code will be printed to `stderr` during compilation.
///
/// Example:
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(BerAlias)]
/// #[debug_derive]
/// struct S(pub u32);
/// ```
pub use asn1_rs_derive::BerAlias;

/// # DerAlias custom derive
///
/// `DerAlias` is a custom derive attribute, to derive a DER object parser automatically from the structure definition.
/// This attribute will automatically derive implementations for the following traits:
///   - [`TryFrom<Any>`](super::Any), also providing [`FromBer`](super::FromBer)
///   - [`Tagged`](super::Tagged)
///
/// `DerAlias` implies `BerAlias`, and will conflict with this custom derive.
///
/// When defining alias, only unnamed (tuple) structs with one field are supported.
///
/// Parser will be automatically derived from the struct field. This field type must implement the `TryFrom<Any>` and `FromDer` traits.
///
/// See [`derive`](crate::doc::derive) documentation for more examples and documentation.
///
/// ## Examples
///
/// To parse the following ASN.1 object:
/// <pre>
/// MyInt ::= INTEGER(0..2^32)
/// </pre>
///
/// Define a structure and add the `DerAlias` derive:
///
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(DerAlias)]
/// struct S(pub u32);
/// ```
///
/// ## Debugging
///
/// To help debugging the generated code, the `#[debug_derive]` attribute has been added.
///
/// When this attribute is specified, the generated code will be printed to `stderr` during compilation.
///
/// Example:
/// ```rust
/// use asn1_rs::*;
///
/// #[derive(DerAlias)]
/// #[debug_derive]
/// struct S(pub u32);
/// ```
pub use asn1_rs_derive::DerAlias;
