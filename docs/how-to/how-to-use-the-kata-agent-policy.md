# Kata Agent Policy

Agent Policy is a Kata Containers feature that enables the Guest VM to perform additional validation for each [ttRPC API](../../src/libs/protocols/protos/agent.proto) request. 

The Policy is commonly used for implementing confidential containers, where the Kata Shim and the Kata Agent have different trust properties. However, the Policy can be used for non-confidential containers too - e.g., for a basic defense in depth step of blocking the Host from starting an application on the Guest. However, for non-confidential containers, the Host might be able to modify the Policy and/or replace the Agent and disable its Policy rules, so a Policy is more helpful for confidential containers.

# Enabling the Kata Containers Policy code

When compiled with default settings, the Kata Containers code doesn't include the Policy feature. `AGENT_POLICY=yes` must be specified when building the Guest rootfs (by using [`rootfs.sh`](../../tools/osbuilder/rootfs-builder/rootfs.sh)). Specifying that build parameter has the following effects:

1. The [`Open Policy Agent (OPA)`](https://www.openpolicyagent.org/) binary gets installed in rootfs.

1. If the `AGENT_INIT=yes` build parameter was not specified, the `kata-opa` service gets included in the Guest rootfs. `OPA` will be started by `systemd` during the Kata Containers sandbox creation.

1. The Kata Agent gets built using `AGENT_POLICY=yes`, and therefore includes Policy support. If the `AGENT_INIT=yes` build parameter was specified in addition to `AGENT_POLICY=yes`, the Kata Agent will start `OPA` during the Kata Containers sandbox creation.

# Providing the Policy to the Kata Agent

There are two methods for providing the Policy document to the Kata Agent:

## Include Policy in the Guest rootfs

When building using `AGENT_POLICY=yes`, a [default Policy file](../../src/kata-opa/allow-all.rego) gets automatically included in the Kata Containers Guest rootfs. That default Policy is very permissive - allowing <i>all</i> the [ttRPC API](../../src/libs/protocols/protos/agent.proto) requests to be executed. To change the permissions granted by the default Policy, users must add their own Policy file and change the `/etc/kata-opa/default-policy.rego` symbolic link to point to their custom Policy file, in the Guest image.

## Specify Policy as a Kubernetes `YAML` annotation

Kubernetes users can encode in `base64` format their Policy documents, and add the encoded string as an annotation. Example:

### Encode a Policy file

For example, the [`allow-all-except-exec-process.rego`](../../src/kata-opa/allow-all-except-exec-process.rego) sample policy file is different from the [default Policy](../../src/kata-opa/allow-all.rego) because it rejects any `ExecProcess` requests. You can encode this policy file:

```bash
$ base64 -w 0 allow-all-except-exec-process.rego
cGFja2FnZSBhZ2VudF9wb2xpY3kKCmRlZmF1bHQgQWRkQVJQTmVpZ2hib3JzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgQWRkU3dhcFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENsb3NlU3RkaW5SZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBDb3B5RmlsZVJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENyZWF0ZUNvbnRhaW5lclJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENyZWF0ZVNhbmRib3hSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBEZXN0cm95U2FuZGJveFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IEdldE1ldHJpY3NSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBHZXRPT01FdmVudFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IEd1ZXN0RGV0YWlsc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IExpc3RJbnRlcmZhY2VzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgTGlzdFJvdXRlc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IE1lbUhvdHBsdWdCeVByb2JlUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgT25saW5lQ1BVTWVtUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUGF1c2VDb250YWluZXJSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBQdWxsSW1hZ2VSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBSZWFkU3RyZWFtUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVtb3ZlQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVtb3ZlU3RhbGVWaXJ0aW9mc1NoYXJlTW91bnRzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVzZWVkUmFuZG9tRGV2UmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVzdW1lQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgU2V0R3Vlc3REYXRlVGltZVJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFNldFBvbGljeVJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFNpZ25hbFByb2Nlc3NSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBTdGFydENvbnRhaW5lclJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFN0YXJ0VHJhY2luZ1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFN0YXRzQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgU3RvcFRyYWNpbmdSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBUdHlXaW5SZXNpemVSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVDb250YWluZXJSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVFcGhlbWVyYWxNb3VudHNSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVJbnRlcmZhY2VSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVSb3V0ZXNSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBXYWl0UHJvY2Vzc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFdyaXRlU3RyZWFtUmVxdWVzdCA6PSB0cnVlCgpkZWZhdWx0IEV4ZWNQcm9jZXNzUmVxdWVzdCA6PSBmYWxzZQo=
```

### Attach the Policy to a pod

Add the encoded Policy to your `YAML` file - e.g., `pod1.yaml`:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: policy-exec-rejected
  annotations:
    io.katacontainers.config.agent.policy: cGFja2FnZSBhZ2VudF9wb2xpY3kKCmRlZmF1bHQgQWRkQVJQTmVpZ2hib3JzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgQWRkU3dhcFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENsb3NlU3RkaW5SZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBDb3B5RmlsZVJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENyZWF0ZUNvbnRhaW5lclJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENyZWF0ZVNhbmRib3hSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBEZXN0cm95U2FuZGJveFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IEdldE1ldHJpY3NSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBHZXRPT01FdmVudFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IEd1ZXN0RGV0YWlsc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IExpc3RJbnRlcmZhY2VzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgTGlzdFJvdXRlc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IE1lbUhvdHBsdWdCeVByb2JlUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgT25saW5lQ1BVTWVtUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUGF1c2VDb250YWluZXJSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBQdWxsSW1hZ2VSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBSZWFkU3RyZWFtUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVtb3ZlQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVtb3ZlU3RhbGVWaXJ0aW9mc1NoYXJlTW91bnRzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVzZWVkUmFuZG9tRGV2UmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVzdW1lQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgU2V0R3Vlc3REYXRlVGltZVJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFNldFBvbGljeVJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFNpZ25hbFByb2Nlc3NSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBTdGFydENvbnRhaW5lclJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFN0YXJ0VHJhY2luZ1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFN0YXRzQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgU3RvcFRyYWNpbmdSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBUdHlXaW5SZXNpemVSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVDb250YWluZXJSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVFcGhlbWVyYWxNb3VudHNSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVJbnRlcmZhY2VSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBVcGRhdGVSb3V0ZXNSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBXYWl0UHJvY2Vzc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFdyaXRlU3RyZWFtUmVxdWVzdCA6PSB0cnVlCgpkZWZhdWx0IEV4ZWNQcm9jZXNzUmVxdWVzdCA6PSBmYWxzZQo=
spec:
  runtimeClassName: kata
  containers:
    - name: first-test-container
      image: quay.io/prometheus/busybox:latest
      command:
        - sleep
        - "120"
```

Create the pod:

```bash
$ kubectl apply -f pod1.yaml
```

While creating the Pod sandbox, the Kata Shim will notice the `io.katacontainers.config.agent.policy` annotation and will send the Policy document to the Kata Agent - by sending a `SetPolicy` request. Note that this request will fail if the default Policy, included in the Guest image, doesn't allow this `SetPolicy` request. If the `SetPolicy` request is rejected by the Guest, the Kata Shim will fail to start the Pod sandbox.

# How is the Policy being enforced?

The Kata Agent is responsible for enforcing the Policy, working together with `OPA`. The Agent checks the Policy for each [ttRPC API](../../src/libs/protocols/protos/agent.proto) request. Before carrying out the actions corresponding to the request, the Agent uses the [`OPA REST API`](https://www.openpolicyagent.org/docs/latest/rest-api/) to check if the Policy allows or blocks the call. The Agent rejects requests that are not allowed by the Policy.
