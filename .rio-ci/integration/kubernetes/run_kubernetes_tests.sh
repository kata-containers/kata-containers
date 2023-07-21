#!/bin/bash
set -e

SHOULD_TEARDOWN="${SHOULD_TEARDOWN:-true}"

kubernetes_dir=$(dirname "$(readlink -f "$0")")
K8S_TEST_UNION=( \
    "k8s-test-rorw.bats" \
    "iptables.bats" \
    "k8s-attach-handlers.bats" \
	"k8s-caps.bats" \
	"k8s-configmap.bats" \
	"k8s-copy-file.bats" \
	"k8s-cpu-ns.bats" \
	"k8s-credentials-secrets.bats" \
	"k8s-custom-dns.bats" \
	"k8s-direct.bats" \
	"k8s-empty-dirs.bats" \
	"k8s-env.bats" \
	"k8s-exec.bats" \
#	"k8s-expose-ip.bats" \ (Temporary disabled)
	"k8s-inotify.bats" \
	"k8s-job.bats" \
	"k8s-limit-range.bats" \
	"k8s-liveness-probes.bats" \
	"k8s-memory.bats" \
	"k8s-nested-configmap-secret.bats" \
	"k8s-number-cpus.bats" \
	"k8s-oom.bats" \
	"k8s-optional-empty-configmap.bats" \
	"k8s-optional-empty-secret.bats" \
	"k8s-parallel.bats" \
	"k8s-pid-ns.bats" \
	"k8s-pod-connectivity.bats" \
	"k8s-pod-quota.bats" \
	"k8s-projected-volume.bats" \
	"k8s-qos-pods.bats" \
	"k8s-scale.bats" \
	"k8s-security-context.bats" \
	"k8s-shared-volume.bats" \
	"k8s-tiny.bats" \
	"k8s-volume.bats" \
)

if [ "${SHOULD_TEARDOWN}" = "true" ]; then
    trap '${kubernetes_dir}/teardown.sh' EXIT
fi

pushd "$kubernetes_dir"
for K8S_TEST_ENTRY in "${K8S_TEST_UNION[@]}"
do
	bats "${K8S_TEST_ENTRY}"
done
popd
