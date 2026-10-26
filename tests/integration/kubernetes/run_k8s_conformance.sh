#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Run the upstream Kubernetes conformance suite (e2e.test) against a cluster
# where Kata Containers has been deployed by kata-deploy.

set -o errexit
set -o nounset
set -o pipefail

kubernetes_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
source "${kubernetes_dir}/../../common.bash"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-runtime-rs}"
KUBECONFIG="${KUBECONFIG:-${HOME}/.kube/config}"
export KUBECONFIG

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
