# RFC: Compose Initdata from CRDs


## 1. Problem


Currently, initdata plays an important role for CoCo's attestation. But for ease of use, it still has some shortages.




1. A pod carries Initdata in one annotation, `io.katacontainers.config.runtime.cc_init_data`, and the value is a gzipped, base64-encoded TOML document.
The initdata document is a [multi-line document](https://github.com/confidential-containers/trustee/blob/main/kbs/docs/initdata.md) and usually users need to write them in one file, and then run some script, then paste the result onto the pod.
This duplicated operation in a way makes life bad.


2. The annotation is also hard to share. Many pods use the same `aa.toml` or `cdh.toml`. Today each pod has its own copy of the whole document. Changing one shared key means rebuilding every annotation.


3. Operators make this worse. They create pods in code and there is no step where a person runs the encode tool. The pod is either missing Initdata, or it is stuck with a blob that was generated earlier. The annotation is all or nothing. Setting it replaces the whole document, and there is no way to replace one key and keep the others.


## 2. Proposal


This RFC propose to add one namespaced CRD, `InitData`.


One object is one entry in the Initdata `data` map. A pod names the objects it wants, and a webhook turns them into today's `cc_init_data` annotation. Kata still reads that annotation and does not know about the CRD.


CRD Object definition
```yaml
apiVersion: confidentialcontainers.org/v1alpha1
kind: InitData
metadata:
 name: aa-config
 namespace: default
spec:
 key: aa.toml
 value: |
   [token_configs.kbs]
   url = "http://kbs.coco-system.svc:8080"
```


Pod Yaml
```yaml
apiVersion: v1
kind: Pod
metadata:
 name: web
 annotations:
   confidentialcontainers.org/initdata: aa-config #,xxx,xxx
spec:
 runtimeClassName: kata-qemu-coco
 containers:
   - name: nginx
     image: nginx:1.25
```


The CRD contains the following fields:


- `metadata.name`: the object name. A pod refers to this name in the `confidentialcontainers.org/initdata` annotation.
- `metadata.namespace`: the namespace of the object. The webhook only reads objects in the same namespace as the pod.
- `metadata.labels.confidentialcontainers.org/initdata-default`: optional. When the value is `"true"`, every confidential pod in that namespace gets this object, even if the pod does not list the name.
- `spec.key`: the key written into the Initdata `data` map, for example `aa.toml` or `cdh.toml`.
- `spec.value`: the text stored under that key. The webhook copies it into the TOML document as a string.


If the pod already has `io.katacontainers.config.runtime.cc_init_data`, the webhook leaves the pod alone. That annotation is still a whole document, which is the path used today. If two selected objects use the same `spec.key`, the webhook rejects the pod.


This CRD does not decide which keys become separate files in the guest. Kata Agent does that. Today it writes the whole document to `/run/confidential-containers/initdata/initdata.toml`, and it also writes `aa.toml` and `cdh.toml` when those keys exist. Any other key stays inside `initdata.toml`. [confidential-containers#303](https://github.com/confidential-containers/confidential-containers/issues/303) discusses this, and this RFC does not change it.


The webhook runs outside the TEE. The guest measures the document it actually receives, and Trustee checks that measurement before reading the keys.


## 3. Examples


A cluster `aa.toml` can apply to every confidential pod in the namespace by label, so the pod does not have to name it.


```yaml
apiVersion: confidentialcontainers.org/v1alpha1
kind: InitData
metadata:
 name: aa-config
 namespace: default
 labels:
   confidentialcontainers.org/initdata-default: "true"
spec:
 key: aa.toml
 value: |
   [token_configs.kbs]
   url = "http://kbs.coco-system.svc:8080"
```


An app-specific key stays in the same `data` map. Kata does not create a separate file for it.


```yaml
apiVersion: confidentialcontainers.org/v1alpha1
kind: InitData
metadata:
 name: app-id
 namespace: default
spec:
 key: com.example.app-id
 value: |
   payments
```


This pod gets `aa-config` from the label and `app-id` from the annotation. The webhook merges them into one document.


```yaml
apiVersion: v1
kind: Pod
metadata:
 name: web
 annotations:
   confidentialcontainers.org/initdata: app-id
spec:
 runtimeClassName: kata-qemu-coco
 containers:
   - name: nginx
     image: nginx:1.25
```


The old annotation still wins. When it is set, the webhook does not read any `InitData` objects.


```yaml
metadata:
 annotations:
   io.katacontainers.config.runtime.cc_init_data: <base64-encoded Initdata>
```


## 4. Webhook


This CRD needs a mutating admission webhook. On pod creation it reads the selected objects, fills in `version` and `algorithm`, gzip-base64 encodes the TOML, and sets `io.katacontainers.config.runtime.cc_init_data`.


Kata and CoCo do not have an issue for one shared admission webhook. The webhook in `kata-containers` under `tools/testing/kata-webhook` only sets a runtime class for tests, and peer-pods has its own webhook for pod VMs.


Whether to include the code logic into current webhook code, or creating a new one is a open.


## 5. Related issues


- [confidential-containers#303](https://github.com/confidential-containers/confidential-containers/issues/303) discusses which Initdata keys Kata accepts, and whether unknown keys may exist. This RFC leaves that to Kata. The CRD only fills the `data` map.
- [confidential-containers#140](https://github.com/confidential-containers/confidential-containers/issues/140) asks for a kubectl plugin because users must pre-process pods before deploy. The CRD is the cluster-side place for the pieces that plugin would otherwise paste into annotations.
- [kata-containers#10980](https://github.com/kata-containers/kata-containers/issues/10980) and [kata-containers#11532](https://github.com/kata-containers/kata-containers/issues/11532) split guest components from the agent. [kata-containers#11655](https://github.com/kata-containers/kata-containers/pull/11655) adds an initdata processor that provisions keys from the document. Those keys still have to get into the document. This CRD is about that earlier step.
- [kata-containers#11428](https://github.com/kata-containers/kata-containers/issues/11428) moves image-pull settings from the kernel command line into Initdata. That is another key that wants to be managed outside a hand-built annotation.
- The Initdata document format stays the one in [kbs/docs/initdata.md](https://github.com/confidential-containers/trustee/blob/main/kbs/docs/initdata.md). This RFC does not add fields to that spec.



