# kata-annotation-webhook

A Kubernetes **mutating admission webhook** that injects Kata Containers–specific annotations into Pods that use the Kata runtime. It enables the Kata runtime to correctly handle `emptyDir` volumes (including size limits and medium) when the pod is created.

## Overview

When a Pod is scheduled with a Kata RuntimeClass, the webhook:

1. Intercepts the Pod **CREATE** request (before the pod is persisted).
2. If the Pod’s `spec.runtimeClassName` **starts with** `kata` (e.g. `kata`, `kata-qemu`, `kata-clh`, `kata-fc`), it collects all `emptyDir` volumes from the Pod spec (name, medium, `sizeLimit`).
3. Injects a single JSON annotation on the Pod (`io.katacontainers.sandbox.volumes.emptydir`) with this metadata.

The Kata runtime and agent then read this annotation when setting up the sandbox and apply the correct size limits and options for each emptyDir volume.

## How It Works

- **Trigger**: Only Pods whose `spec.runtimeClassName` starts with `kata` are mutated (e.g. `kata`, `kata-qemu`, `kata-clh`). 
- **Mutation**: For those Pods, the webhook adds one annotation whose value is a JSON-serialized list of emptyDir definitions (name, medium, size_limit).
- **Side effects**: The webhook declares `sideEffects: None`; it does not create external resources or change cluster state beyond the Pod object.

## Prerequisites

- A Kubernetes cluster (1.16+ with `admissionregistration.k8s.io/v1`).
- A [RuntimeClass](https://kubernetes.io/docs/concepts/containers/runtime-class/) for Kata (e.g. `kata`).
- `kubectl` and `openssl` for deployment and certificate generation.

## Building

From the repository root or from `src/tools/kata-annotation-webhook`:

```bash
make build
```

This produces `bin/kata-annotation-webhook`. The binary depends on the `src/runtime` module (see `go.mod`).

To build the container image:

```bash
make image
```

This builds `quay.io/kata-containers/kata-annotation-webhook` (see the Makefile and Dockerfile for details).

## Deployment

### 1. Generate TLS certificates and webhook registration

The webhook is called by the API server over HTTPS. Use the provided script to generate a key and certificate and to create the Kubernetes Secret and patch the `MutatingWebhookConfiguration` with the CA bundle.

From `deploy/`:

```bash
cd deploy
./gen-cert.sh [NAMESPACE]
```

- `NAMESPACE` defaults to `default`. The script creates certificates for `kata-annotation-webhook.<NAMESPACE>.svc`.
- It creates a Secret `kata-annotation-webhook-certs` and updates `webhook-registration.yaml` with the CA bundle (base64-encoded). It expects `webhook-registration.yaml` to contain a placeholder like `caBundle: <CA_BUNDLE>` (or similar) that it replaces.

**Note:** The script uses `sed -i`; ensure `webhook-registration.yaml` is in the expected format. You may need to set `CA_BUNDLE` in the `MutatingWebhookConfiguration` manually if your YAML structure differs.

### 2. Deploy the webhook and registration

- Apply the Deployment and Service (adjust image tag and namespace if needed):

  ```bash
  kubectl apply -f deploy/webhook.yaml
  ```

- Apply the mutating webhook configuration (after `gen-cert.sh` has set the CA bundle):

  ```bash
  kubectl apply -f deploy/webhook-registration.yaml
  ```

### 3. Verify

Create a test Pod with your Kata RuntimeClass and an `emptyDir` volume; then check the Pod’s annotations:

```bash
kubectl get pod <pod-name> -o jsonpath='{.metadata.annotations}' | jq .
```

You should see `io.katacontainers.sandbox.volumes.emptydir` with JSON describing the emptyDir volumes.

## Configuration

| Flag                 | Description                                      |
|----------------------|--------------------------------------------------|
| `-tls-cert-file`     | Path to TLS certificate file (enables HTTPS).   |
| `-tls-key-file`      | Path to TLS private key file.                   |

- If both TLS flags are set, the webhook serves on port **443** with TLS.
- If either is missing, it serves on port **8080** without TLS (suitable for local or in-cluster testing; production should use TLS and the 443 service port).

The `deploy/webhook.yaml` example uses TLS and mounts the certificate and key from the `kata-annotation-webhook-certs` Secret at `/etc/webhook/certs/`.

## Project structure

```
kata-annotation-webhook/
├── main.go              # Webhook server and pod mutator
├── go.mod / go.sum      # Go module (depends on src/runtime)
├── Makefile             # build, image, clean
├── Dockerfile           # Image that runs the binary
└── deploy/
    ├── webhook.yaml             # Deployment and Service
    ├── webhook-registration.yaml # MutatingWebhookConfiguration
    └── gen-cert.sh              # Certificate generation and CA bundle injection
```

## License

Apache-2.0 (see repository and file headers).
