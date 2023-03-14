# Change Log

## [Unreleased][unreleased]

### Added/Changed/Fixed

### Thanks

## 0.14.0

### Added/Changed

- Add support for parsing signature parameters and value (closes #94)

- Change `ASN1Time::to_rfc2822()` to return a Result
- ASN1Time: modify `from_timestamp` to return a Result
- ASN1Time: implement Display
- Upgrade versions of asn1-rs, oid-registry and der-parser
- AlgorithmIdentifier: add const methods to create object/access fields
- Globally: start using `asn1-rs` types, simplify parsers:
  - AlgorithmIdentifier: automatically derive struct, use type ANY
  - Merge old FromDer trait into `asn1_rs::FromDer` (using X509Error)
  - Replace BitStringObject with BitString
  - AttributeTypeAndValue: use Any instead of DerObject
  - Extensions: replace UnparsedObject with Any
  - X509Error: add methods to simplify conversions
  - CRI Attributes: rewrite and simplify parsers
  - Simplify parsers for multiple types and extensions

### Fixed

- Fix ECDSA signature verification when CA and certificate use different curves

### Thanks

## 0.13.2

### Fixed

- Fix panic in ASN1Time::to_rfc2822() when year is less than 1900

## 0.13.1

### Fixed

- Fix regression with certificate verification for ECDSA signatures using the P-256 curve and SHA-384 (#118)
- Set minimum version of `time` to 0.3.7 (#119)
- Allow empty SEQUENCE when OPTIONAL, for ex in CRL extensions (#120)

### Thanks

- @SergioBenitez, @flavio, @acarlson0000

## 0.13.0

### Added/Changed/Fixed

Crate:
- Update to der-parser 7.0 and asn1-rs
- Remove chrono (#111)
- Set MSRV to 1.53

Validators:
- Add `Deref<Target=TbsCertificate>` trait to `X509Certificate`
- Add `Validator` trait and deprecate `Validate`
  * The previous validation is implemented in `X509StructureValidator`
  * Split some checks (not on structure) to `X509CertificateValidator`

Extensions:
- add support for nsComment
- add support for IssuerAltName
- start adding support for CT Signed Certificate Timestamp (rfc6962)
- raise error if a SAN entry cannot be parsed
- deprecate `TbsCertificate::find_extension()` and add preferred method `TbsCertificate::get_extension_unique()`:
  the latter checks for duplicate extensions (#113)

Signatures:
- Fix signature verification for EC curves (#116)

Public Keys:
- Add base functions for parsing public keys (RSA, DSA, GOST)

### Thanks

- @lilyball, @g2p

## 0.12.0

### Added/Changed/Fixed

- Upgrade to nom 7

## 0.11.0

### Added

- Add SubjectPublicKeyInfo::raw field

### Changed/Fixed

- Fix der-parser dependency (#102)
- Update oid-registry dependency (#77)
- Set MSRV to 1.46 (indirect dependency on lexical-core and bitvec)
- Extend the lifetimes exposed on TbsCertificate (#104)
- Add missing test assets (#103)

### Thanks

- @jgalenson, @g2p, @kpp

## 0.10.0

### Added

- Add the `Validate` trait to run post-parsing validations of X.509 structure
- Add the `FromDer` trait to unify parsing methods and visibility (#85)
- Add method to format X509Name using a given registry
- Add `X509Certificate::public_key()` method
- Add ED25519 as a signature algorithm (#95)
- Add support for extensions (#86):
  - CRL Distribution Points
- Add `X509CertificateParser` builder to allow specifying parsing options

### Changed/Fixed

- Extensions are now stored in order of appearance in the certificate/CRL (#80)
  - `.extensions` field is not public anymore, but methods `.extensions()` and `.extensions_map()`
    have been added
- Store CRI attributes in order
- Fix parsing of CertificatePolicies, and use named types (closes #82)
- Allow specifying registry in oid2sn and similar functions (closes #88)
- Mark X509Extension::new as const fn + inline
- Allow leading zeroes in serial number
- Derive `Clone` for all types (when possible) (#89)
- Fix certificate validity period check to be inclusive (#90)
- Do not fail GeneralName parsing for x400Address and ediPartyName, read it as unparsed objects (#87)
- Change visibility of fields in `X509Name` (replaced by accessors)

### Thanks

- @lilyball for numerous issues, ideas and comments
- @SergioBenitez for lifetimes fixes (#93) and validity period check fixes (#90)
- @rappet for Ed25519 signature verification support (#95)
- @xonatius for the work on CRLDistributionPoints (#96, #98)

## 0.9.3

### Added/Changed/Fixed

- Add functions oid2description() and oid_registry() (closes #79)
- Fix typo 'ocsp_signing' (closes #84)
- Extension: use specific variant if unsupported or failed to parse (closes #83)
- Relax constrains on parsing to accept certificates that do not strictly respect
  DER encoding, but are widely accepted by other X.509 libraries:
  - SubjectAltName: accept non-ia5string characters
  - Extensions: accept boolean values not enoded as `00` or `ff`
  - Serial: build BigUint from raw bytes (do not check sign)

## 0.9.2

### Added/Changed/Fixed

- Remove der-oid-macro from dependencies, not used directly
- Use der_parser::num_bigint, remove it from direct dependencies
- Add methods to iterate all blocks from a PEM file (#75)
- Update MSRV to 1.45.0

## 0.9.1

### Added/Changed/Fixed

- Fix: X509Name::iter_state_or_province OID value
- Re-export oid-registry, and add doc to show how to access OID

### Thanks

- @0xazure for fixing X509Name::iter_state_or_province

## 0.9.0

### Added/Changed/Fixed

- Upgrade to `nom` 6.0
- Upgrade to `der-parser` 5.0
- Upgrade MSRV to 1.44.0
- Re-export crates so crate users do not have to import them

- Add function parse_x509_pem and deprecate pem_to_der (#53)
- Add helper methods to X509Name and simplify accessing values
- Add support for ReasonCode extension
- Add support for InvalidityDate extension
- Add support for CRL Number extension
- Add support for Certificate Signing Request (#58)

- Change type of X509Version (now directly using the u32 value)
- X509Name: relax check, allow some non-rfc compliant strings (#50)
- Relax some constraints for invalid dates
- CRL: extract raw serial, and add methods to access it
- CRL: add method to iterate revoked certificates
- RevokedCertificate: convert extensions list to hashmap

- Refactor crate modules and visibility
- Rename top-level functions to `parse_x509_certificate` and parse_x509_crl`

- Refactor error handling, return meaningful errors when possible
- Make many more functions public (parse_tbs_certificate, etc.)

### Thanks

- Dirkjan Ochtman (@djc): support for Certificate Signing Request (CSR), code refactoring, etc.

## 0.8.0

### Added/Changed

- Upgrade to `der-parser` 4.0
- Move from `time` to `chrono`
  - `time 0.1 is very old, and time 0.2 broke compatibility and cannot parse timezones
  - Add public type `ASN1Time` object to abstract implementation
  - *this breaks API for direct access to `not_before`, `not_after` etc.*
- Fix clippy warnings
  - `nid2obj` argument is now passed by copy, not reference
- Add method to get a formatted string of the certificate serial number
- Add method to get decoded version
- Add convenience methods to access the most common fields (subject, issuer, etc.)
- Expose the raw DER of an X509Name
- Make `parse_x509_name` public, for parsing distinguished names
- Make OID objects public
- Implement parsing for some extensions
  - Support for extensions is not complete, support for more types will be added later
- Add example to decode and print certificates
- Add `verify` feature to verify cryptographic signature by a public key

### Fixed

- Fix parsing of types not representable by string in X509Name (#36)
- Fix parsing of certificates with empty subject (#37)

### Thanks

- @jannschu, @g2p for the extensions parsing
- @wayofthepie for the tests and contributions
- @nicholasbishop for contributions

## 0.7.0

- Expose raw bytes of the certificate serial number
- Set edition to 2018

## 0.6.4

- Fix infinite loop when certificate has no END mark

## 0.6.3

- Fix infinite loop when reading non-pem data (#28)

## 0.6.2

- Remove debug code left in `Pem::read`

## 0.6.1

- Add CRL parser
- Expose CRL tbs bytes
- PEM: ignore lines before BEGIN label (#21)
- Fix parsing default values for TbsCertificate version field (#24)
- Use BerResult from der-parser for simpler function signatures
- Expose tbsCertificate bytes
- Upgrade dependencies (base64)

## 0.6.0

- Update to der-parser 3.0 and nom 5
- Breaks API, cleaner error types

## 0.5.1

- Add `time_to_expiration` to `Validity` object
- Add method to read a `Pem` object from `BufRead + Seek`
- Add method to `Pem` to decode and extract certificate

## 0.5.0

- Update to der-parser 2.0

## 0.4.3

- Make `parse_subject_public_key_info` public
- Add function `sn2oid` (get an OID by short name)

## 0.4.2

- Support GeneralizedTime conversion

## 0.4.1

- Fix case where certificate has no extensions

## 0.4.0

- Upgrade to der-parser 1.1, and Use num-bigint over num
- Rename x509_parser to parse_x509_der
- Do not export subparsers
- Improve documentation

## 0.3.0

- Upgrade to nom 4

## 0.2.0

- Rewrite X.509 structures and parsing code to work in one pass
  **Warning: this is a breaking change**
- Add support for PEM-encoded certificates
- Add some documentation


