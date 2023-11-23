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

	bash tests/metrics/time/launch_times.sh -i public.ecr.aws/ubuntu/ubuntu:latest -n 20
}

function run_test_memory_usage() {
	info "Running memory-usage test using ${KATA_HYPERVISOR} hypervisor"

	bash tests/metrics/density/memory_usage.sh 20 5
}

function run_test_memory_usage_inside_container() {
	info "Running memory-usage inside the container test using ${KATA_HYPERVISOR} hypervisor"

	bash tests/metrics/density/memory_usage_inside_container.sh 5
}

function run_test_blogbench() {
	if [ "${KATA_HYPERVISOR}" = "stratovirt" ]; then
		exit 0
	fi
	info "Running Blogbench test using ${KATA_HYPERVISOR} hypervisor"

	bash tests/metrics/storage/blogbench.sh
}

function run_test_tensorflow() {
	if [ "${KATA_HYPERVISOR}" = "stratovirt" ]; then
		exit 0
	fi
	info "Running TensorFlow test using ${KATA_HYPERVISOR} hypervisor"

	bash tests/metrics/machine_learning/tensorflow_nhwc.sh 1 20
}

function run_test_fio() {
	if [ "${KATA_HYPERVISOR}" = "stratovirt" ]; then
		exit 0
	fi
	info "Running FIO test using ${KATA_HYPERVISOR} hypervisor"

	bash tests/metrics/storage/fio_test.sh
}

function run_test_iperf() {
	if [ "${KATA_HYPERVISOR}" = "stratovirt" ]; then
		exit 0
	fi
	info "Running Iperf test using ${KATA_HYPERVISOR} hypervisor"

	bash tests/metrics/network/iperf3_kubernetes/k8s-network-metrics-iperf3.sh -a
}

function run_test_latency() {
	if [ "${KATA_HYPERVISOR}" = "stratovirt" ]; then
		exit 0
	fi
	info "Running Latency test using ${KATA_HYPERVISOR} hypervisor"

	bash tests/metrics/network/latency_kubernetes/latency-network.sh

	check_metrics
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-kata) install_kata && install_checkmetrics ;;
		enabling-hypervisor) enabling_hypervisor ;;
		make-tarball-results) make_tarball_results ;;
		run-test-launchtimes) run_test_launchtimes ;;
		run-test-memory-usage) run_test_memory_usage ;;
		run-test-memory-usage-inside-container) run_test_memory_usage_inside_container ;;
		run-test-blogbench) run_test_blogbench ;;
		run-test-tensorflow) run_test_tensorflow ;;
		run-test-fio) run_test_fio ;;
		run-test-iperf) run_test_iperf ;;
		run-test-latency) run_test_latency ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
