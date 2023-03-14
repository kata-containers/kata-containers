# Integration Test for Image-rs

This integration test has two main sub-type test sets:
* Image decryption using [ocicrypt-rs](https://github.com/confidential-containers/ocicrypt-rs)
* Image signature verification.

And both of test set will use the following key broker client:
* `Offline-fs-kbc`

## Image Decryption

Implemented in `image_decryption.rs`.

Image decryption will cover `Offline-fs-kbc`:
* `Offline-fs-kbc` uses `docker.io/xynnn007/busybox:encrypted`

Each test suite will follow these steps:

* Pull manifest of the image without verification of signature.
* Pull layers of the mentioned image.
* Ocicrypt-rs will ask the Attestation-Agent to decrypt the Layer Encryption Key (LEK for short), which is 
encrypted using Key Encryption Key (KEK for short). KEK is stored in KBS.
* Ocicrypt-rs decrypt the layers using LEK. Finish the image pulling.

Different KBCs use different protocol format, so different KBSs are needed to
encrypt the images. To genetate KBS encrypted image, please refer to the following link:

* [Using Offline-fs-kbs](https://github.com/confidential-containers/attestation-agent/tree/main/sample_keyprovider/src/enc_mods/offline_fs_kbs/README.md)

## Image Signature Verification

Implemented in `signature_verification.rs`.

Image Signature Verification includes the following four
tests illustrated in 
<https://github.com/confidential-containers/image-rs/issues/43>,
s.t.

| |signed image|unsigned image|
|---|---|---|
|protected registry|protected_signed_allow, protected_signed_deny|protected_unsigned_deny|
|unprotected registry|-|unprotected_unsigned_allow|

Here
* `protected_signed_allow`: Allow pulling image from a protected registry, including `Simple Signing` and `Cosign`
* `protected_signed_deny`: Deny pulling image from a protected registry, including 
    * `Simple Signing` with an unknown signature
    * `Cosign` with a wrong public key
* `protected_unsigned_deny`: Deny pulling an unsigned image from a protected registry
* `unprotected_unsigned_allow`: Allow pulling an unsigned image from a unprotected registry

In `signature_verification.rs`, the tests are organized due different kinds
of KBCs, which means for each given KBC, all four tests mentioned will be
covered.

## Registry Credential Retrievement

Implemented in `credential.rs`.

Registry Credential Retrievement will do the following steps to test this feature.
- Get `auth.json` from the Attestation Agent. The test KBC is `Offline-Fs-Kbc`.
- Try to pull images from private registry using the matched credential in `auth.json`

The test cases are
| Image Reference | Related credential|
|---|---|
|`docker.io/liudalibj/private-busy-box` |`bGl1ZGFsaWJqOlBhc3N3MHJkIXFhego=`|
|`quay.io/liudalibj/private-busy-box`|`bGl1ZGFsaWJqOlBhc3N3MHJkIXFhego=`|