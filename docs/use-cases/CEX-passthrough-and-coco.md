# Using IBM Crypto Express with Confidential Containers

On IBM Z (s390x), IBM Crypto Express (CEX) hardware security modules (HSM) can be passed through to virtual guests.
This VFIO pass-through is domain-wise, i.e. guests can securely share one physical card.
For the Accelerator and Enterprise PKCS #11 (EP11) modes of CEX, on IBM z16 and up, pass-through is also supported when using the IBM Secure Execution trusted execution environment.
To maintain confidentiality when using EP11 within Secure Execution, additional steps are required.
When using Secure Execution within Kata Containers, some of these steps are managed by the Kata agent, but preparation is required to make pass-through work.
The Kata agent will expect required confidential information at runtime via [Confidential Data Hub](https://github.com/confidential-containers/guest-components/tree/main/confidential-data-hub) from Confidential Containers, and this guide assumes Confidential Containers components as a means of secret provisioning.

At the time of writing, devices for trusted execution environments are only supported via the `--device` option of e.g. `ctr`, `docker`, or `podman`, but **not** via Kubernetes.
Refer to [KEP 4113](https://github.com/kubernetes/enhancements/pull/4113) for details.

Using a CEX card in Accelerator mode is much simpler and does not require the steps below.
To do so, prepare [Kata for Secure Execution](../how-to/how-to-run-kata-containers-with-SE-VMs.md), set `vfio_mode = "vfio"` and `cold_plug_vfio = "bridge-port"` in the Kata `configuration.toml` file and use a [mediated device](../../src/runtime/virtcontainers/README.md#how-to-pass-a-device-using-vfio-ap-passthrough) similar to operating without Secure Execution.
The Kata agent will do the [Secure Execution bind](https://www.ibm.com/docs/en/linux-on-systems?topic=adapters-accelerator-mode) automatically.

## Prerequisites

- A host kernel that supports adjunct processor (AP) pass-through with Secure Execution. [Official support](https://www.ibm.com/docs/en/linux-on-systems?topic=restrictions-required-software) exists as of Ubuntu 24.04, RHEL 8.10 and 9.4, and SLES 15 SP6.
- An EP11 domain with a master key set up. In this process, you will need the master key verification pattern (MKVP) [1].
- A [mediated device](../../src/runtime/virtcontainers/README.md#how-to-pass-a-device-using-vfio-ap-passthrough), created from this domain, to pass through.
- Working [Kata Containers with Secure Execution](../how-to/how-to-run-kata-containers-with-SE-VMs.md).
- Working access to a [key broker service (KBS) with the IBM Secure Execution verifier](https://github.com/confidential-containers/trustee/blob/main/deps/verifier/src/se/README.md) from a Kata container. The provided Secure Execution header must match the Kata guest image and a policy to allow the appropriate secrets for this guest must be set up.
- In Kata's `configuration.toml`, set `vfio_mode = "vfio"` and `cold_plug_vfio = "bridge-port"`

## Prepare an association secret

An EP11 Secure Execution workload requires an [association secret](https://www.ibm.com/docs/en/linux-on-systems?topic=adapters-ep11-mode) to be inserted in the guest and associated with the adjunct processor (AP) queue.
In Kata Containers, this secret must be created and made available via Trustee, whereas the Kata agent performs the actual secret insertion and association.
On a trusted system, to create an association secret using the host key document (HKD) `z16.crt`, a guest header `hdr.bin`, a CA certificate `DigiCertCA.crt`, an IBM signing key `ibm-z-host-key-signing-gen2.crt`, and let the command create a random association secret that is named `my secret` and save this random association secret to `my_random_secret`, run:

```
[trusted]# pvsecret create -k z16.crt --hdr hdr.bin -o my_addsecreq \
  --crt DigiCertCA.crt --crt ibm-z-host-key-signing-gen2.crt \
  association "my secret" --output-secret my_random_secret
```

using `pvsecret` from the [s390-tools](https://github.com/ibm-s390-linux/s390-tools) suite.
`hdr.bin` **must** be the Secure Execution header matching the Kata guest image, i.e. the one also provided to Trustee.
This command saves the add-secret request itself to `my_addsecreq`, and information on the secret, including the secret ID, to `my_secret.yaml`.
This secret ID must be provided alongside the secret.
Write it to `my_addsecid` with or without leading `0x` or, using `yq`:

```
[trusted]# yq ".id" my_secret.yaml > my_addsecid
```

## Provision the association secret with Trustee

The secret and secret ID must be provided via Trustee with respect to the MKVP.
The paths where the Kata agent will expect this info are `vfio_ap/${mkvp}/secret` and `vfio_ap/${mkvp}/secret_id`, where `$mkvp` is the first 16 bytes (32 hex numbers) without leading `0x` of the MKVP.

For example, if your MKVPs read [1] as

```
WK CUR: valid 0xdb3c3b3c3f097dd55ec7eb0e7fdbcb933b773619640a1a75a9161cec00000000
WK NEW: empty -
```

use `db3c3b3c3f097dd55ec7eb0e7fdbcb93` in the provision for Trustee.
With a KBS running at `127.0.0.1:8080`, to store the secret and ID created above in the KBS with the authentication key `kbs.key` and this MKVP, run:

```
[trusted]# kbs-client --url http://127.0.0.1:8080 config \
  --auth-private-key kbs.key set-resource \
  --path vfio_ap/db3c3b3c3f097dd55ec7eb0e7fdbcb93/secret \
  --resource-file my_addsecreq
[trusted]# kbs-client --url http://127.0.0.1:8080 config \
  --auth-private-key kbs.key set-resource \
  --path vfio_ap/db3c3b3c3f097dd55ec7eb0e7fdbcb93/secret_id \
  --resource-file my_addsecid
```

## Run the workload

Assuming the mediated device exists at `/dev/vfio/0`, run e.g.

```
[host]# docker run --rm --runtime io.containerd.run.kata.v2 --device /dev/vfio/0 -it ubuntu
```

If you have [s390-tools](https://github.com/ibm-s390-linux/s390-tools) available in the container, you can see the available CEX domains including Secure Execution info using `lszcrypt -V`:

```
[container]# lszcrypt -V
CARD.DOM TYPE  MODE        STATUS     REQUESTS  PENDING HWTYPE QDEPTH FUNCTIONS  DRIVER      SESTAT     
--------------------------------------------------------------------------------------------------------
03       CEX8P EP11-Coproc online            2        0     14     08 -----XN-F- cex4card    -          
03.0041  CEX8P EP11-Coproc online            2        0     14     08 -----XN-F- cex4queue   usable     
```

---

[1] If you have access to the host, the MKVP can be read at `/sys/bus/ap/card${cardno}/${apqn}/mkvps`, where `${cardno}` is the the two-digit hexadecimal identification for the card, and `${apqn}` is the APQN of the domain you want to pass, e.g. `card03/03.0041` for the the domain 0x41 on card 3.
This information is only readable when card and domain are not yet masked for use with VFIO.
If you do not have access to the host, you should receive the MKVP from your HSM domain administrator.
