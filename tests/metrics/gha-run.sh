#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

kata_tarball_dir="${2:-kata-artifacts}"
metrics_dir="$(dirname "$(readlink -f "$0")")"
source "${metrics_dir}/../common.bash"
source "${metrics_dir}/lib/common.bash"

declare -r results_dir="${metrics_dir}/results"
declare -r checkmetrics_dir="${metrics_dir}/cmd/checkmetrics"
declare -r checkmetrics_config_dir="${checkmetrics_dir}/ci_worker"

function install_checkmetrics() {
	# Ensure we have the latest checkmetrics
	pushd "${checkmetrics_dir}"
	make
	sudo make install
	popd
}

# @path_results: path to the input metric-results folder
# @tarball_fname: path and filename to the output tarball
function compress_metrics_results_dir()
{
	local path_results="${1:-results}"
	local tarball_fname="${2:-}"

	[ -z "${tarball_fname}" ] && die "Missing the tarball filename or the path to save the tarball results is incorrect."
	[ ! -d "${path_results}" ] && die "Missing path to the results folder."

	cd "${path_results}" && tar -czf "${tarball_fname}" *.json && cd -
	info "tarball generated: ${tarball_fname}"
}

function check_metrics() {
	local cm_base_file="${checkmetrics_config_dir}/checkmetrics-json-${KATA_HYPERVISOR}-kata-metric8.toml"
	checkmetrics --debug --percentage --basefile "${cm_base_file}" --metricsdir "${results_dir}"
	cm_result=$?
	if [ "${cm_result}" != 0 ]; then
		die "run-metrics-ci: checkmetrics FAILED (${cm_result})"
	fi
}

function make_tarball_results() {
	compress_metrics_results_dir "${metrics_dir}/results" "${GITHUB_WORKSPACE}/results-${KATA_HYPERVISOR}.tar.gz"
}

function run_test_launchtimes() {
	info "Running Launch Time test using ${KATA_HYPERVISOR} hypervisor"

	create_symbolic_links ${KATA_HYPERVISOR}
	bash tests/metrics/time/launch_times.sh -i public.ecr.aws/ubuntu/ubuntu:latest -n 20
}

function run_test_memory_usage() {
	info "Running memory-usage test using ${KATA_HYPERVISOR} hypervisor"

	create_symbolic_links ${KATA_HYPERVISOR}
	bash tests/metrics/density/memory_usage.sh 20 5
}

function run_test_memory_usage_inside_container() {
	info "Running memory-usage inside the container test using ${KATA_HYPERVISOR} hypervisor"

	create_symbolic_links ${KATA_HYPERVISOR}
	bash tests/metrics/density/memory_usage_inside_container.sh 5
}

function run_test_blogbench() {
	info "Running Blogbench test using ${KATA_HYPERVISOR} hypervisor"

	create_symbolic_links ${KATA_HYPERVISOR}
	bash tests/metrics/storage/blogbench.sh

	check_metrics
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-kata) install_kata ;;
		make-tarball-results) make_tarball_results ;;
		run-test-launchtimes) run_test_launchtimes ;;
		run-test-memory-usage) run_test_memory_usage ;;
		run-test-memory-usage-inside-container) run_test_memory_usage_inside_container ;;
		run-test-blogbench) run_test_blogbench ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
