# josekit

JOSE (Javascript Object Signing and Encryption: JWT, JWS, JWE, JWA, JWK) library based on OpenSSL for Rust.

## Install

```toml
[dependencies]
josekit = "0.8.1"
```

This library depends on OpenSSL 1.1.1 DLL. Read more about [Crate openssl](https://docs.rs/openssl/). 

## Build

```sh
sudo apt install build-essential pkg-config libssl-dev
cd josekit-rs
cargo build --release
```

<!-- 
## Publish

```sh
cargo test
cargo publish --dry-run
git tag vX.X.X
git push origin --tags
cargo publish
```
-->

## Supported signing algorithms

<table>
<thead>
    <tr>
        <th width="30%">Name</th>
        <th width="45%">Description</th>
        <th width="25%">Key Type</th>
    </tr>
</thead>
<tbody>
    <tr>
        <td>HS256</td>
        <td>HMAC using SHA-256</td>
        <td>oct (size: 32 bytes or more)</td>
    </tr>
    <tr>
        <td>HS384</td>
        <td>HMAC using SHA-384</td>
        <td>oct (size: 48 bytes or more)</td>
    </tr>
    <tr>
        <td>HS512</td>
        <td>HMAC using SHA-512</td>
        <td>oct (size: 64 bytes or more)</td>
    </tr>
    <tr>
        <td>RS256</td>
        <td>RSASSA-PKCS1-v1_5 using SHA-256</td>
        <td rowspan="6">RSA (size: 1024 bits or more)</td>
    </tr>
    <tr>
        <td>RS384</td>
        <td>RSASSA-PKCS1-v1_5 using SHA-384</td>
    </tr>
    <tr>
        <td>RS512</td>
        <td>RSASSA-PKCS1-v1_5 using SHA-512</td>
    </tr>
    <tr>
        <td>PS256</td>
        <td>RSASSA-PSS using SHA-256 and MGF1 with SHA-256</td>
    </tr>
    <tr>
        <td>PS384</td>
        <td>RSASSA-PSS using SHA-384 and MGF1 with SHA-384</td>
    </tr>
    <tr>
        <td>PS512</td>
        <td>RSASSA-PSS using SHA-512 and MGF1 with SHA-512</td>
    </tr>
    <tr>
        <td>ES256</td>
        <td>ECDSA using P-256 and SHA-256</td>
        <td>EC (curve: P-256)</td>
    </tr>
    <tr>
        <td>ES384</td>
        <td>ECDSA using P-384 and SHA-384</td>
        <td>EC (curve: P-384)</td>
    </tr>
    <tr>
        <td>ES512</td>
        <td>ECDSA using P-521 and SHA-512</td>
        <td>EC (curve: P-521)</td>
    </tr>
    <tr>
        <td>ES256K</td>
        <td>ECDSA using secp256k1 curve and SHA-256</td>
        <td>EC (curve: secp256k1)</td>
    </tr>
    <tr>
        <td>EdDSA</td>
        <td>EdDSA signature algorithms</td>
        <td>OKP (curve: Ed25519 or Ed448)</td>
    </tr>
    <tr>
        <td>none</td>
        <td>No digital signature or MAC performed</td>
        <td>-</td>
    </tr>
</tbody>
</table>

## Supported encryption algorithms

<table>
<thead>
    <tr>
        <th width="30%">Name</th>
        <th width="45%">Description</th>
        <th width="25%">Key Type</th>
    </tr>
</thead>
<tbody>
    <tr>
        <td>dir</td>
        <td>Direct use of a shared symmetric key as the CEK</td>
        <td>oct (size: the CEK depended. See below)
            <ul>
                <li>A128CBC-HS256: 32 bytes</li>
                <li>A192CBC-HS384: 48 bytes</li>
                <li>A256CBC-HS512: 64 bytes</li>
                <li>A128GCM: 16 bytes</li>
                <li>A192GCM: 24 bytes</li>
                <li>A256GCM: 32 bytes</li>
            </ul>
        </td>
    </tr>
    <tr>
        <td>ECDH-ES</td>
        <td>Elliptic Curve Diffie-Hellman Ephemeral Static key agreement using Concat KDF</td>
        <td rowspan="4">EC (curve: P-256, P-384, P-521 or secp256k1)<br />
            OKP (curve: X25519 or X448)</td>
    </tr>
    <tr>
        <td>ECDH-ES+A128KW</td>
        <td>ECDH-ES using Concat KDF and CEK wrapped with "A128KW"</td>
    </tr>
    <tr>
        <td>ECDH-ES+A192KW</td>
        <td>ECDH-ES using Concat KDF and CEK wrapped with "A192KW"</td>
    </tr>
    <tr>
        <td>ECDH-ES+A256KW</td>
        <td>ECDH-ES using Concat KDF and CEK wrapped with "A256KW"</td>
    </tr>
    <tr>
        <td>A128KW</td>
        <td>AES Key Wrap with default initial value using 128-bit key</td>
        <td>oct (size: 16 bytes)</td>
    </tr>
    <tr>
        <td>A192KW</td>
        <td>AES Key Wrap with default initial value using 192-bit key</td>
        <td>oct (size: 24 bytes)</td>
    </tr>
    <tr>
        <td>A256KW</td>
        <td>AES Key Wrap with default initial value using 256-bit key</td>
        <td>oct (size: 32 bytes)</td>
    </tr>
    <tr>
        <td>A128GCMKW</td>
        <td>Key wrapping with AES GCM using 128-bit key</td>
        <td>oct (size: 16 bytes)</td>
    </tr>
    <tr>
        <td>A192GCMKW</td>
        <td>Key wrapping with AES GCM using 192-bit key</td>
        <td>oct (size: 24 bytes)</td>
    </tr>
    <tr>
        <td>A256GCMKW</td>
        <td>Key wrapping with AES GCM using 256-bit key</td>
        <td>oct (size: 32 bytes)</td>
    </tr>
    <tr>
        <td>PBES2-HS256+A128KW</td>
        <td>PBES2 with HMAC SHA-256 and "A128KW" wrapping</td>
        <td rowspan="3">oct (size: 1 bytes or more)</td>
    </tr>
    <tr>
        <td>PBES2-HS384+A192KW</td>
        <td>PBES2 with HMAC SHA-384 and "A192KW" wrapping</td>
    </tr>
    <tr>
        <td>PBES2-HS512+A256KW</td>
        <td>PBES2 with HMAC SHA-512 and "A256KW" wrapping</td>
    </tr>
    <tr>
        <td>RSA1_5</td>
        <td>RSAES-PKCS1-v1_5</td>
        <td rowspan="5">RSA (size: 1024 bits or more)</td>
    </tr>
    <tr>
        <td>RSA-OAEP</td>
        <td>RSAES OAEP using default parameters</td>
    </tr>
    <tr>
        <td>RSA-OAEP-256</td>
        <td>RSAES OAEP using SHA-256 and MGF1 with SHA-256</td>
    </tr>
    <tr>
        <td>RSA-OAEP-384</td>
        <td>RSAES OAEP using SHA-384 and MGF1 with SHA-384</td>
    </tr>
    <tr>
        <td>RSA-OAEP-512</td>
        <td>RSAES OAEP using SHA-512 and MGF1 with SHA-512</td>
    </tr>
</tbody>
</table>

## Supported key formats

### Private Key

<table style="width:100%">
<thead>
<tr>
    <th width="25%" rowspan="2">Algorithm</th>
    <th width="15%" rowspan="2">JWK</th>
    <th colspan="2">PEM</th>
    <th colspan="2">DER</th>
</tr>
<tr>
    <th width="15%">PKCS#8</th>
    <th width="15%">Traditional</th>
    <th width="15%">PKCS#8</th>
    <th width="15%" >Raw</th>
</tr>
</thead>
<tbody>
<tr>
    <td>RSA</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
</tr>
<tr>
    <td>RSA-PSS</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
</tr>
<tr>
    <td>EC</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
</tr>
<tr>
    <td>ED</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>-</td>
</tr>
<tr>
    <td>ECX</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>-</td>
</tr>
</tbody>
</table>

### Public Key

<table style="width:100%">
<thead>
<tr>
    <th width="25%" rowspan="2">Algorithm</th>
    <th width="15%" rowspan="2">JWK</th>
    <th colspan="2">PEM</th>
    <th colspan="2">DER</th>
</tr>
<tr>
    <th width="15%">SPKI</th>
    <th width="15%">Traditional</th>
    <th width="15%">SPKI</th>
    <th width="15%">Raw</th>
</tr>
</thead>
<tbody>
<tr>
    <td>RSA</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
</tr>
<tr>
    <td>RSA-PSS</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
    <td>OK</td>
</tr>
<tr>
    <td>EC</td>
    <td>OK</td>
    <td>OK</td>
    <td>-</td>
    <td>OK</td>
    <td>-</td>
</tr>
<tr>
    <td>ED</td>
    <td>OK</td>
    <td>OK</td>
    <td>-</td>
    <td>OK</td>
    <td>-</td>
</tr>
<tr>
    <td>ECX</td>
    <td>OK</td>
    <td>OK</td>
    <td>-</td>
    <td>OK</td>
    <td>-</td>
</tr>
</tbody>
</table>

## Usage

### Signing a JWT by HMAC

HMAC is used to verify the integrity of a message by common secret key.
Three algorithms are available for HMAC: HS256, HS384, and HS512.

You can use any bytes as the key. But the key length must be larger than
or equal to the output hash size.

```rust
use josekit::{JoseError, jws::{JwsHeader, HS256}, jwt::{self, JwtPayload}};

fn main() -> Result<(), JoseError> {
    let mut header = JwsHeader::new();
    header.set_token_type("JWT");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    let key = b"0123456789ABCDEF0123456789ABCDEF";

    // Signing JWT
    let signer = HS256.signer_from_bytes(key)?;
    let jwt = jwt::encode_with_signer(&payload, &header, &signer)?;

    // Verifing JWT
    let verifier = HS256.verifier_from_bytes(key)?;
    let (payload, header) = jwt::decode_with_verifier(&jwt, &verifier)?;

    Ok(())
}
```

### Signing a JWT by RSASSA

RSASSA is used to verify the integrity of a message by two keys: public and private.
Three algorithms are available for RSASSA: RS256, RS384, and RS512.

You can generate the keys by executing openssl command.

```sh
# Generate a new private key. Keygen bits must be 2048 or more.
openssl openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out private.pem

# Generate a public key from the private key.
openssl pkey -in private.pem -pubout -out public.pem
```

```rust
use josekit::{JoseError, jws::{JwsHeader, RS256}, jwt::{self, JwtPayload}};

const PRIVATE_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/RSA_2048bit_private.pem");
const PUBLIC_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/RSA_2048bit_public.pem");

fn main() -> Result<(), JoseError> {
    let mut header = JwsHeader::new();
    header.set_token_type("JWT");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    // Signing JWT
    let private_key = std::fs::read(PRIVATE_KEY).unwrap();
    let signer = RS256.signer_from_pem(&private_key)?;
    let jwt = jwt::encode_with_signer(&payload, &header, &signer)?;

    // Verifing JWT
    let public_key = std::fs::read(PUBLIC_KEY).unwrap();
    let verifier = RS256.verifier_from_pem(&public_key)?;
    let (payload, header) = jwt::decode_with_verifier(&jwt, &verifier)?;
    
    Ok(())
}
```

### Signing a JWT by RSASSA-PSS

RSASSA-PSS is used to verify the integrity of a message by two keys: public and private.

The raw key format of RSASSA-PSS is the same as RSASSA. So you should use a PKCS#8 wrapped key. It contains some optional attributes.

Three algorithms are available for RSASSA-PSS: PS256, PS384, and PS512.
You can generate the keys by executing openssl command.

```sh
# Generate a new private key

# for PS256
openssl genpkey -algorithm RSA-PSS -pkeyopt rsa_keygen_bits:2048 -pkeyopt rsa_pss_keygen_md:sha256 -pkeyopt rsa_pss_keygen_mgf1_md:sha256 -pkeyopt rsa_pss_keygen_saltlen:32 -out private.pem

# for PS384
openssl genpkey -algorithm RSA-PSS -pkeyopt rsa_keygen_bits:2048 -pkeyopt rsa_pss_keygen_md:sha384 -pkeyopt rsa_pss_keygen_mgf1_md:sha384 -pkeyopt rsa_pss_keygen_saltlen:48 -out private.pem

# for PS512
openssl genpkey -algorithm RSA-PSS -pkeyopt rsa_keygen_bits:2048 -pkeyopt rsa_pss_keygen_md:sha512 -pkeyopt rsa_pss_keygen_mgf1_md:sha512 -pkeyopt rsa_pss_keygen_saltlen:64 -out private.pem

# Generate a public key from the private key.
openssl pkey -in private.pem -pubout -out public.pem
```

```rust
use josekit::{JoseError, jws::{JwsHeader, PS256}, jwt::{self, JwtPayload}};

const PRIVATE_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/RSA-PSS_2048bit_SHA-256_private.pem");
const PUBLIC_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/RSA-PSS_2048bit_SHA-256_public.pem");

fn main() -> Result<(), JoseError> {
    let mut header = JwsHeader::new();
    header.set_token_type("JWT");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    // Signing JWT
    let private_key = std::fs::read(PRIVATE_KEY).unwrap();
    let signer = PS256.signer_from_pem(&private_key)?;
    let jwt = jwt::encode_with_signer(&payload, &header, &signer)?;

    // Verifing JWT
    let public_key = std::fs::read(PUBLIC_KEY).unwrap();
    let verifier = PS256.verifier_from_pem(&public_key)?;
    let (payload, header) = jwt::decode_with_verifier(&jwt, &verifier)?;

    Ok(())
}
```

### Signing a JWT by ECDSA

ECDSA is used to verify the integrity of a message by two keys: public and private.
Four algorithms are available for ECDSA: ES256, ES384, ES512 and ES256K.

You can generate the keys by executing openssl command.

```sh
# Generate a new private key

# for ES256
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out private.pem

# for ES384
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-384 -out private.pem

# for ES512
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-521 -out private.pem

# for ES256K
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:secp256k1 -out private.pem

# Generate a public key from the private key.
openssl pkey -in private.pem -pubout -out public.pem
```

```rust
use josekit::{JoseError, jws::{JwsHeader, ES256}, jwt::{self, JwtPayload}};

const PRIVATE_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/EC_P-256_private.pem");
const PUBLIC_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/EC_P-256_public.pem");

fn main() -> Result<(), JoseError> {
    let mut header = JwsHeader::new();
    header.set_token_type("JWT");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    // Signing JWT
    let private_key = std::fs::read(PRIVATE_KEY).unwrap();
    let signer = ES256.signer_from_pem(&private_key)?;
    let jwt = jwt::encode_with_signer(&payload, &header, &signer)?;

    // Verifing JWT
    let public_key = std::fs::read(PUBLIC_KEY).unwrap();
    let verifier = ES256.verifier_from_pem(&public_key)?;
    let (payload, header) = jwt::decode_with_verifier(&jwt, &verifier)?;

    Ok(())
}
```

### Signing a JWT by EdDSA

EdDSA is used to verify the integrity of a message by two keys: public and private.
A algorithm is only available "EdDSA" for EdDSA.
But it has two curve types: Ed25519, Ed448.

You can generate the keys by executing openssl command.

```sh
# Generate a new private key

# for Ed25519
openssl genpkey -algorithm ED25519 -out private.pem

# for Ed448
openssl genpkey -algorithm ED448 -out private.pem

# Generate a public key from the private key.
openssl pkey -in private.pem -pubout -out public.pem
```

```rust
use josekit::{JoseError, jws::{JwsHeader, EdDSA}, jwt::{self, JwtPayload}};

const PRIVATE_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/ED25519_private.pem");
const PUBLIC_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/ED25519_public.pem");

fn main() -> Result<(), JoseError> {
    let mut header = JwsHeader::new();
    header.set_token_type("JWT");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    // Signing JWT
    let private_key = std::fs::read(PRIVATE_KEY).unwrap();
    let signer = EdDSA.signer_from_pem(&private_key)?;
    let jwt = jwt::encode_with_signer(&payload, &header, &signer)?;

    // Verifing JWT
    let public_key = std::fs::read(PUBLIC_KEY).unwrap();
    let verifier = EdDSA.verifier_from_pem(&public_key)?;
    let (payload, header) = jwt::decode_with_verifier(&jwt, &verifier)?;

    Ok(())
}
```

### Encrypting a JWT by a Direct method

A "Direct" method is used to encrypt a message by CEK (content encryption key).
The algorithm name is "dir" only.

You can use any bytes as the key. But the length must be the same as the length of the CEK.

```rust
use josekit::{JoseError, jwe::{JweHeader, Dir}, jwt::{self, JwtPayload}};

fn main() -> Result<(), JoseError> {
    let mut header = JweHeader::new();
    header.set_token_type("JWT");
    header.set_content_encryption("A128CBC-HS256");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    let key = b"0123456789ABCDEF0123456789ABCDEF";

    // Encrypting JWT
    let encrypter = Dir.encrypter_from_bytes(key)?;
    let jwt = jwt::encode_with_encrypter(&payload, &header, &encrypter)?;

    // Decrypting JWT
    let decrypter = Dir.decrypter_from_bytes(key)?;
    let (payload, header) = jwt::decode_with_decrypter(&jwt, &decrypter)?;

    Ok(())
}
```

### Encrypting a JWT by ECDH-ES

ECDH-ES is used to encrypt a message a message by random bytes as CEK (content encryption key)
and the CEK is delivered safely by two keys: public and private.
Four algorithms are available for ECDH-ES: ECDH-ES, ECDH-ES+A128KW, ECDH-ES+A192KW and ECDH-ES+A256KW.

The types of key are available both EC and ECX.
The EC key has four curve types: P-256, P-384, P-521 and secp256k1.
The ECX key has two curve types: X25519 and X448.

You can generate the keys by executing openssl command.

```sh
# Generate a new private key

# for P-256 EC key
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out private.pem

# for P-384 EC key
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-384 -out private.pem

# for P-521 EC key
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-521 -out private.pem

# for secp256k1 EC key
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:secp256k1 -out private.pem

# for X25519 ECX key
openssl genpkey -algorithm X25519 -out private.pem

# for X448 ECX key
openssl genpkey -algorithm X448 -out private.pem

# Generate a public key from the private key.
openssl pkey -in private.pem -pubout -out public.pem
```

```rust
use josekit::{JoseError, jwe::{JweHeader, ECDH_ES}, jwt::{self, JwtPayload}};

const PRIVATE_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/EC_P-256_private.pem");
const PUBLIC_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/EC_P-256_public.pem");

fn main() -> Result<(), JoseError> {
    let mut header = JweHeader::new();
    header.set_token_type("JWT");
    header.set_content_encryption("A128CBC-HS256");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    // Encrypting JWT
    let public_key = std::fs::read(PUBLIC_KEY).unwrap();
    let encrypter = ECDH_ES.encrypter_from_pem(&public_key)?;
    let jwt = jwt::encode_with_encrypter(&payload, &header, &encrypter)?;

    // Decrypting JWT
    let private_key = std::fs::read(PRIVATE_KEY).unwrap();
    let decrypter = ECDH_ES.decrypter_from_pem(&private_key)?;
    let (payload, header) = jwt::decode_with_decrypter(&jwt, &decrypter)?;

    Ok(())
}
```

### Encrypting a JWT by AESKW

AES is used to encrypt a message by random bytes as CEK (content encryption key)
and the CEK is wrapping by common secret key.
Three algorithms are available for AES: A128KW, A192KW and A256KW.

You can use any bytes as the key. But the length must be AES key size.

```rust
use josekit::{JoseError, jwe::{JweHeader, A128KW}, jwt::{self, JwtPayload}};

fn main() -> Result<(), JoseError> {
    let mut header = JweHeader::new();
    header.set_token_type("JWT");
    header.set_content_encryption("A128CBC-HS256");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    let key = b"0123456789ABCDEF";

    // Encrypting JWT
    let encrypter = A128KW.encrypter_from_bytes(key)?;
    let jwt = jwt::encode_with_encrypter(&payload, &header, &encrypter)?;

    // Decrypting JWT
    let decrypter = A128KW.decrypter_from_bytes(key)?;
    let (payload, header) = jwt::decode_with_decrypter(&jwt, &decrypter)?;
    Ok(())
}
```

### Encrypting a JWT by AES-GCM

AES-GCM is used to encrypt a message by random bytes as CEK (content encryption key)
and the CEK is wrapping by common secret key.
Three algorithms are available for AES-GCM: A128GCMKW, A192GCMKW and A256GCMKW.

You can use any bytes as the key. But the length must be AES key size.

```rust
use josekit::{JoseError, jwe::{JweHeader, A128GCMKW}, jwt::{self, JwtPayload}};

fn main() -> Result<(), JoseError> {
    let mut header = JweHeader::new();
    header.set_token_type("JWT");
    header.set_content_encryption("A128CBC-HS256");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    let key = b"0123456789ABCDEF";

    // Encrypting JWT
    let encrypter = A128GCMKW.encrypter_from_bytes(key)?;
    let jwt = jwt::encode_with_encrypter(&payload, &header, &encrypter)?;

    // Decrypting JWT
    let decrypter = A128GCMKW.decrypter_from_bytes(key)?;
    let (payload, header) = jwt::decode_with_decrypter(&jwt, &decrypter)?;
    Ok(())
}
```

### Encrypting a JWT by PBES2-HMAC+AESKW

PBES2-HMAC+AES is used to encrypt a message by random bytes as CEK (content encryption key)
and the CEK is wrapping by common secret key.
Three algorithms are available for AES-GCM: PBES2-HS256+A128KW, PBES2-HS384+A192KW and PBES2-HS512+A256KW.

You can use any bytes as the key. But a password is recommended that the length is no shorter 
than AES key size and no longer than 128 octets.

```rust
use josekit::{JoseError, jwe::{JweHeader, PBES2_HS256_A128KW}, jwt::{self, JwtPayload}};

fn main() -> Result<(), JoseError> {
    let mut header = JweHeader::new();
    header.set_token_type("JWT");
    header.set_content_encryption("A128CBC-HS256");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    let key = b"01234567";

    // Encrypting JWT
    let encrypter = PBES2_HS256_A128KW.encrypter_from_bytes(key)?;
    let jwt = jwt::encode_with_encrypter(&payload, &header, &encrypter)?;

    // Decrypting JWT
    let decrypter = PBES2_HS256_A128KW.decrypter_from_bytes(key)?;
    let (payload, header) = jwt::decode_with_decrypter(&jwt, &decrypter)?;
    Ok(())
}
```

### Encrypting a JWT by RSAES

RSAES is used to encrypt a message a message by random bytes as CEK (content encryption key)
and the CEK is delivered safely by two keys: public and private.
Two algorithms are available for now: RSA1_5, RSA-OAEP.

You can generate the keys by executing openssl command.

```sh
# Generate a new private key. Keygen bits must be 2048 or more.
openssl openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out private.pem

# Generate a public key from the private key.
openssl pkey -in private.pem -pubout -out public.pem
```

```rust
use josekit::{JoseError, jwe::{JweHeader, RSA_OAEP}, jwt::{self, JwtPayload}};

const PRIVATE_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/RSA_2048bit_private.pem");
const PUBLIC_KEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), 
    "/data/pem/RSA_2048bit_public.pem");

fn main() -> Result<(), JoseError> {
    let mut header = JweHeader::new();
    header.set_token_type("JWT");
    header.set_content_encryption("A128CBC-HS256");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    // Encrypting JWT
    let public_key = std::fs::read(PUBLIC_KEY).unwrap();
    let encrypter = RSA_OAEP.encrypter_from_pem(&public_key)?;
    let jwt = jwt::encode_with_encrypter(&payload, &header, &encrypter)?;

    // Decrypting JWT
    let private_key = std::fs::read(PRIVATE_KEY).unwrap();
    let decrypter = RSA_OAEP.decrypter_from_pem(&private_key)?;
    let (payload, header) = jwt::decode_with_decrypter(&jwt, &decrypter)?;
    Ok(())
}
```

### Unsecured JWT

```rust
use josekit::{JoseError, jws::JwsHeader, jwt::{self, JwtPayload}};

fn main() -> Result<(), JoseError> {
    let mut header = JwsHeader::new();
    header.set_token_type("JWT");

    let mut payload = JwtPayload::new();
    payload.set_subject("subject");

    let jwt = jwt::encode_unsecured(&payload, &header)?;
    let (payload, header) = jwt::decode_unsecured(&jwt)?;
    Ok(())
}
```

### Validate payload

```rust,should_panic
use josekit::{JoseError, jwt::{JwtPayload, JwtPayloadValidator}};
use std::time::{Duration, SystemTime};

fn main() -> Result<(), JoseError> {
    let mut validator = JwtPayloadValidator::new();
    // value based validation
    validator.set_issuer("http://example.com");
    validator.set_audience("user1");
    validator.set_jwt_id("550e8400-e29b-41d4-a716-446655440000");

    // time based validation: not_before <= base_time < expires_at
    validator.set_base_time(SystemTime::now() + Duration::from_secs(30));

    // issued time based validation: min_issued_time <= issued_time <= max_issued_time
    validator.set_min_issued_time(SystemTime::now() - Duration::from_secs(48 * 60));
    validator.set_max_issued_time(SystemTime::now() + Duration::from_secs(24 * 60));

    let mut payload = JwtPayload::new();

    validator.validate(&payload)?;

    Ok(())
}
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## References

- [RFC7515: JSON Web Signature (JWS)](https://tools.ietf.org/html/rfc7515)
- [RFC7516: JSON Web Encryption (JWE)](https://tools.ietf.org/html/rfc7516)
- [RFC7517: JSON Web Key (JWK)](https://tools.ietf.org/html/rfc7517)
- [RFC7518: JSON Web Algorithms (JWA)](https://tools.ietf.org/html/rfc7518)
- [RFC7519: JSON Web Token (JWT)](https://tools.ietf.org/html/rfc7519)
- [RFC7797: JSON Web Signature (JWS) Unencoded Payload Option](https://tools.ietf.org/html/rfc7797)
- [RFC8017: PKCS #1: RSA Cryptography Specifications Version 2.2](https://tools.ietf.org/html/rfc8017)
- [RFC5208: PKCS #8: Private-Key Information Syntax Specification Version 1.2](https://tools.ietf.org/html/rfc5208)
- [RFC5280: Internet X.509 Public Key Infrastructure Certificate and Certificate Revocation List (CRL) Profile](https://tools.ietf.org/html/rfc5280)
- [RFC5480: Elliptic Curve Cryptography Subject Public Key Information](https://tools.ietf.org/html/rfc5480)
- [RFC5915: Elliptic Curve Private Key Structure](https://tools.ietf.org/html/rfc5915)
- [RFC6979: Deterministic Usage of the Digital Signature Algorithm (DSA) and Elliptic Curve Digital Signature Algorithm (ECDSA)](https://tools.ietf.org/html/rfc6979)
- [RFC8410: Algorithm Identifiers for Ed25519, Ed448, X25519, and X448 for Use in the Internet X.509 Public Key Infrastructure](https://tools.ietf.org/html/rfc8410)
- [RFC8037: CFRG Elliptic Curve Diffie-Hellman (ECDH) and Signatures in JSON Object Signing and Encryption (JOSE)](https://tools.ietf.org/html/rfc8037)
- [RFC7468: Textual Encodings of PKIX, PKCS, and CMS Structures](https://tools.ietf.org/html/rfc7468)
