## RSA keypair

### Create RSA PKCS#8 PEM private key
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out rsa_2048_private.pem

### Generate RSA PKCS#8 PEM public key from RSA PKCS#8 PEM private key
openssl pkey -in rsa_2048_private.pem -pubout -outform PEM -out rsa_2048_public.pem

### Convert RSA private key from PKCS#8 PEM to PKCS#8 DER
openssl pkcs8 -nocrypt -in rsa_2048_private.pem -topk8 -outform DER -out rsa_2048_private.der

## Generate RSA PKCS#8 DER public key from RSA PKCS#8 PEM private key
openssl pkey -in rsa_2048_private.pem -pubout -outform DER -out rsa_2048_public.der

### Convert RSA private key from PKCS#8 DER to PKCS#8 PEM
openssl pkey -inform DER -in rsa_2048_private.der -out rsa_2048_private.pem

### Convert RSA private key from PKCS#8 PEM to PKCS#1 PEM
openssl pkcs8 -nocrypt -in rsa_2048_private.pem -traditional -out rsa_2048_pk1_private.pem

### Convert RSA private key from PKCS#8 PEM to PKCS#1 DER
openssl pkey -in rsa_2048_private.pem -outform DER -out rsa_2048_pk1_private.der

### Convert RSA private key from PKCS#1 PEM to PKCS#8 PEM
openssl pkcs8 -nocrypt -in rsa_2048_pk1_private.pem -topk8 -out rsa_2048_private.pem

### Generate RSA PKCS#1 PEM public key from RSA PKCS#8 PEM private key
openssl rsa -in rsa_2048_private.pem -RSAPublicKey_out -out rsa_2048_pk1_public.pem

### Generate RSA PKCS#1 DER public key from RSA PKCS#8 PEM private key
openssl rsa -in rsa_2048_private.pem -RSAPublicKey_out -outform DER -out rsa_2048_pk1_public.der

## ECDSA keypair

### Create ECDSA PKCS#8 PEM private key
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -outform PEM -out ecdsa_p256_private.pem
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-384 -outform PEM -out ecdsa_p384_private.pem
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-521 -outform PEM -out ecdsa_p521_private.pem
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:secp256k1 -outform PEM -out ecdsa_p521k_private.pem

### Generate ECDSA PKCS#8 PEM public key from ECDSA PKCS#8 PEM private key
openssl pkey -in ecdsa_p256_private.pem -pubout -outform PEM -out ecdsa_p256_public.pem

### Convert ECDSA private key from PKCS#8 PEM to PKCS#8 DER
openssl pkcs8 -nocrypt -in ecdsa_p256_private.pem -topk8 -outform DER -out ecdsa_p256_private.der

### Generate ECDSA PKCS#8 DER public key from ECDSA PKCS#8 PEM private key
openssl pkey -in ecdsa_p256_private.pem -pubout -outform DER -out ecdsa_p256_public.der

### Convert ECDSA private key from PKCS#8 DER to PKCS#8 PEM
openssl pkey -inform DER -in ecdsa_p256_private.der -out ecdsa_p256_private.pem

### Convert ECDSA private key from PKCS#8 PEM to PKCS#1 PEM
openssl pkcs8 -nocrypt -in ecdsa_p256_private.pem -traditional -out ecdsa_p256_pk1_private.pem

### Convert ECDSA private key from PKCS#8 PEM to PKCS#1 DER
openssl pkey -in ecdsa_p256_private.pem -outform DER -out ecdsa_p256_pk1_private.der

### Convert ECDSA private key from PKCS#1 PEM to PKCS#8 PEM
openssl pkcs8 -nocrypt -in ecdsa_p256_pk1_private.pem -topk8 -out ecdsa_p256_private.pem

## RSA-PSS keypair

