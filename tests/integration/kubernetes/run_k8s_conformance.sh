#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Run the upstream Kubernetes conformance suite (e2e.test) against a cluster
# where Kata Containers has been deployed by kata-deploy. Upstream test pods do
# not set a runtimeClassName, so the kata-webhook from tools/testing moves them
# to the Kata runtime class under test.

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
source "${kubernetes_dir}/../../common.bash"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-runtime-rs}"
KUBECONFIG="${KUBECONFIG:-${HOME}/.kube/config}"
export KUBECONFIG

# Runtime class assigned to the conformance pods.
CONFORMANCE_RUNTIME_CLASS="${CONFORMANCE_RUNTIME_CLASS:-kata-${KATA_HYPERVISOR}}"
# Version of e2e.test to run, defaults to the version of the API server.
CONFORMANCE_K8S_VERSION="${CONFORMANCE_K8S_VERSION:-}"
CONFORMANCE_FOCUS="${CONFORMANCE_FOCUS:-\[Conformance\]}"
# Extra regex of tests to skip, on top of the known Kata limitations below.
CONFORMANCE_SKIP="${CONFORMANCE_SKIP:-}"
CONFORMANCE_PARALLEL_NODES="${CONFORMANCE_PARALLEL_NODES:-4}"
# Set to "no" to skip the [Serial] conformance tests.
CONFORMANCE_RUN_SERIAL="${CONFORMANCE_RUN_SERIAL:-yes}"
CONFORMANCE_WORK_DIR="${CONFORMANCE_WORK_DIR:-/tmp/kata-k8s-conformance}"
CONFORMANCE_REPORT_DIR="${CONFORMANCE_REPORT_DIR:-${kubernetes_dir}/reports/conformance-${KATA_HYPERVISOR}}"
# kata-webhook image to deploy. When unset, the webhook is built from this tree
# with podman and imported into the node's containerd.
CONFORMANCE_WEBHOOK_IMAGE="${CONFORMANCE_WEBHOOK_IMAGE:-}"
# Pod-level memory limit the webhook gives pods that set a CPU limit but no
# memory limit. Kata sizes the VM from limits only, so without it those pods get
# a VM with just the configured memory overhead. Matches the default_memory of
# the runtime-rs configurations.
CONFORMANCE_DEFAULT_MEMORY_LIMIT="${CONFORMANCE_DEFAULT_MEMORY_LIMIT:-2Gi}"

webhook_src_dir="${kubernetes_dir}/../../../tools/testing/kata-webhook"
webhook_ns="kata-webhook"
webhook_name="pod-annotate"
webhook_local_image="localhost/kata-webhook:conformance"
# Namespaces whose pods are left on the default runtime.
excluded_namespaces=(kube-system kube-public kube-node-lease kube-flannel "${webhook_ns}")

