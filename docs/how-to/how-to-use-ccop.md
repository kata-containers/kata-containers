# How to Use cc-op

## What is cc-op

Confidential Container Operations (`cc-op`) provides authenticated and encrypted support for the `exec` and `cp` operations in Confidential Containers (CoCo) environments. It establishes an end-to-end cryptographic channel between the workload owner and the `kata-agent` inside the confidential guest VM, preventing the host OS, container runtime, and Kubernetes control plane from observing or tampering with operational traffic.

For the full design, see the [cc-op Design Document](../design/ccop.md).

---

## Prerequisites

Before enabling cc-op you will need:

- A Kubernetes cluster with Confidential Containers support (AMD SEV, Intel TDX, or compatible TEE).
- A `kata-agent` image built with the `cc-op` functionality. The feature can be enabled when building the `rootfs` by running the [rootfs.sh](../../tools/osbuilder/rootfs-builder/rootfs.sh) script  with`CC_OP=yes`, see the [Developer Guide](../Developer-Guide.md).
- [A Trustee / Key Broker Service (KBS)](https://github.com/salmanyam/trustee/blob/2b011e9ba0e2bf50533be66e6e58aa77071558d4/kbs/docs/plugins/credgen.md) that can attest the guest and provision key material.

- The `ccop` client tool, released in a separate [repository](https://github.com/IBM/ccop).
- `openssl` available on the workload owner's workstation.

---

## Step 1 — Generate a Key Pair

The workload owner generates a P-256 EC key pair and a self-signed X.509 certificate. The private key stays on the owner's workstation and is used to sign (and authenticate) every cc-op operation. The certificate (containing the corresponding public key) is embedded in the pod's initdata so the `kata-agent` can verify the owner's identity.

### Create a certificate configuration file

Save the following as `openssl.cnf`:

```ini
[ req ]
default_bits       = 256
default_md         = sha256
prompt             = no
distinguished_name = dn
x509_extensions    = v3_req

[ dn ]
CN = ccop-workload-owner

[ v3_req ]
keyUsage         = digitalSignature
extendedKeyUsage = clientAuth
subjectKeyIdentifier = hash
```

### Generate the key pair and self-signed certificate

```
# Generate the EC private key
$ openssl ecparam -name prime256v1 -genkey -noout -out key.priv

# Derive the public key (optional; kept for reference)
$ openssl ec -in key.priv -pubout -out key.pub

# Create a certificate signing request
$ openssl req -new -key key.priv -out cert.csr -config openssl.cnf

# Self-sign the certificate (valid for 365 days)
$ openssl x509 -req -in cert.csr -signkey key.priv -out cert.crt -days 365 \
    -extensions v3_req -extfile openssl.cnf
```

Keep `key.priv` secret. `cert.crt` is the public artifact embedded in the initdata.

---

## Step 2 — Set Environment Variables

Before building the initdata, export the KBS URL and the path to the KBS admin bearer token. These are used in the shell commands that follow.

```
$ export KBS_URL=<KBS_SERVER_URL>          # e.g. http://1.2.3.4:8080
$ export KBS_ADMIN_TOKEN=/path/to/admin-token
```

Also set the pod name and namespace used in the pod configuration deployed in Step 3 (this example uses `ccop-demo` and `default`):

```
$ export PODNAME=ccop-demo
$ export NAMESPACE=default
```

---

## Step 3 — Build the Initdata

The initdata is delivered to the guest VM at boot. It contains:

- `ccop.toml` — the cc-op configuration, including `cert.crt`.
- `policy.rego` — the Kata Agent Policy. When cc-op is active, the agent skips the policy check for `ExecProcessRequest`, so this setting has no effect; only the cc-op client (which wraps the real command in an HPKE-encrypted payload) can succeed. The setting matters only if the agent image supports the `cc-op` feature but cc-op is not active (no `ccop.toml` in the initdata); in that case `ExecProcessRequest := false` blocks `kubectl exec` as in a baseline agent.
- `aa.toml` and `cdh.toml` — Attestation Agent and Confidential Data Hub configuration, pointing at `KBS_URL`.
- `name` and `ns` — the pod's name and namespace.

> [!NOTE]
> The initdata must specify the pod's `name` and `ns` (namespace). The credential generator plugin (`credgen`) uses them to verify that a credential request comes from the pod that owns them. They must match the pod's `metadata.name` and namespace.

Read the certificate into a shell variable, then create `initdata.toml`:

```
$ CERTDATA=$(cat cert.crt)

$ tee initdata.toml <<EOF
version = "0.1.0"
algorithm = "sha256"
[data]
"policy.rego" = '''
package agent_policy

default AddARPNeighborsRequest := true
default AddSwapRequest := true
default CloseStdinRequest := true
default CopyFileRequest := true
default CreateContainerRequest := true
default CreateSandboxRequest := true
default DestroySandboxRequest := true

# Block exec by default
default ExecProcessRequest := false

default GetMetricsRequest := true
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default ListInterfacesRequest := true
default ListRoutesRequest := true
default MemHotplugByProbeRequest := true
default OnlineCPUMemRequest := true
default PauseContainerRequest := true
default PullImageRequest := true
default ReadStreamRequest := true
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default ReseedRandomDevRequest := true
default ResumeContainerRequest := true
default SetGuestDateTimeRequest := true
default SetPolicyRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StartTracingRequest := true
default StatsContainerRequest := true
default StopTracingRequest := true
default TtyWinResizeRequest := true
default UpdateContainerRequest := true
default UpdateEphemeralMountsRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := true
'''

"ccop.toml" = '''
auth_cert = """
${CERTDATA}
"""
'''

"aa.toml" = '''
[token_configs]
[token_configs.kbs]
url = "${KBS_URL}"
'''

"cdh.toml" = '''
[kbc]
name = "cc_kbc"
url = "${KBS_URL}"
'''

"name" = "${PODNAME}"
"ns"   = "${NAMESPACE}"
EOF
```

Compress and base64-encode the initdata, then embed it in the pod annotation `io.katacontainers.config.hypervisor.cc_init_data`:

```
$ INITDATA=$(cat initdata.toml | gzip | base64 -w0)

$ tee pod.yaml <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: ${PODNAME}
  namespace: ${NAMESPACE}
  annotations:
    io.containerd.cri.runtime-handler: kata-qemu-snp-ccop
    io.katacontainers.config.hypervisor.cc_init_data: ${INITDATA}
spec:
  runtimeClassName: kata-qemu-snp-ccop
  containers:
    - name: workload
      image: quay.io/prometheus/busybox:latest
      imagePullPolicy: Always
      command: ["sleep", "infinity"]
  restartPolicy: Never
EOF
```

---

## Step 4 — Deploy the Workload

```
$ kubectl apply -f pod.yaml
```

Wait for the pod to reach the `Running` state:

```
$ kubectl wait pod/ccop-demo --for=condition=Ready --timeout=120s
```

At startup, the `kata-agent` reads `ccop.toml` from the initdata, parses the owner's certificate, and stores the public verifying key. From this point on, any `ExecProcess` request must carry a valid HPKE-encrypted payload signed with the owner's private key.

> **Note:** pod creation fails if the guest VM cannot retrieve its credential from Trustee at boot. This also happens if the `name`/`ns` in the initdata don't match the pod's actual name and namespace. Confirm the pod reaches `Running` before proceeding.

---

## Step 5 — Retrieve the Agent's Public Key Certificate

At boot, the guest VM retrieves the agent's **private key** from Trustee (via the Attestation Agent and CDH). The corresponding **public key certificate** must be fetched by the workload owner before running any `ccop` operation.

Fetch the agent's public key certificate and save it to `./receiver.crt`:

```
$ ccop fetch --url "${KBS_URL}" --admin-token-file "${KBS_ADMIN_TOKEN}" \
    --namespace "${NAMESPACE}" --pod "${PODNAME}" \
    secret_name=ccop_p256 secret_type=cert kind=self_signed_ca algorithm=ec_p256 --out ./receiver.crt
```

> **Note:** This step must be completed before running any `ccop exec` or `ccop cp` command. The certificate in `./receiver.crt` is the agent's public key and is distinct from `cert.crt` (the workload owner's certificate embedded in the initdata).

---

## Step 6 — Run cc-op Operations

### Non-interactive exec

```
$ ccop exec ccop-demo --privkey ./key.priv --pubkeycert ./receiver.crt -- ls /
```

### Interactive shell

```
$ ccop exec ccop-demo -it --privkey ./key.priv --pubkeycert ./receiver.crt -- /bin/sh
```
<!--

### Secure copy

Copy a file out of the container:

```
ccop cp ccop-demo:/etc/hostname ./hostname --privkey ./key.priv --pubkeycert ./receiver.crt
```

Copy a file into the container:

```
ccop cp ./local-file ccop-demo:/tmp/ --privkey ./key.priv --pubkeycert ./receiver.crt
```

The `ccop` tool wraps `kubectl` internally, constructing the HPKE-encrypted payload and transparently en/decrypting all IO streams. From the user's perspective the experience is nearly identical to plain `kubectl exec` and `kubectl cp`.
 -->

The `ccop` tool wraps `kubectl` internally, constructing the HPKE-encrypted payload and transparently en/decrypting all IO streams. From the user's perspective the experience is nearly identical to plain `kubectl exec`.

---

## Persistent Configuration

Once you have verified the basic workflow above, you can set up a persistent configuration to avoid repeating `--privkey` and `--pubkeycert` on every command.

### Create the configuration directory

```
$ ccop config -c
```

This creates `~/.ccop/` with a `config.toml` file and a `cache/` sub-directory that holds per-pod artifacts (e.g., `receiver.crt`).

### Set your key paths

Edit `~/.ccop/config.toml` to point at your key pair:

```toml
UserPublicKeyCert = "/path/to/cert.crt"
UserPrivateKey    = "/path/to/key.priv"
```

### Cache a pod's certificate

Run `ccop config` once for the pod. It extracts the KBS URL from the running pod's initdata, retrieves the public key certificate from KBS, and stores it in `~/.ccop/cache/`:

```
$ ccop config ccop-demo --admin-token-file "${KBS_ADMIN_TOKEN}"
```

With both the key paths and the certificate cached, all key flags can be omitted:

```
$ ccop exec ccop-demo -- ls /
```

```
$ ccop exec ccop-demo -it -- /bin/sh
```

### Inspect cc-op status across all running pods

```
$ ccop config -l
```

### Check a specific pod's cc-op status

```
$ ccop config -l ccop-demo
```

---

## Notes

### Plain `kubectl exec` is intentionally blocked

When cc-op is enabled, the agent does not consult the `ExecProcessRequest` policy setting. A direct `kubectl exec` call is rejected because it carries no HPKE-encrypted payload, with an error similar to:

```
error: … rpc error: … missing arguments: --payload
```

This is expected behavior. Only the `ccop` client, which wraps the command in an HPKE-encrypted payload, can successfully execute `exec` and `cp` operations on a cc-op–enabled pod.

### Pods without cc-op are unaffected

If `ccop.toml` is absent from a pod's initdata, the `kata-agent` behaves exactly like a standard agent with no cc-op code paths active. A single `kata-agent` image can serve both cc-op–enabled and conventional pods.

### The `ccop` client tool is in a separate repository

The `ccop` binary is not part of the `kata-containers` or `guest-components` repositories. Refer to its release page for installation instructions and version compatibility information.