### Create RSA-PSS PKCS#8 PEM private key
openssl genpkey -algorithm RSA-PSS -pkeyopt rsa_keygen_bits:2048 -pkeyopt rsa_pss_keygen_md:sha256 -pkeyopt rsa_pss_keygen_mgf1_md:sha256 -pkeyopt rsa_pss_keygen_saltlen:32 -out rsapss_2048_sha256_pkcs8_private.pem
openssl genpkey -algorithm RSA-PSS -pkeyopt rsa_keygen_bits:2048 -pkeyopt rsa_pss_keygen_md:sha384 -pkeyopt rsa_pss_keygen_mgf1_md:sha384 -pkeyopt rsa_pss_keygen_saltlen:48 -out rsapss_2048_sha384_pkcs8_private.pem
openssl genpkey -algorithm RSA-PSS -pkeyopt rsa_keygen_bits:2048 -pkeyopt rsa_pss_keygen_md:sha512  -pkeyopt rsa_pss_keygen_mgf1_md:sha512 -pkeyopt rsa_pss_keygen_saltlen:64 -out rsapss_2048_sha512_pkcs8_private.pem

### Generate RSA-PSS PKCS#8 PEM public key from RSA PKCS#8 PEM private key
openssl pkey -in rsapss_2048_sha256_pkcs8_private.pem -pubout -outform PEM -out rsapss_2048_sha256_spki_public.pem
openssl pkey -in rsapss_2048_sha384_pkcs8_private.pem -pubout -outform PEM -out rsapss_2048_sha384_spki_public.pem
openssl pkey -in rsapss_2048_sha512_pkcs8_private.pem -pubout -outform PEM -out rsapss_2048_sha512_spki_public.pem

### Convert RSA-PSS private key from PKCS#8 PEM to PKCS#8 DER
openssl pkcs8 -nocrypt -in rsapss_2048_sha256_pkcs8_private.pem -topk8 -outform DER -out rsapss_2048_sha256_pkcs8_private.der
openssl pkcs8 -nocrypt -in rsapss_2048_sha384_pkcs8_private.pem -topk8 -outform DER -out rsapss_2048_sha384_pkcs8_private.der
openssl pkcs8 -nocrypt -in rsapss_2048_sha512_pkcs8_private.pem -topk8 -outform DER -out rsapss_2048_sha512_pkcs8_private.der

### Generate RSA-PSS PKCS#8 DER public key from RSA-PSS PKCS#8 PEM private key
openssl pkey -in rsapss_2048_sha256_pkcs8_private.pem -pubout -outform DER -out rsapss_2048_sha256_spki_public.der
openssl pkey -in rsapss_2048_sha384_pkcs8_private.pem -pubout -outform DER -out rsapss_2048_sha384_spki_public.der
openssl pkey -in rsapss_2048_sha512_pkcs8_private.pem -pubout -outform DER -out rsapss_2048_sha512_spki_public.der

### Convert RSA-PSS private key from PKCS#8 DER to PKCS#8 PEM
openssl pkey -inform DER -in rsapss_2048_sha256_private.der -out rsapss_2048_sha256_private.pem

### Convert RSA-PSS private key from PKCS#8 PEM to traditional PKCS#8 PEM
openssl pkcs8 -nocrypt -in rsapss_2048_sha256_pkcs8_private.pem -traditional -out rsapss_2048_pkcs1_private.pem

### Convert RSA-PSS private key from PKCS#8 PEM to PKCS#1 DER
Unknown

### Convert RSA-PSS private key from PKCS#1 PEM to PKCS#8 PEM
openssl pkcs8 -nocrypt -in rsapss_2048_sha256_pkcs1_private.pem -topk8 -out rsapss_2048_sha256_pkcs8_private.pem

## EdDSA keypair

### Create EdDSA PKCS#8 PEM private key
openssl genpkey -algorithm ED25519 -out ED25519_pkcs8_private.pem
openssl genpkey -algorithm ED448 -out ED448_pkcs8_private.pem

### Generate EdDSA PKCS#8 PEM public key from EdDSA PKCS#8 PEM private key
openssl pkey -in ED25519_pkcs8_private.pem -pubout -outform PEM -out ED25519_spki_public.pem
openssl pkey -in ED448_pkcs8_private.pem -pubout -outform PEM -out ED448_spki_public.pem

### Convert EdDSA private key from PKCS#8 PEM to PKCS#8 DER
openssl pkcs8 -nocrypt -in ED25519_pkcs8_private.pem -topk8 -outform DER -out ED25519_pkcs8_private.der
openssl pkcs8 -nocrypt -in ED448_pkcs8_private.pem -topk8 -outform DER -out ED448_pkcs8_private.der