# Conformance tests that require more than one node. This cluster is a single
# kubeadm node, so both fail instead of exercising Kata:
#   - "should have at least two untainted nodes" fails with
#     "Conformance requires at least two nodes".
#   - "should rollback without unnecessary restarts" fails with
#     "needs a cluster with at least 2 nodes".
# These are the only two [Conformance] tests in Kubernetes v1.37.1 with that
# requirement. Other e2e tests that want two nodes are outside this suite and
# skip themselves.
#
# Conformance tests that assume pods carry no RuntimeClass overhead. The Kata
# RuntimeClasses declare a memory overhead for the VM, which the API server adds
# to the pod requests and quota charges, so pods sized to just fit a 500Mi
# quota are rejected at admission, e.g. 252Mi + 320Mi on qemu-runtime-rs:
#   - "should create a ResourceQuota and capture the life of a pod"
#   - "should verify ResourceQuota with terminating scopes"
#
# "resize with equivalents" creates a burstable pod with a CPU limit, no memory
# limit and no pod-level resources, then requires spec.resources to stay nil.
# Kata sizes the sandbox from limits only, so that pod would get a VM with
# nothing but the runtime's memory overhead, which cannot boot. The webhook
# therefore sets a pod-level memory limit, and the API server then defaults the
# rest of spec.resources (the memory request becomes that limit, and the CPU
# comes from the containers). The test compares that object to nil and fails
# before it resizes anything.
#
# Five Downward API env var tests give their container a 64Mi memory limit.
# Kata sizes the VM as that limit plus overhead_memory (32MiB), and in a 96M
# guest the kernel, the agent and the container setup do not reliably fit:
# about one pod in twenty fails at create with "Broken pipe" or "failed to
# receive async message", because the OOM killer took the agent's child
# process. The tests read the 64Mi limit back, so the webhook cannot raise it.
declare -A kata_skips=(
	["should have at least two untainted nodes"]="asserts the cluster has two schedulable nodes"
	["should rollback without unnecessary restarts"]="DaemonSet rollback needs a second node to roll onto"
	["should create a ResourceQuota and capture the life of a pod"]="RuntimeClass overhead exceeds the quota"
	["should verify ResourceQuota with terminating scopes\\. "]="RuntimeClass overhead exceeds the quota"
	["resize with equivalents \\[MinimumKubeletVersion:1.34\\] \\[Conformance\\]"]="pod-level memory limit makes spec.resources non-nil"
	["Downward API should provide pod name, namespace and IP address as env vars"]="a 96M VM can OOM the agent"
	["Downward API should provide host IP as an env var"]="a 96M VM can OOM the agent"
	["Downward API should provide hostIPs as an env var"]="a 96M VM can OOM the agent"
	["Downward API should provide container's limits.cpu/memory and requests.cpu/memory as env vars"]="a 96M VM can OOM the agent"
	["Downward API should provide pod UID as env vars"]="a 96M VM can OOM the agent"
)

# Conformance tests the NVIDIA runtimes cannot pass by design. They run with
# shared_fs = "none", where volumes are copied into the guest and guest writes
# never reach the host, and with emptydir_mode = "block-plain":
#   - "ignore failure matching on DisruptionTarget condition" has evicted pods
#     touch a file in a hostPath volume, and only completes when their
#     replacements find it there.
#   - The two Variable Expansion subpath tests mount a subPath of an emptyDir.
#     kubelet creates that subPath in the host directory, while a block emptyDir
#     lives on a guest block device, so the subPath and the whole volume end up
#     on different storage.
declare -A no_shared_fs_skips=(
	["should allow to use a pod failure policy to ignore failure matching on DisruptionTarget condition"]="hostPath writes stay in the guest"
	["should allow substituting values in a volume subpath"]="subPath of a block emptyDir is not supported"
	["should succeed in writing subpaths in container"]="subPath of a block emptyDir is not supported"
)

# Conformance tests that fail intermittently on OpenVMM only:
#   - Three Projected PodCertificate tests start a container that cats its
#     credential bundle, and read the pod logs once as soon as the pod is
#     Running. On OpenVMM the output can reach the log file after that read,
#     and the test fails with "data does not contain any valid RSA or ECDSA
#     certificates". The mTLS test of the same suite retries and passes.
#   - The HostPort no conflict test needs its third pod admitted, and kubelet
#     rejected it with OutOfcpu while the parallel tests still held the CPU.
declare -A openvmm_skips=(
	["should honor UserAnnotations for SPIFFE URI path"]="pod logs are read before the output arrives"
	["should issue certificate with default life time \\(24h\\) when MaxExpirationSeconds is not set"]="pod logs are read before the output arrives"
	["should issue certificate with specified duration \\(1h\\) when MaxExpirationSeconds is set"]="pod logs are read before the output arrives"
	["same hostPort but different hostIP and protocol"]="a pod is rejected with OutOfcpu"
)

