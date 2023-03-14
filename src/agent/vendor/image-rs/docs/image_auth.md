# CCv1 Image Auth

## Backgrounds

When tenants want to use an image from a private registry to deploy a confidential container, the `image-rs` module inside the pod needs to pulling the image from this private registry.
Here, a private registry means that the `image-rs` module needs credentials to show the privilege of this registry.
Different OCI registry supports different types of authentication, however the upstream crate [`oci_distribution`](https://github.com/krustlet/oci-distribution/tree/main) of `image-rs` only supports the following two types:
- `Anonymous`: Pulling an image from a public registry without any authentication.
In this way, most of the public images can be pulled, except images from private registries.
Besides, registries like `dockerhub` can limit an image to (~100 IIRC) requests a month.
- `Basic`: Using `username` and `password` as a _Credential_ of this registry. In this way, no limitions mentioned occur.

[Containers](https://github.com/containers) has a [well-defined mechinism](https://github.com/containers/image/blob/main/docs/containers-auth.json.5.md) to distribute _Credentials_ for registries when pulling images. The work process, considerations will be detailed in the following.

### Related Links

- [Need of pulling images from private repositories](https://github.com/kata-containers/kata-containers/issues/4601)
- [Syntax of registry authentication file](https://github.com/containers/image/blob/main/docs/containers-auth.json.5.md)

## Goals

The total goal is to support pulling images using authentication information, s.t. credentials of specific registries.
Below this, the sub-goals are
- Support [`registry authentication file`](https://github.com/containers/image/blob/main/docs/containers-auth.json.5.md) mechinism when pulling images.
- Support to getting [`registry authentication file`](https://github.com/containers/image/blob/main/docs/containers-auth.json.5.md) from the KBS when it is enabled and not found in the specific filesystem path.

## Work Process

This section will clarify the authorization and authentication process of a registry.
First let us make some assumptions to help.

### Basic Model

- User `Alice` wants to deploy an image `busybox` inside a confidential VM, s.t. pod. Let's say the host of the confidential VM is `Bob`, who can represent CSP.
- The image is from a private registry `private-registry.org` which needs credential to login.
- The image is pulled inside the VM to prevent `Bob` from knowing the credential.
- `Alice` has the credential of this registry, s.t. `username:Alice`, `password:pswd`
- The pod of `Bob` side to deploy confidential container is exclusive by `Alice`.

### Authorization

`Alice` should first edit an `auth.json` following [the format](https://github.com/containers/image/blob/main/docs/containers-auth.json.5.md#format). In this story, for example, the `auth.json` can be

```json
{
	"auths": {
		"private-registry.org": {
			"auth": "QWxpY2U6cHN3ZAo="
		},
	}
}
```

Here `QWxpY2U6cHN3ZAo=` is the base64-encoded `Alice:pswd`.
All the credentials have a common format that `<username>:<password>`.

All above happens on the `Alice`'s side, so it is exactly safe and unknown to `Bob`.

### Authentication

When `Alice` wants to deploy the image `private-registry.org/busybox` inside the confidential VM of `Bob`'s side, `image-rs` needs to pull the image.

1. Firstly, it checks whether there is an file `auth.json`, if yes, do step 3.
2. Fetch the `auth.json` via `GetResource` gRPC of Attestation Agent inside the same Pod. More details about `GetResource` and Attestation Agent please refer to [get-resource-service](ccv1_image_security_design.md#get-resource-service) and [attestation-agent](ccv1_image_security_design.md#attestation-agent).
3. The `image-rs` module then match the entries inside the `auth.json` using longest prefix match. In this story, it will do:
* Firstly look for the entry `private-registry.org/busybox`, but there is not.
* Then search for the entry `private-registry.org`, and the matched value will be `"auth": "QWxpY2U6cHN3ZAo="`.
In other scenarios, if no entries is matched, use `Anonymous` authentication in step 4.
4. The `image-rs` module use the credential matched to pull the image.

## Considerations

### Where to store the `auth.json`?

The same as `policy.json` in [image security](ccv1_image_security_design.md#policy).
We put the file in `/run/image-security/auth.json` inside the Pod, because the `/run` directory is mounted in `tmpfs`, which is located in the encrypted memory protected by HW-TEE.

### When `image-rs` should check and get `auth.json` if not present?

Every time a pulling operation is performed, check and get `auth.json` if not find. This way is _lazy_ and _in need_.
This way is preferred, because it has low start-up latency of `image-rs`.

### How `image-rs` knows the related `auth.json`'s `ResourceDescriptor`?

There are two steps:

- **Step 1** We set a hard-coded `ResourceDescriptor` for `auth.json` in the both `image-rs` side and [`attestation-agent`](https://github.com/confidential-containers/attestation-agent/blob/main/src/kbc_modules/mod.rs#L123) side. In this way we can test the ability of `auth.json`.

- **Step 2** We provide a configuration for `PullClient` of the `image-rs` crate, which will indicates what the `ResourceDescriptor` is.
In this way, not only `auth.json` but also `policy.json` and other resources `image-rs` needs can be customized.

Step 1 is for short term, and in future we will gradually implement Step 2. At that time, this document should be modified.