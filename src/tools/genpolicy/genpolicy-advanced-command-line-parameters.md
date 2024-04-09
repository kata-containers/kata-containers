# Agent Policy generation tool - advanced command line parameters

See [`genpolicy`](README.md) for general information about the Kata Agent Policy generation tool.

# Basic `genpolicy` usage

The most basic way to use `genpolicy` is to provide just a Kubernetes YAML file as command line parameter - e.g.,

```bash
$ genpolicy -y test.yaml
```

`genpolicy` encodes the auto-generated Policy text in base64 format and appends the encoded string as an annotation to user's `YAML` file.

# Enable `genpolicy` logging

`genpolicy` is using standard Rust logging. To enable logging, use the RUST_LOG environment variable - e.g., 

```bash
$ RUST_LOG=info genpolicy -y test.yaml
```
or
```bash
$ RUST_LOG=debug genpolicy -y test.yaml
```

`RUST_LOG=debug` logs are more detailed than the `RUST_LOG=info` logs.

# Cache container image information

See [`genpolicy` Policy details](genpolicy-auto-generated-policy-details.md) for information regarding the contents of the auto-generated Policy. Part of the Policy contents is information used to verify the integrity of container images. In order to calculate the image integrity information, `genpolicy` needs to download the container images referenced by the `YAML` file. For example, when specifying the following YAML file as parameter:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: policy-test
spec:
  runtimeClassName: kata
  containers:
    - name: first-test-container
      image: quay.io/prometheus/busybox:latest
      command:
        - sleep
        - "120"
```

`genpolicy` downloads the `quay.io/prometheus/busybox:latest` container image. 

Depending on the size of the container images and the speed of the network connection to the container registry, downloading these images might take several minutes. For testing scenarios where `genpolicy` gets executed several times, it can be useful to cache the container images after downloading them, in order to avoid most of the time needed to download the same container images multiple times. If a container image layer was already cached locally, `genpolicy` uses the local copy of that container layer. The application caches the image information under the `./layers_cache` directory.

**Warning** Using cached image layers can lead to undesirable results. For example, if one or more locally cached layers have been modified (e.g., by an attacker) then the auto-generated Policy will allow those modified container images to be executed on the Guest VM.

To enable caching, use the `-u` command line parameter - e.g.,

```bash
$ RUST_LOG=info genpolicy -u -y test.yaml
```

# Use containerd to pull and manage images
You may specify `-d` to use existing `containerd` installation as image manager. This method supports a wider set of images (e.g., older images with `v1` manifest). Needs `sudo` permission to access socket - e.g.,

```bash
$ sudo genpolicy -d -y test.yaml
```

This will use `/var/contaienrd/containerd.sock` as default socket path. Or you may specify your own socket path - e.g.,

```bash
$ sudo genpolicy -d=/my/path/containerd.sock -y test.yaml
```

# Print the Policy text

To print the auto-generated Policy text, in addition to adding its `base64` encoding into the `YAML` file, specify the `-r` parameter - e.g.,

```bash
$ genpolicy -r -y test.yaml
```

# Print the `base64` encoded Policy

To print the `base64` encoded Policy, in addition to adding it into the `YAML` file, specify the `-b` parameter - e.g.,

```bash
$ genpolicy -b -y test.yaml
```

# Use a custom `genpolicy` settings file

The default `genpolicy` settings file is `./genpolicy-settings.json`. Users can specify in the command line a different settings file by using the `-j` parameter - e.g.,

```bash
$ genpolicy -j my-settings.json -y test.yaml
```

# Use a custom path to `genpolicy` input files

By default, the `genpolicy` input files [`rules.rego`](rules.rego) and [`genpolicy-settings.json`](genpolicy-settings.json) must be present in the current directory - otherwise `genpolicy` returns an error. Users can specify different paths to these two files, using the `-p` and `-j` command line parameters - e.g.,

```bash
$ genpolicy -p /tmp/rules.rego -j /tmp/genpolicy-settings.json -y test.yaml
```

# Silently ignore unsupported input `YAML` fields

As described by the [Kubernetes docs](https://kubernetes.io/docs/reference/), K8s supports a very large number of fields in `YAML` files. `genpolicy` supports just a subset of these fields (hopefully the most commonly used fields!). The `genpolicy` authors reviewed the `YAML` fields that are supported as inputs to this tool, and evaluated the impact of each field for confidential containers. Some other input fields were not evaluated and/or don't make much sense when present in an input `YAML` file. By default, when `genpolicy` encounters an unsupported field in its input `YAML` file, the application returns an error.

For example, when the input `YAML` contains:

```yaml
apiVersion: v1
kind: Pod
metadata:
  creationTimestamp: "2023-09-18T23:08:02Z"
```

`genpolicy` returns an error, because:
1. Specifying a fixed creation timestamp as input for a pod doesn't seem very helpful.
1. The `genpolicy` authors did not evaluate the potential effects of this field when creating a confidential containers pod.

Users can choose to silently ignore unsupported fields by using the `-s` parameter:

```bash
$ genpolicy -s -y test.yaml
```

**Warning** Ignoring unsupported input `YAML` fields can result in generating an unpredictably incorrect Policy. The `-s` parameter should be used just by expert `K8s` and confidential container users, and only after they carefully evaluate the effects of ignoring these fields.

**Tip** The `-s` parameter can be helpful for example when investigating a problem related to an already created Kubernetes pod - e.g.,:

1. Obtain the existing pod YAML from Kubernetes:

```bash
kubectl get pod my-pod -o yaml > my-pod.yaml
```

2. Auto-generate a Policy corresponding to that `YAML` file:

```bash
$ genpolicy -s -y my-pod.yaml
```

# Specify an input `ConfigMap YAML` file

`genpolicy` doesn't attach a Policy to [`ConfigMap`](https://kubernetes.io/docs/reference/kubernetes-api/config-and-storage-resources/config-map-v1/) `YAML` files. However, a `ConfigMap` `YAML` file might be required for generating a reasonable Policy for other types of `YAML` files.

For example, given just this `Pod` input file (`test.yaml`):

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: policy-test
spec:
  runtimeClassName: kata
  containers:
    - name: first-test-container
      image: quay.io/prometheus/busybox:latest
      command:
        - sleep
        - "120"
      env:
        - name: CONFIG_MAP_VALUE1
          valueFrom:
            configMapKeyRef:
              key: simple_value1
              name: config-map1
```

`genpolicy` is not able to generate the Policy data used to verify the expected value of the CONFIG_MAP_VALUE1 environment variable. There are two ways to specify the required `ConfigMap` information:

## Specify a `ConfigMap` input `YAML` file in the command line

A user can create for example `test-config.yaml` with the following contents:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: config-map1
data:
  simple_value1: value1
```

and specify that file in the `genpolicy` command line using the `-c` parameter:

```bash
$ genpolicy -c test-config.yaml -y test.yaml
```

## Add the `ConfigMap` information into the input `YAML` file

The same `ConfigMap` information above can be added to `test.yaml`:

```yaml
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: config-map1
data:
  simple_value1: value1
---
apiVersion: v1
kind: Pod
metadata:
  name: policy-test
spec:
  runtimeClassName: kata
  containers:
    - name: first-test-container
      image: quay.io/prometheus/busybox:latest
      command:
        - sleep
        - "120"
      env:
        - name: CONFIG_MAP_VALUE1
          valueFrom:
            configMapKeyRef:
              key: simple_value1
              name: config-map1
```

and then the `-c` parameter is no longer needed:

```bash
$ genpolicy -y test.yaml
```
