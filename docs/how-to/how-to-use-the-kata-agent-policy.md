# Kata Agent Policy

Agent Policy is a Kata Containers feature that enables the Guest VM to perform additional validation for each [ttRPC API](../../src/libs/protocols/protos/agent.proto) request. 

The Policy is commonly used for implementing confidential containers, where the Kata Shim and the Kata Agent have different trust properties. However, the Policy can be used for non-confidential containers too - e.g., for a basic defense in depth step of blocking the Host from starting an application on the Guest. However, for non-confidential containers, the Host might be able to modify the Policy and/or replace the Agent and disable its Policy rules, so a Policy is more helpful for confidential containers.

# Enabling the Kata Containers Policy code

When compiled with default settings, the Kata Containers code doesn't include the Policy feature. `AGENT_POLICY=yes` must be specified when building the Guest rootfs (by using [`rootfs.sh`](../../tools/osbuilder/rootfs-builder/rootfs.sh)). Specifying that build parameter has the following effects:

1. The [`Open Policy Agent (OPA)`](https://www.openpolicyagent.org/) binary gets installed in rootfs.

1. If the `AGENT_INIT=yes` build parameter was not specified, the `kata-opa` service gets included in the Guest rootfs. `OPA` will be started by `systemd` during the Kata Containers sandbox creation.

1. The Kata Agent gets built using `AGENT_POLICY=yes`, and therefore includes Policy support. If the `AGENT_INIT=yes` build parameter was specified in addition to `AGENT_POLICY=yes`, the Kata Agent will start `OPA` during the Kata Containers sandbox creation.

# Policy format

The Policy document is a text file using the [`Rego` policy language](https://www.openpolicyagent.org/docs/latest/policy-language/). See [Creating the Policy document](#creating-the-policy-document) for information related to creating Policy files.

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

The Kata Agent is responsible for enforcing the Policy, working together with [`OPA`](https://www.openpolicyagent.org/). The Agent checks the Policy for each [ttRPC API](../../src/libs/protocols/protos/agent.proto) request. Before carrying out the actions corresponding to the request, the Agent uses the [`OPA REST API`](https://www.openpolicyagent.org/docs/latest/rest-api/) to check if the Policy allows or blocks the request. The Agent rejects requests that are not allowed by the Policy.

# Creating the Policy document

## Creating the Policy document manually

For relatively simple uses cases, users can write the Policy text using the [`Rego` policy language documentation](https://www.openpolicyagent.org/docs/latest/policy-language/) as reference.

See [Policy contents](#policy-contents) for additional information.

## Using auto-generated Policy

The [`genpolicy`](../../src/tools/genpolicy/) application can be used to generate automatically a Policy matching an input Kubernetes `YAML` file. The Policy generated by this application is typically used for implementing confidential containers, where the Kata Shim and the Kata Agent have different trust properties.

**Warning** Users should review carefully the automatically-generated Policy, and modify the Policy file if needed to match better their use case, before using this Policy.

See the [`genpolicy` documentation](../../src/tools/genpolicy/README.md) and the [Policy contents examples](#policy-contents) for additional information.

## Policy contents

### The [`Rego`](https://www.openpolicyagent.org/docs/latest/policy-language/) package name

The name of the Kata Agent Policy package must be `agent_policy`. Therefore, all Agent Policy documents must start with:

```
package agent_policy
```
### Default values

When the Kata Shim sends a [ttRPC API](../../src/libs/protocols/protos/agent.proto) request to the Kata Agent, the [Policy rules](#rules) corresponding to that request type are evaluated. For example, when the Agent receives a `CopyFile` request, any rules defined in the Policy that are using the name `CopyFileRequest` are evaluated. [`OPA`](https://www.openpolicyagent.org/) evaluates these rules and tries to find at least one `CopyFileRequest` rule that returns value `true`:

1. If at least one `CopyFileRequest` rule returns `true`, `OPA` returns a `true` result to the Kata Agent, and the Agent carries out the file copy requested by the Shim.

1. If all the `CopyFileRequest` rules return `false`:
     - If the Policy includes a default value for `CopyFileRequest`, `OPA` returns that value to the Agent.
     - If the Policy doesn't include a default value for `CopyFileRequest`, `OPA` returns an empty response to the Agent. The Agent treats the empty response the same way as a `false` response, so it rejects the `CopyFile` request.

**Tip:** Although the Kata Agent treats empty responses from `OPA` similarly to `false` responses, it is recommended to always provide default values. With default values, the Policy document and the logs from `OPA` and Kata Agent are easier to understand.

Examples of default values:

```
default WaitProcessRequest := true
default ExecProcessRequest := false
```

### Policy data

Policy data is optional. It typically contains values that are compared by the [Policy rules](#rules) with the input parameters of a [ttRPC API](../../src/libs/protocols/protos/agent.proto) request. Based on this comparison, a rule can either allow or deny the request, by returning `true` or `false`.

Example of Policy data:

```
policy_data := {
  "common": {
    "cpath": "/run/kata-containers/shared/containers"
  },
  "request_defaults": {
    "CopyFileRequest": [
      "^$(cpath)/"
    ],
    "ExecProcessRequest": {
      "commands": [
        "/bin/foo"
      ],
      "regex": []
    }
  }
}
```

### Rules

Policy rules are optional. They typically compare the input parameters of a [ttRPC API](../../src/libs/protocols/protos/agent.proto) request with values from the [policy data](#policy-data). Based on this comparison, a rule can either allow or deny the request, by returning `true` or `false`.

Multiple rules having the same name can be defined in the same Policy. As described [above](#default-values), when the Kata Agent queries [`OPA`](https://www.openpolicyagent.org/) by using the [`OPA REST API`](https://www.openpolicyagent.org/docs/latest/rest-api/), `OPA` tries to find at least one rule having the same name as the request that returns `true` given the API input parameters defined by the [ttRPC API](../../src/libs/protocols/protos/agent.proto).

Examples of rules, corresponding to the Kata Agent `CopyFile` and `ExecProcess` requests:

```
import future.keywords.in
import input

CopyFileRequest {
    print("CopyFileRequest: input.path =", input.path)

    some regex1 in policy_data.request_defaults.CopyFileRequest
    regex2 := replace(regex1, "$(cpath)", policy_data.common.cpath)
    regex.match(regex2, input.path)

    print("CopyFileRequest: true")
}

ExecProcessRequest {
    print("ExecProcessRequest 1: input =", input)

    i_command = concat(" ", input.process.Args)
    print("ExecProcessRequest 1: i_command =", i_command)

    some p_command in policy_data.request_defaults.ExecProcessRequest.commands
    p_command == i_command

    print("ExecProcessRequest 1: true")
}

ExecProcessRequest {
    print("ExecProcessRequest 2: input =", input)

    i_command = concat(" ", input.process.Args)
    print("ExecProcessRequest 2: i_command =", i_command)

    some p_regex in policy_data.request_defaults.ExecProcessRequest.regex
    print("ExecProcessRequest 2: p_regex =", p_regex)

    regex.match(p_regex, i_command)

    print("ExecProcessRequest 2: true")
}

```

The `input` data from these examples is provided to `OPA` by the Kata Agent, as a ``JSON`` format representation of the API request parameters.

For additional examples of Policy rules, see [`rules.rego`](../../src/tools/genpolicy/rules.rego).
