import os
import subprocess
import sys

# runs genpolicy tools on the following files
# should run this after any change to genpolicy
# usage: python3 update_policy_samples.py

yaml_files = [
    "configmap/pod-cm1.yaml",
    "configmap/pod-cm2.yaml",
    "deployment/deployment-azure-vote-back.yaml",
    "deployment/deployment-azure-vote-front.yaml",
    "deployment/deployment-busybox.yaml",
    "job/test-job.yaml",
    "kubernetes/conformance/conformance-e2e.yaml",
    "kubernetes/conformance/csi-hostpath-plugin.yaml",
    "kubernetes/conformance/csi-hostpath-testing.yaml",
    "kubernetes/conformance/etcd-statefulset.yaml",
    "kubernetes/conformance/hello-populator-deploy.yaml",
    "kubernetes/conformance/netexecrc.yaml",
    "kubernetes/conformance2/ingress-http-rc.yaml",
    "kubernetes/conformance2/ingress-http2-rc.yaml",
    "kubernetes/conformance2/ingress-multiple-certs-rc.yaml",
    "kubernetes/conformance2/ingress-nginx-rc.yaml",
    "kubernetes/conformance2/ingress-static-ip-rc.yaml",
    "kubernetes/fixtures/appsv1deployment.yaml",
    "kubernetes/fixtures/daemon.yaml",
    "kubernetes/fixtures/deploy-clientside.yaml",
    "kubernetes/fixtures/job.yaml",
    "kubernetes/fixtures/multi-resource-yaml.yaml",
    "kubernetes/fixtures/rc-lastapplied.yaml",
    "kubernetes/fixtures/rc-noexist.yaml",
    "kubernetes/fixtures/replication.yaml",
    "kubernetes/fixtures2/rc-service.yaml",
    "kubernetes/fixtures2/valid-pod.yaml",
    "kubernetes/incomplete-init/cassandra-statefulset.yaml",
    "kubernetes/incomplete-init/controller.yaml",
    "kubernetes/incomplete-init/cockroachdb-statefulset.yaml",
    "kubernetes/incomplete-init/node_ds.yaml",
    "pod/pod-exec.yaml",
    "pod/pod-lifecycle.yaml",
    "pod/pod-one-container.yaml",
    "pod/pod-persistent-volumes.yaml",
    "pod/pod-same-containers.yaml",
    "pod/pod-spark.yaml",
    "pod/pod-three-containers.yaml",
    "replica-set/replica-busy.yaml",
    "secrets/azure-file-secrets.yaml",
    "stateful-set/web.yaml",
]

silently_ignored_yaml_files=[
    "webhook/webhook-pod1.yaml",
    "webhook/webhook-pod2.yaml",
    "webhook/webhook-pod3.yaml",
    "webhook/webhook-pod4.yaml",
    "webhook/webhook-pod5.yaml",
    "webhook/webhook-pod6.yaml",
    "webhook/webhook-pod7.yaml",
    "webhook2/webhook-pod8.yaml",
    "webhook2/webhook-pod9.yaml",
    "webhook2/webhook-pod10.yaml",
    "webhook2/webhook-pod11.yaml",
    "webhook2/webhook-pod12.yaml",
    "webhook2/webhook-pod13.yaml",
    "webhook3/dns-test.yaml",
    "webhook3/many-layers.yaml",
]

no_policy_yaml_files=[
    "kubernetes/fixtures/limits.yaml",
    "kubernetes/fixtures/namespace.yaml",
    "kubernetes/fixtures/quota.yaml",
]

file_base_path = "../../agent/samples/policy/yaml"

def runCmd(arg):
    proc = subprocess.run([arg], stdout=sys.stdout, stderr=sys.stderr, universal_newlines=True, input="", shell=True)
    if proc.returncode != 0:
        print(f"`{arg}` failed with exit code {proc.returncode}. Stderr: {proc.stderr}, Stdout: {proc.stdout}")
    return proc

# check we can access all files we are about to update
for file in yaml_files + silently_ignored_yaml_files + no_policy_yaml_files:
    filepath = os.path.join(file_base_path, file)
    if not os.path.exists(filepath):
        print(f"filepath does not exists: {filepath}")

# build tool
runCmd("cargo build")

# update files
genpolicy_path = "target/debug/genpolicy"
for file in yaml_files:
    runCmd(f"{genpolicy_path} -y {os.path.join(file_base_path, file)}")

for file in silently_ignored_yaml_files:
    runCmd(f"{genpolicy_path} -y {os.path.join(file_base_path, file)} -s")

for file in no_policy_yaml_files:
    runCmd(f"{genpolicy_path} -y {os.path.join(file_base_path, file)}")

