# Kata Admission controller webhook

Implement a simple admission controller webhook to annotate pods with the 
Kata runtime class.

## How to build the admission controller

First build the admission controller image and the associated
Kubernetes yaml files required to instantiate the admission
controller.

```bash
$ docker build -t katadocker/kata-webhook-example:latest .
$ ./create_certs.sh
```

> **Note:**
> Image needs to be published for the webhook needs to work. Alternately
> on a single machine cluster change the `imagePullPolicy` to use the locally
> built image.

## Making Kata the default runtime using an admission controller

Today in `crio.conf` `runc` is the default runtime when a user does not specify
`runtimeClass` in the pod spec. If you want to run a cluster where Kata is used
by default, except for workloads we know for sure will not work with Kata, use
the [admission webhook](https://kubernetes.io/docs/reference/access-authn-authz/extensible-admission-controllers/#admission-webhooks)
and sample admission controller we created by running

```bash
$ kubectl apply -f deploy/
```
The admission webhook (deploy/webhook-registration.yaml)
is setup to exclude certian namespaces from being run with Kata using filters on namespace labels.

```yaml
    namespaceSelector:
      matchExpressions:
        -  {key: "kata", operator: NotIn, values: ["false"]}
```
The rook operators for example are marked as such.

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: rook-ceph-system
  labels:
    kata: "false"
```

Pods not explicitly excluded by the namespace filter are dynamically tagged to run with Kata with some exceptions.

* `hostNetwork: true`
* `rook-ceph` and `rook-ceph-system` namespaces

