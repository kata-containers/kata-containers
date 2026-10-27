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
declare -A kata_skips=(
	["should have at least two untainted nodes"]="asserts the cluster has two schedulable nodes"
	["should rollback without unnecessary restarts"]="DaemonSet rollback needs a second node to roll onto"
)

kata_skip_regex() {
	local regex=""
	local test_name

	for test_name in "${!kata_skips[@]}"; do
		regex+="${regex:+|}${test_name}"
	done
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
