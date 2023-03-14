This is a simple example program that shows how perform cosign verification.

The program allows also to use annotation, in the same way as `cosign verify -a key=value`
does.

The program prints to the standard output all the Simple Signing objects that
have been successfully verified.

# Key based verification

Create a keypair using the official cosign client:

```console
cosign generate-key-pair
```

Sign a container image:

```console
cosign sign -key cosign.key registry-testing.svc.lan/busybox
```

Verify the image signature using the example program defined under
[`examples/verify`](https://github.com/flavio/sigstore-rs/tree/main/examples/verify):

```console
cargo run --example verify -- \
  -k cosign.pub \
  --rekor-pub-key ~/.sigstore/root/targets/rekor.pub \
  --fulcio-cert fulcio.crt.pem \
  registry-testing.svc.lan/busybox
```