# Conformance tests that do not fit on a node with fewer than
# small_node_cpu_threshold allocatable CPUs. The GitHub-hosted runners have 4,
# and every Kata pod also requests the RuntimeClass CPU overhead (250m), so
# about a dozen pods fill the node after the control plane. Pods left over fail
# scheduling with "Insufficient cpu", or kubelet rejects them with OutOfcpu.
# All of them pass on a 12-CPU node:
#   - "Guestbook application" starts six pods, 350m each with the overhead,
#     next to the other parallel tests.
#   - "deployment should support proportional scaling" waits for all 10
#     replicas of its Deployment, 2.5 CPUs of overhead, to be running. While it
#     waits, the pods it got starve the other parallel tests.
#   - "Service endpoints latency" needs one more pod while the parallel tests
#     already hold most of the CPU.
#   - "should not cause race condition when used for configmaps" creates its
#     replication controller again while the previous pods still hold their
#     CPU, which leaves new pods Pending past the 5 minute timeout.
#   - "should orphan pods created by rc if delete options say so" counts the
#     pods of a 50-replica replication controller. Pods rejected with OutOfcpu
#     are replaced, so the count is off.
#   - "should delete a collection of pods" and "Should recreate evicted
#     statefulset" need every pod they create to run, and kubelet rejects one
#     of them with OutOfcpu while the parallel tests hold the CPU.
#   - "should *not* be restarted with a tcp:8080 liveness probe" requires zero
#     restarts over four minutes. With the node CPU saturated, a probe misses
#     its one second timeout ("i/o timeout") and kubelet restarts the
#     container.
small_node_cpu_threshold=8
declare -A small_node_skips=(
	["Guestbook application should create and stop a working application"]="six pods with CPU overhead do not fit"
	["deployment should support proportional scaling"]="ten replicas with CPU overhead do not fit"
	["Service endpoints latency should not be very high"]="no CPU left for the latency pod"
	["should not cause race condition when used for configmaps"]="replacement pods stay Pending on CPU"
	["should orphan pods created by rc if delete options say so"]="OutOfcpu rejections change the pod count"
	["Pods should delete a collection of pods"]="a pod is rejected with OutOfcpu"
	["Should recreate evicted statefulset"]="a pod is rejected with OutOfcpu"
	["should \\*not\\* be restarted with a tcp:8080 liveness probe"]="probes time out on a saturated node"
)

# Allocatable CPUs of the node, rounded down.
node_allocatable_cpus() {
	local cpu
	cpu="$(kubectl get nodes -o jsonpath='{.items[0].status.allocatable.cpu}')"
	if [[ "${cpu}" == *m ]]; then
		echo $(( ${cpu%m} / 1000 ))
	else
		echo "${cpu}"
	fi
}

kata_skip_regex() {
	local regex=""
	local test_name

	for test_name in "${!kata_skips[@]}"; do
		regex+="${regex:+|}${test_name}"
	done
	if [[ "${KATA_HYPERVISOR}" == *nvidia* ]]; then
		for test_name in "${!no_shared_fs_skips[@]}"; do
			regex+="${regex:+|}${test_name}"
		done
	fi
	if [[ "${KATA_HYPERVISOR}" == openvmm* ]]; then
		for test_name in "${!openvmm_skips[@]}"; do
			regex+="${regex:+|}${test_name}"
		done
	fi
	if (( $(node_allocatable_cpus) < small_node_cpu_threshold )); then
		for test_name in "${!small_node_skips[@]}"; do
			regex+="${regex:+|}${test_name}"
		done
	fi
	[[ -n "${CONFORMANCE_SKIP}" ]] && regex+="${regex:+|}${CONFORMANCE_SKIP}"

	echo "${regex}"
}

