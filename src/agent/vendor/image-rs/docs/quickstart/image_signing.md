# Image signing with cosign

This document includes the following:
* Guide to generate a cosign-signed image.
* Guide to config image security policy to enable signature verification of the image.
* Guide to distribute the public key and image security policy via offline-fs-kbs.

## Signing the encrypted/unencrypted image and enable signature verification when running the workload

When an image is being pulled from a container registry, [policy requirements](https://github.com/containers/image/blob/main/docs/containers-policy.json.5.md#policy-requirements)
can determined whether the image can be accepted.

The requirements can be:

* Directly reject
* Unconditional accept
* Signature verification required

This section shows how to sign an image, and enable signature verification of specific repository.

### Signing an image with cosign

Both encrypted and unencrypted image can be signed.

We need to install `cosign` to sign images. Detailed work process behind `cosign` can be found in [this doc](https://github.com/confidential-containers/image-rs/tree/main/signature/src/mechanism/cosign).
Follow [the guide here](https://github.com/sigstore/cosign#installation) to install `cosign`:

After installing `cosign`, we need to generate a key pair to sign images and verify related signatures.

```
# Generate signing key pair
cosign generate-key-pair
```

After typing a password twice, a key pair will be generated, s.t. private key `cosign.key` and public key `cosign.pub`. 
Here, the password is used to encrypt the private key. 
When we use the private key to sign an image, the password is required. Of cource, the password can be empty.

Suppose there is already an image prepared to be signed named `example.org/test`:

```
# sudo docker images
REPOSITORY            TAG                  IMAGE ID       CREATED         SIZE
example.org/test      latest               ff4a8eb070e1   2 weeks ago     1.24MB
```
Now let us sign this image with the newly generated private key

```
cosign sign --key cosign.key [REGISTRY_URL]:cosign-signed
```

Here, `cosign.key` can be replaced with any cosign-generated private key.

Now the image is signed by cosign, and the signature is pushed to the same repository as the image.

To learn more about cosign, please refer to [the github repository](https://github.com/sigstore/cosign).

### Enable cosign image signature verification and retrieve public key via KBC channel

Take [offline file system key broker](https://github.com/confidential-containers/attestation-agent/tree/64c12fbecfe90ba974d5fe4896bf997308df298d/src/kbc_modules/offline_fs_kbc) (Offline-Fs-KBC for short) for example.

#### Prepare Attestation-Agent and Offline-Fs-KBC

Clone the repository.

```
git clone https://github.com/confidential-containers/attestation-agent.git
```

Build Offline-Fs-KBC & AA
```
cd attestation-agent
make KBC=offline_fs_kbc

install_dir=/path/to/be/installed
make install
```

#### Add Offline-Fs-KBC resources

All signature verification rules are defined in a `policy.json`. But before we work on
the `policy.json`, there are a few things to be clarified:
* The `policy.json` is provided by the relying party, so we will use Offline-Fs-KBC to provide
this secret file. The channel is secure.
* The verification public key is also retrieved via KBC secure channel.

So the next steps will firstly add the two resources (`policy.json` and public key) to the
Offline-Fs-KBC resources.

Let's continue with the image `example.org/test`, and enable security strategy (including signature verification).

Firstly edit an `policy.json` like

```
{
    "default": [{"type": "reject"}], 
    "transports": {
        "docker": {
            "example.org": [
                {
                    "type": "sigstoreSigned",
                    "keyPath": "/run/image-security/cosign/cosign.pub"
                }
            ]
        }
    }
}
```

Here, `"keyPath"` refers to the path to the `cosign.pub` public key. When verification
occurs, firstly the path is checked to see whether there is such a file. If not, the
key will be retrieved via the KBC secure channel.

let's calculate base64-encoded values for the two resources
```bash
cat /path/to/policy.json | base64 --wrap=0
cat /path/to/cosign.pub | base64 --wrap=0
```

Let's return to the dir of `attestation-agent` and edit the resources.
Replace the values of `Policy` and `Cosign Key` in `src/kbc_modules/offline_fs_kbc/aa-offline_fs_kbc-resources.json`
to the related base64 code generated. 

Then copy the newly edited `aa-offline_fs_kbc-resources.json` to the target dir.
```
cd src/kbc_modules/offline_fs_kbc/
cp aa-offline_fs_kbc-resources.json /etc/aa-offline_fs_kbc-resources.json
cp aa-offline_fs_kbc-keys.json /etc/aa-offline_fs_kbc-keys.json
```

In this way, when the images from `"example.org"` is being pulled,
the signature will be verified using the public key of path `"/run/image-security/cosign/cosign.pub"`.

Now let's start the AA with Offline-Fs-KBC

```
attestation-agent --keyprovider_sock 127.0.0.1:50000 --getresource_sock 127.0.0.1:50001
```

Now the attestation-agent can response with the correct resources.
