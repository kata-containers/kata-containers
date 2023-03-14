# Cosign

Cosign is short for Container Signing, Verification and Storage in an OCI registry.
It aims to make signatures invisible infrastructure.
[Cosign](https://github.com/sigstore/cosign/blob/main/README.md).

## Policy Format

Cosign is verified due to the container's policy configuration file.

A Policy Requirement of Cosign should be like this

```json
{
    "type": "sigstoreSigned",
    "keyData": "<PUBKEY-DATA-IN-BASE64>",
    "keyPath": "<URL-TO-THE-PUBKEY>",
    "signedIdentity": <JSON-OBJECT>,
},
```

Here, 
* The `type` field must be `sigstoreSigned`, showing that this image is signed by `cosign` (`cosign`
is a sub-project for image-signing of [`sigstore`](https://www.sigstore.dev)).
* The `keyData` field includes the pubkey's content in base64.
* The `keyPath` field indicates the pubkey's URL.
* `signedIdentity` includes a JSON object, refer to [signedIdentity](https://github.com/containers/image/blob/main/docs/containers-policy.json.5.md#sigstoreSigned) for details.
Because of the mechanism of Cosign, only `matchRepository` and `exactRepository`
can be used to accept an image.

> **Warning**: Must specify either `keyData` or `keyPath`, but not both.

## Implementation

We wrap the [rust implementation](https://github.com/sigstore/sigstore-rs) for sigstore to fit
in our defined interface for a signing scheme.

## Work Flow

Let's quickly see how Cosign works. For example, there's an image
to be signed, named `example.com/alpine:test`.
So as a reference, its
* `registry`: `example.com`, which is a test registry.
* `repository`: `alpine`
* `tag`: `test`

### Signing

There will be three steps for the signing:
- Sign the `Payload`
- Wrap the `Payload` and signature into an image bundle
- Upload the image bundle to the registry

#### Signature Generate

When we sign this image, we use a cosign-generated key pair.
The digital signature algorithm is ECDSA-P256.
The signed object, i.e. `Payload`, is [simple signing payload](https://github.com/sigstore/cosign/blob/main/specs/SIGNATURE_SPEC.md#simple-signing).
The reference for the signed image will be included in the `Payload`.
However, rather than the full reference, only `registry` and
`repository` are included. `tag` **will not** be included. In this example,
will be like

```json
{
    "critical": {
        "identity": {
            "docker-reference": "example.com/alpine"
        },
        "image": {
            "docker-manifest-digest": "sha256:9b2a28eb47540823042a2ba401386845089bb7b62a9637d55816132c4c3c36eb"
        },
        "type": "cosign container image signature"
    },
    "optional": null
}
```

Due to this, only `matchRepository` and `exactRepository`
can be used to accept an image in Cosign.

How Cosign knows which image in the same repository is signed?
The answer is `docker-manifest-digest`. Every image with different `tag`
will have its own manifest. Due to the different `tag`, the manifest's
sha256 digest is different. We can treat `docker-manifest-digest`
the same as `tag`.

Now, if a client gets the signature and the `Payload`, they
can verify the signature, and then check whether the
`"docker-reference"` in the `Payload` matches the image
to be used.

#### Signature Storage

Now, let's store the signature and the `Payload` in the repository.

There will be a new entry to be stored in the repository.
This new entry includes the signature and the `Payload`.

Due to [OCI Distribution Spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md),
an image is stored in the following way in a image registry:
* There must be a `manifest` file to show the components that make up a container image.
* Each component (e.g., filesystem layer) is stored in a sha256-addressed
blob.

A manifest's format may look like
```json
{
   "schemaVersion": 2,
   "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
   "config": {
      "mediaType": "application/vnd.docker.container.image.v1+json",
      "size": 1472,
      "digest": "sha256:d7d3d98c851ff3a95dbcb70ce09d186c9aaf7e25d48d55c0f99aae360aecfd53"
   },
   "layers": [
      {
         "mediaType": "application/vnd.docker.image.rootfs.diff.tar.gzip",
         "size": 2798806,
         "digest": "sha256:530afca65e2ea04227630ae746e0c85b2bd1a179379cbf2b6501b49c4cab2ccc"
      }
   ]
}
```

Here the layers are the components of the image, and can be addressed
by the `digest` field in the blob.

The manifest can be addressed due to the image's reference.

To avoid extra storage for a signature, Cosign smartly store the signature
and `Payload` in the same registry as the image.

#### "image" format

As the example, when the signature of `example.com/alpine:latest` is stored,
there will be a new "image" to be stored, which "image" contains
the signature and the signed `Payload`. Here the "image" is not a
true image, but some files following the [OCI Distribution Spec](https://github.com/opencontainers/distribution-spec/blob/main/spec.md).

The manifest of the "image" will include the signature encoded in base64, and
have a pointer to the `Payload`. May look like the following

```json
{
    "schemaVersion": 2,
    "mediaType": "application/vnd.oci.image.manifest.v1+json",
    "config": {
        "mediaType": "application/vnd.oci.image.config.v1+json",
        "size": 233,
        "digest": "sha256:eca5f5f99822d62181a93ebe1df31db7eeebdf47c47b0a5515c26adc3abf3226"
    },
    "layers": [
        {
            "mediaType": "application/vnd.dev.cosign.simplesigning.v1+json",
            "size": 238,
            "digest": "sha256:a252c3b3b26bad13a82cb65b252d5f409ab72d7db433ceb7242ebdbcf0fd9889",
            "annotations": {
                "dev.cosignproject.cosign/signature": "MEUCIQDVshyS+xIpdq5YkAAvrACS5wzg3Zsk4n4UTwcgjiwsnwIgfhy+SeAocHKQVFjX4HgljoAOu27g/p4E2x8izHicbeY="
            }
        }
    ]
}
```

Here, `"dev.cosignproject.cosign/signature"` in `"annotations"` is the base64 encoded
signature for the image using ECDSA P256 algorithm.

The layer `sha256:a252c3b3b26bad13a82cb65b252d5f409ab72d7db433ceb7242ebdbcf0fd9889` in
the blob, is the `Payload`. It is exactly the example `Payload` mentioned before.

```json
{
    "critical": {
        "identity": {
            "docker-reference": "example.com/alpine"
        },
        "image": {
            "docker-manifest-digest": "sha256:9b2a28eb47540823042a2ba401386845089bb7b62a9637d55816132c4c3c36eb"
        },
        "type": "cosign container image signature"
    },
    "optional": null
}
```

#### store "image" in the registry

Let's see how the "image" is stored:
- The image to be signed is `example.com/alpine:latest`
- Resolve the image reference tag to the digest `sha256:9b2a28eb47540823042a2ba401386845089bb7b62a9637d55816132c4c3c36eb`. Here
the sha256 digest has the same function as a `tag`. They both can be used
to find a specific image. And this sha256 digest is for example, not true.
- Encoding the digest into a new name `sha256-9b2a28eb47540823042a2ba401386845089bb7b62a9637d55816132c4c3c36eb.sig`. The rule is
    - Replace `:` character with a `-`
    - Append the `.sig` suffix
- Store the "image" in `example.com/alpine:sha256-9b2a28eb47540823042a2ba401386845089bb7b62a9637d55816132c4c3c36eb.sig`

### Verification

When a Policy Requirement with `type` set `sigstoreSigned`, the relative public key will be read
due to `keyPath` or `keyData` field.

Then, follow the steps to verify a Cosign-signed image.
- Download the image's manifest and digest.
- Due to the digest, calculate the reference for the signature "image".
- Download the signature "image".
- Extract signature and `Payload` from the downloaded signature "image".
- Cryptographically verify the signature and the `Payload`.
- Check the `signedIdentity` rules for the reference in it and the
reference in `Payload`.

> **Warning**: Only `matchRepository` and `exactRepository` can be use for Cosign in a `signedIdentity`.