install_e2e_binaries() {
	local version="${CONFORMANCE_K8S_VERSION}"
	local arch
	local bin_dir="${CONFORMANCE_WORK_DIR}/bin"

	if [[ -z "${version}" ]]; then
		ensure_yq
		version="$(kubectl version -o yaml | yq '.serverVersion.gitVersion')"
	fi
	[[ "${version}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]] || die "Unexpected Kubernetes version '${version}'"
	arch="$(arch_to_golang)"

	if [[ -x "${bin_dir}/e2e.test" ]] && "${bin_dir}/e2e.test" --version 2>/dev/null | grep -qw "${version}"; then
		info "Reusing e2e.test ${version} from ${bin_dir}"
		return
	fi

	info "Downloading the Kubernetes ${version} e2e binaries"
	rm -rf "${bin_dir}"
	mkdir -p "${bin_dir}"
	curl -fsSL "https://dl.k8s.io/${version}/kubernetes-test-linux-${arch}.tar.gz" | \
		tar -xz -C "${bin_dir}" --strip-components=3 \
			kubernetes/test/bin/e2e.test kubernetes/test/bin/ginkgo
	"${bin_dir}/e2e.test" --version
}

# The ubuntu-24.04 runner image ships podman. It has no daemon, so unlike
# dockerd it does not set the iptables FORWARD policy to DROP. The build uses
# the host network so it does not install NAT rules next to kube-proxy, and
# the image is then imported into the node's containerd.
build_webhook_image() {
	command -v podman >/dev/null || die "podman is required to build the kata-webhook image"
	info "Building the kata-webhook image ${webhook_local_image}"
	make -C "${webhook_src_dir}" IMAGE="${webhook_local_image}" \
		BUILDER="sudo podman" BUILD_FLAGS="--network=host"
	sudo podman save "${webhook_local_image}" | \
		sudo ctr -n k8s.io images import --platform "linux/$(arch_to_golang)" -
	# containerd's CRI does not always notice an image imported with ctr until
	# it is restarted, and the webhook pod then fails with ImagePullBackOff.
	if command -v crictl >/dev/null && ! sudo crictl images | grep -q 'kata-webhook'; then
		info "containerd did not see the imported webhook image, restarting it"
		sudo systemctl restart containerd
		sudo crictl images | grep -q 'kata-webhook' || \
			die "containerd did not import ${webhook_local_image}"
	fi
	CONFORMANCE_WEBHOOK_IMAGE="${webhook_local_image}"
}

# The webhook registration fails closed, so a pod is never silently run on the
# default runtime, and skips the namespaces above, which include the webhook's
# own. The webhook itself leaves host network pods on the default runtime, as
# Kata cannot share the host network namespace.
deploy_webhook() {
	local deploy_dir="${CONFORMANCE_WORK_DIR}/kata-webhook"
	local namespaces

	kubectl get runtimeclass "${CONFORMANCE_RUNTIME_CLASS}" >/dev/null || \
		die "RuntimeClass ${CONFORMANCE_RUNTIME_CLASS} does not exist, is kata-deploy installed?"
	ensure_yq
	[[ -n "${CONFORMANCE_WEBHOOK_IMAGE}" ]] || build_webhook_image

	rm -rf "${deploy_dir}"
	cp -r "${webhook_src_dir}" "${deploy_dir}"
	(cd "${deploy_dir}" && WEBHOOK_NS="${webhook_ns}" WEBHOOK_NAME="${webhook_name}" ./create-certs.sh)

	# A namespace left terminating by a previous run would reject the new objects.
	kubectl delete namespace "${webhook_ns}" --ignore-not-found --wait=true --timeout=300s
	kubectl create namespace "${webhook_ns}" --dry-run=client -o yaml | kubectl apply -f -
	kubectl -n "${webhook_ns}" create configmap kata-webhook \
		--from-literal=runtime_class="${CONFORMANCE_RUNTIME_CLASS}" \
		--from-literal=default_memory_limit="${CONFORMANCE_DEFAULT_MEMORY_LIMIT}" \
		--dry-run=client -o yaml | kubectl apply -f -
	kubectl -n "${webhook_ns}" apply -f "${deploy_dir}/deploy/webhook-certs.yaml"
	# The SchedulerPreemption tests fill the node and then preempt the lowest
	# priority pods. With the default priority the webhook is a victim, and
	# every pod created until its replacement is ready is rejected.
	IMAGE="${CONFORMANCE_WEBHOOK_IMAGE}" yq '
		select(.kind != "ConfigMap") |
		(select(.kind == "Deployment") | .spec.template.spec.priorityClassName) = "system-cluster-critical" |
		(select(.kind == "Deployment") | .spec.template.spec.containers[0]) |= (
			.image = strenv(IMAGE) | .imagePullPolicy = "IfNotPresent")
		' "${deploy_dir}/deploy/webhook.yaml" | kubectl -n "${webhook_ns}" apply -f -
	kubectl -n "${webhook_ns}" rollout status deployment "${webhook_name}-webhook" --timeout=300s

	namespaces="$(printf '"%s",' "${excluded_namespaces[@]}")"
	NAMESPACES="[${namespaces%,}]" yq '
		.webhooks[0].failurePolicy = "Fail" |
		.webhooks[0].namespaceSelector.matchExpressions = [{
			"key": "kubernetes.io/metadata.name",
			"operator": "NotIn",
			"values": env(NAMESPACES)}]
		' "${deploy_dir}/deploy/webhook-registration.yaml" | kubectl apply -f -
	check_webhook
}

# Wait until a probe pod is mutated: the registration and the webhook endpoint
# take a moment to become effective.
check_webhook() {
	local namespace="kata-conformance-probe"
	local mutated=""
	local expected="${CONFORMANCE_RUNTIME_CLASS} ${CONFORMANCE_DEFAULT_MEMORY_LIMIT}"
	local i

	kubectl create namespace "${namespace}" --dry-run=client -o yaml | kubectl apply -f - >/dev/null
	for i in $(seq 1 30); do
		mutated="$(kubectl create -n "${namespace}" --dry-run=server \
			-o jsonpath='{.spec.runtimeClassName} {.spec.resources.limits.memory}' \
			-f - 2>/dev/null <<-EOF || true
			apiVersion: v1
			kind: Pod
			metadata:
			  name: probe
			spec:
			  containers:
			    - name: probe
			      image: registry.k8s.io/pause:3.10
			      resources:
			        limits:
			          cpu: 100m
			EOF
		)"
		[[ "${mutated}" == "${expected}" ]] && break
		sleep 2
	done
	kubectl delete namespace "${namespace}" --wait=false >/dev/null
	[[ "${mutated}" == "${expected}" ]] || \
		die "Pods are not mutated by the kata-webhook (expected '${expected}', got '${mutated}') after ${i} attempts"
	info "Pods are assigned the ${CONFORMANCE_RUNTIME_CLASS} runtime class"
}

delete_webhook() {
	kubectl delete --ignore-not-found mutatingwebhookconfiguration "${webhook_name}-webhook" || true
	kubectl delete --ignore-not-found namespace "${webhook_ns}" --wait=false || true
}

run_ginkgo() {
	local name="${1}"
	local nodes="${2}"
	local focus="${3}"
	local skip="${4}"
	local bin_dir="${CONFORMANCE_WORK_DIR}/bin"
	local report_dir="${CONFORMANCE_REPORT_DIR}/${name}"

	mkdir -p "${report_dir}"
	info "Running the ${name} conformance tests (focus='${focus}', skip='${skip}')"
	"${bin_dir}/ginkgo" \
		--nodes="${nodes}" \
		--focus="${focus}" \
		${skip:+--skip="${skip}"} \
		--timeout=24h \
		--no-color \
		--silence-skips \
		--junit-report="junit_${name}.xml" \
		--output-dir="${report_dir}" \
		"${bin_dir}/e2e.test" -- \
		--kubeconfig="${KUBECONFIG}" \
		--provider=skeleton \
		--report-dir="${report_dir}" \
		2>&1 | tee "${report_dir}/e2e.log"
}

main() {
	local skip
	local serial_skip
	local rc=0

	command -v kubectl >/dev/null || die "kubectl is required"
	mkdir -p "${CONFORMANCE_WORK_DIR}" "${CONFORMANCE_REPORT_DIR}"

	install_e2e_binaries
	trap delete_webhook EXIT
	deploy_webhook

	skip="$(kata_skip_regex)"
	serial_skip="${skip}"
	skip+="${skip:+|}\[Serial\]"

	run_ginkgo "parallel" "${CONFORMANCE_PARALLEL_NODES}" "${CONFORMANCE_FOCUS}" "${skip}" || rc=1
	if [[ "${CONFORMANCE_RUN_SERIAL}" == "yes" ]]; then
		run_ginkgo "serial" 1 "\[Serial\].*${CONFORMANCE_FOCUS}|${CONFORMANCE_FOCUS}.*\[Serial\]" "${serial_skip}" || rc=1
	fi

	# common.bash traps ERR and prints the failing command. Returning the
	# suite status would be that command, and the trap evaluates it outside
	# this function, where rc is not set.
	trap - ERR
	return "${rc}"
}

main "$@"
