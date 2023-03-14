# Signature Module for Image-rs

This is the signature module for image-rs. In fact, signature verification
is included in the policy processing.

## How is signature verification working?

Up to now, all signature verification in image-rs happens due to
the image security [policy](https://github.com/confidential-containers/image-rs/blob/main/docs/ccv1_image_security_design.md#policy) 
file.

The format of policy file is detailed [here](../docs/ccv1_image_security_design.md#policy).

Whether the policy requirement is a signing scheme and which signing scheme it is is due to the `type` field in the
[Policy Requirement](https://github.com/containers/image/blob/main/docs/containers-policy.json.5.md#policy-requirements).

Here are some examples for [Simple Signing](src/mechanism/simple/README.md)

```json
{
    "type": "signedBy",
    "keyType": "GPGKeys",
    "keyPath": "/etc/pki/rpm-gpg/RPM-GPG-KEY-redhat-release",
}
```

Here, the `signedBy` type shows that this Policy Requirement
is a Simple Signing requirement. The rest of the 
fields may be different due to different signing scheme. 

For example,
[Simple Signing](src/mechanism/simple/README.md) here requires fields
`keyType`, `keyPath`, `keyData`, and `signedIdentity`.

## How to add new Signing Scheme?

For example, a new scheme called `new-sign-scheme` is to be added.
Here are the positions must be modified.

### `src/mechanism/new-sign-scheme` directory
Create `src/mechanism/new-sign-scheme/mod.rs`

Add `pub mod new_sign_scheme` into  `src/mechanism/mod.rs`

In `src/mechanism/new-sign-scheme/mod.rs`, define the unique parameters 
used in the `policy.json` by `new-sign-scheme`.
For example, a field named `signature-path` should be included, like

```json
// ... A Policy Requirement
{
    "type": "newSignScheme",
    "signature-path": "/keys/123.key",
}
```

Then the parameters' struct can be defined in `src/mechanism/new-sign-scheme/mod.rs`,
like this

```rust
#[derive(Deserialize, Debug, PartialEq, Serialize)]
pub struct NewSignSchemeParameters {
    #[serde(rename = "signature-path")]
    pub signature_path: String,
}
```

Besides, Implement the trait `SignScheme` for `NewSignSchemeParameters`.
```rust
/// The interface of a signing scheme
#[async_trait]
pub trait SignScheme {
    /// Do initialization jobs for this scheme. This may include the following
    /// * preparing runtime directories for storing signatures, configurations, etc.
    /// * gathering necessary files.
    async fn init(&self) -> Result<()>;

    /// Reture a HashMap including a resource's name => file path in fs.
    /// 
    /// Here `resource's name` is the `name` field for a ResourceDescription
    /// in GetResourceRequest.
    /// Please refer to https://github.com/confidential-containers/image-rs/blob/main/docs/ccv1_image_security_design.md#get-resource-service
    /// for more information about the `GetResourceRequest`.
    /// 
    /// This function will be called by `Agent`, to get the manifest
    /// of all the resources to be gathered from kbs. The gathering
    /// operation will happen after `init_scheme()`, to prepare necessary
    /// resources. The HashMap here uses &str rather than String,
    /// which encourages developer of new signing schemes to define
    /// const &str for these information.
    fn resource_manifest(&self) -> HashMap<&str, &str>;

    /// Judge whether an image is allowed by this SignScheme.
    async fn allows_image(&self, image: &mut Image) -> Result<()>;
}
```

The basic architecture for signature verification is the following figure:

```plaintext
                +-------------+
                | ImageClient |
                +-------------+
                       |
                       | allows_image(image_url,  image_digest, aa_kbc_params)
                       v
              +-----------------+   gRPC Client
              |      Agent      | ---------------> KBS
              +-----------------+    Access
                       |
                       |
      +----------------+-----------------+
      |                                  |
      |                                  |
+-----+-------+                   +------+------+
|   Signing   |                   |   Signing   |
|    Scheme   |                   |    Scheme   |
|   Module 1  |                   |   Module 2  |
+-------------+                   +-------------+
```

When a `ImageClient` need to pull an image, it will call
`allows_image`. `allows_image` will instanialize
a `Agent` to handle Policy Requirements if needed.
The `Agent` can communicate with KBS to retrieve needed
resources. Also, it can call specific signing scheme verification
module to verify a signature due to the Policy Requirement in
`policy.json`. So there must be three interfaces for a signing
scheme to implement:
1. `init()`: This function is called for every signing scheme
policy requirement, so it should be **idempotent**.
It can do initialization work for this scheme. This may include the following
* preparing runtime directories for storing signatures, configurations, etc.
* gathering necessary files.

2. `resource_manifest()`: This function will tell the `Agent`
which resources it need to retrieve from the kbs. The return value should be
a HashMap. The key of the HashMap is the `name` field for a ResourceDescription
in GetResourceRequest. The value is the file path that the returned resource will be
written into after retrieving the resource. Refer to 
[get-resource-service](https://github.com/confidential-containers/image-rs/blob/main/docs/ccv1_image_security_design.md#get-resource-service)
for more information about GetResourceRequest. This function will be called
on every check for a Policy Requirement of this signing scheme.

3. `allows_image()`: This function will do the verification. This
function will be called on every check for a Policy Requirement of this signing scheme.

### `src/policy/policy_requirement.rs`

Because every signing scheme for an image is recorded in
a policy requirement, we should add here.
Add a new enum value `NewSignScheme` for `PolicyReqType` in 

```rust
pub enum PolicyReqType {
    ...

    /// Signed by Simple Signing
    #[serde(rename = "signedBy")]
    SimpleSigning(SimpleParameters),

    /// Signed by new sign scheme
    #[serde(rename = "newSignScheme")]
    NewSignScheme(NewSignSchemeParameters),
}
```

Here, `NewSignSchemeParameters` must be inside the enum.

Add new arm in the `allows_image` function. 
```rust
pub async fn allows_image(&self, image: &mut Image) -> Result<()> {
    match self {
        PolicyReqType::Accept => Ok(()),
        PolicyReqType::Reject => Err(anyhow!(r#"The policy is "reject""#)),
        PolicyReqType::SimpleSigning(inner) => inner.allows_image(image).await,
        PolicyReqType::NewSignScheme(inner) => inner.allows_image(image).await,
    }
}
```

Add new arm in the `try_into_sign_scheme` function.
```rust
pub fn try_into_sign_scheme(&self) -> Option<&dyn SignScheme> {
    match self {
        PolicyReqType::SimpleSigning(scheme) => Some(scheme as &dyn SignScheme),
        PolicyReqType::NewSignScheme(scheme) => Some(scheme as &dyn SignScheme),
        _ => None,
    }
}
```

## Supported Signatures

|Sign Scheme|Readme|
|---|---|
|[Simple Signing](src/mechanism/simple)| [README](src/mechanism/simple/README.md) |
|[Cosign](src/mechanism/cosign)| [README](src/mechanism/cosign/README.md)|