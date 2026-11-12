#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

[[ $# == 2 ]] || { echo "Usage: $0 run|report CHECKOUT" >&2; exit 1; }
mode=$1
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
checkout=$(realpath "$2")
assets=${SUBSTRATE_ASSET_DIR:-}
cd "${checkout}"
prepare_host() {
	[[ -c /dev/kvm ]] || { echo 'microVM e2e tests require /dev/kvm' >&2; exit 1; }
	sudo chmod a+rw /dev/kvm
	sudo modprobe nfs
	sudo modprobe nfsd
}

deploy_substrate() {
	bash hack/create-kind-cluster.sh
	go run ./cmd/ate-setup --kind deploy ate-system
	bash "${script_dir}/stage-artifacts.sh" "${assets}"
	go run ./cmd/ate-setup --kind setup csi nfs
	go run ./cmd/ate-setup --kind deploy demo counter-microvm
	go run ./cmd/ate-setup --kind deploy demo egress-microvm
	bash hack/setup-e2e-egress-tls-kind.sh
	kubectl wait deployment --all-namespaces -l ate.dev/worker-pool --for=condition=Available --timeout=300s
}

run_e2e() {
	bash "${script_dir}/verify-artifacts.sh" "${assets}" before
	local suite_status=0 verify_status=0
	E2E_SANDBOX_CLASS=microvm bash hack/run-e2e-kind.sh -args --no-color --preview="${ATE_PREVIEW:-}" || suite_status=$?
	bash "${script_dir}/verify-artifacts.sh" "${assets}" after || verify_status=$?
	if (( suite_status != 0 )); then
		return "${suite_status}"
	fi
	return "${verify_status}"
}

case "${mode}" in
	run)
		(cd "${assets:?SUBSTRATE_ASSET_DIR is required}"; sha256sum -c SHA256SUMS)
		git log -1 --format='Testing Substrate %H'
		prepare_host
		deploy_substrate
		run_e2e
		;;
	report)
		go -C tools/junittool run . verify -manifest "${ARTIFACTS:?}/expected-junit.txt"
		go -C tools/junittool run . report "${ARTIFACTS}"
		;;
	*) echo "Unknown e2e command: ${mode}" >&2; exit 1 ;;
esac
