# Kata Admission controller webhook

Implement a simple admission controller webhook to annotate pods with the
Kata runtime class.

## How to build the admission controller

> **Note:**
> Only run this step if you are modifying the current webhook or don't
> want to use the webhook available in docker hub.

First build the admission controller image and the associated
Kubernetes YAML files required to instantiate the admission
controller.

```bash
docker build -t quay.io/kata-containers/kata-webhook-example:latest -f Dockerfile .
```

> **Note**
> Image needs to be published for the webhook needs to work. Alternately
> on a single machine cluster change the `imagePullPolicy` to use the locally
> built image.

## Making Kata the default runtime using an admission controller

Today in `crio.conf` `runc` is the default runtime when a user does not specify
`runtimeClass` in the pod spec. If you want to run a cluster where Kata is used
by default, except for workloads we know for sure will not work with Kata, use
the [admission webhook](https://kubernetes.io/docs/reference/access-authn-authz/extensible-admission-controllers/#admission-webhooks)
and sample admission controller we created by running the commands below:

> **Note**
>
> By default, the `runtimeClass` name used in this webhook is `kata`. If your
> cluster is configured with another `runtimeClass`, you'll need to change the
> value of the `RUNTIME_CLASS` environment variable defined in the
> [webhook file](deploy/webhook.yaml). You can manually edit the file or run:
>
> `export RUNTIME_CLASS=<>`
>
> `kubectl create cm kata-webhook --from-literal runtime_class=$RUNTIME_CLASS`

```bash
./create-certs.sh
kubectl apply -f deploy/
```

Afterwards you can run the `webhook-check.sh` script to check the webhook was
deployed correctly and is working:

```bash
./webhook-check.sh
```

The webhook mutates pods to use the Kata runtime class for all pods except
those with

* `hostNetwork: true`
* namespace: `rook-ceph` and `rook-ceph-system`
