#!/bin/bash
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Checks that runtime-rs exports traces: runs a Kata container with tracing
# enabled and waits for its spans to reach a local Jaeger.
#

# KATA_HYPERVISOR and KATA_CONFIG_PATH are set by common.bash and enabling_hypervisor.
# shellcheck disable=SC2154

set -o errexit
set -o nounset
set -o pipefail

# shellcheck disable=SC2034
kata_tarball_dir="${2:-kata-artifacts}"
tracing_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
source "${tracing_dir}/../../common.bash"

jaeger_image="jaegertracing/jaeger:2.22.0"
jaeger_name="kata-tracing-jaeger"
otlp_endpoint="http://localhost:4317"
jaeger_api="http://localhost:16686/api/v3"

container_image="quay.io/prometheus/busybox:latest"
container_name="kata-tracing-test"

dropin=""

function install_dependencies() {
	bash "${tracing_dir}/../../integration/nerdctl/gha-run.sh" install-dependencies
}

function start_jaeger() {
	info "Starting Jaeger"
	sudo nerdctl run -d --name "${jaeger_name}" --net host "${jaeger_image}" > /dev/null
	waitForProcess 60 1 "curl -sf ${jaeger_api}/services > /dev/null" ||
		die "Jaeger did not start"
}

function enable_tracing() {
	dropin="$(dirname "${KATA_CONFIG_PATH}")/config.d/90-tracing.toml"
	info "Enabling runtime tracing in ${dropin}"
	sudo mkdir -p "$(dirname "${dropin}")"
	sudo tee "${dropin}" > /dev/null <<-EOF
	[runtime]
	enable_tracing = true
	otlp_endpoint = "${otlp_endpoint}"
	EOF
}

# Prints the container ID.  The container keeps running so that its sandbox
# exports spans while the test waits for them.
function start_kata_container() {
	sudo nerdctl run -d --name "${container_name}" \
		--runtime "io.containerd.kata-${KATA_HYPERVISOR}.v2" \
		"${container_image}" sleep infinity
}

# Jaeger only lists a service once it has received spans from it.
function wait_for_traces() {
	local service="$1"

	info "Waiting for traces from ${service}"
	waitForProcess 60 1 "curl -s ${jaeger_api}/services | grep -q '\"${service}\"'" ||
		die "No traces received from ${service}"
	info "Traces received from ${service}"
}

function cleanup() {
	sudo nerdctl rm -f "${container_name}" "${jaeger_name}" &> /dev/null || true
	if [[ -n "${dropin}" ]]; then
		sudo rm -f "${dropin}"
	fi
}

function run() {
	enabling_hypervisor
	trap cleanup EXIT

	start_jaeger
	enable_tracing

	info "Running a container with ${KATA_HYPERVISOR}"
	local cid
	cid=$(start_kata_container)

	# runtime-rs reports each sandbox as service "kata-sb-<first 8 chars of its ID>".
	wait_for_traces "kata-sb-${cid:0:8}"
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		install-kata) install_kata ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
