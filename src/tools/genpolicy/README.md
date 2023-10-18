# Agent Policy generation tool

The Kata Containers policy generation tool (`genpolicy`): 

1. Reads user's Kubernetes YAML file.

1. Infers user's intentions based on the contents of that file.

1. Generates a Kata Containers Agent (`kata-agent`) policy file
corresponding to the input YAML, using the Rego/Open Policy Agent
format.

1. Appends the policy as an annotation to user's YAML file.

When the user deploys that YAML file, the Kata Agent uses the attached
policy to reject possible Agent API calls that are not consistent with
the policy.

Example:

```sh
$ genpolicy -y samples/pod-one-container.yaml
```

For a usage statement, run:

```sh
$ genpolicy --help
```