### Generate EdDSA PKCS#8 DER public key from EdDSA PKCS#8 PEM private key
openssl pkey -in ED25519_pkcs8_private.pem -pubout -outform DER -out ED25519_spki_public.der
openssl pkey -in ED448_pkcs8_private.pem -pubout -outform DER -out ED448_spki_public.der

### Convert EdDSA private key from PKCS#8 PEM to PKCS#8 Traditional PEM
openssl pkcs8 -nocrypt -in ED25519_pkcs8_private.pem -traditional -out ED25519_pkcs8_private_traditional.pem
openssl pkcs8 -nocrypt -in ED448_pkcs8_private.pem -traditional -out ED448_pkcs8_private_traditional.pem

## ECx keypair

### Create ECx PKCS#8 PEM private key
openssl genpkey -algorithm X25519 -out X25519_pkcs8_private.pem
openssl genpkey -algorithm X448 -out X448_pkcs8_private.pem

### Generate ECx PKCS#8 PEM public key from ECx PKCS#8 PEM private key
openssl pkey -in X25519_pkcs8_private.pem -pubout -outform PEM -out X25519_spki_public.pem
openssl pkey -in X448_pkcs8_private.pem -pubout -outform PEM -out X448_spki_public.pem

### Convert ECx private key from PKCS#8 PEM to PKCS#8 DER
openssl pkcs8 -nocrypt -in X25519_pkcs8_private.pem -topk8 -outform DER -out X25519_pkcs8_private.der
openssl pkcs8 -nocrypt -in X448_pkcs8_private.pem -topk8 -outform DER -out X448_pkcs8_private.der

### Generate ECx PKCS#8 DER public key from ECx PKCS#8 PEM private key
openssl pkey -in X25519_pkcs8_private.pem -pubout -outform DER -out X25519_spki_public.der
openssl pkey -in X448_pkcs8_private.pem -pubout -outform DER -out X448_spki_public.der

### Convert ECx private key from PKCS#8 PEM to PKCS#8 Traditional PEM
openssl pkcs8 -nocrypt -in X25519_pkcs8_private.pem -traditional -out X25519_pkcs8_private_traditional.pem
openssl pkcs8 -nocrypt -in X448_pkcs8_private.pem -traditional -out X448_pkcs8_private_traditional.pem


PrivateKeyInfo ::= SEQUENCE {
    version             INTEGER,
    privateKeyAlgorithm AlgorithmIdentifier,
    privateKey          OCTET STRING,
    attributes          SET OF Attribute OPTIONAL
}

https://tools.ietf.org/html/rfc8017#appendix-A.1.2
RSAPrivateKey SEQUENCE {
    version           INTEGER,  -- 0
    modulus           INTEGER,  -- n
    publicExponent    INTEGER,  -- e
    privateExponent   INTEGER,  -- d
    prime1            INTEGER,  -- p
    prime2            INTEGER,  -- q
    exponent1         INTEGER,  -- d mod (p-1)
    exponent2         INTEGER,  -- d mod (q-1)
    coefficient       INTEGER,  -- (inverse of q) mod p
    otherPrimeInfos   SEQUENCE OPTIONAL {
        prime             INTEGER,  -- ri
        exponent          INTEGER,  -- di
        coefficient       INTEGER   -- ti
    }
}

SubjectPublicKeyInfo  ::=  SEQUENCE {
   algorithm            AlgorithmIdentifier,
   subjectPublicKey     BIT STRING
}

https://tools.ietf.org/html/rfc8017#appendix-A.1.1
RSAPublicKey ::= SEQUENCE {
    modulus           INTEGER,  -- n
    publicExponent    INTEGER   -- e
}

const key_pair = await window.crypto.subtle.generateKey(
    {
        name: "RSA-OAEP",
        modulusLength: 2048, // 1024, 2048 or 4096
        publicExponent: new Uint8Array([0x01, 0x00, 0x01]),
        hash: {name: "SHA-256"} // SHA-256, SHA-384 or SHA-512
    },
    true,
    ["encrypt", "decrypt"]
);
btoa(String.fromCharCode(...new Uint8Array(await window.crypto.subtle.exportKey("pkcs8", key_pair.privateKey))));
btoa(String.fromCharCode(...new Uint8Array(await window.crypto.subtle.exportKey("spki", result.publicKey))));