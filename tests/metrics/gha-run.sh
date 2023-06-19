#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

metrics_dir="$(dirname "$(readlink -f "$0")")"

function run_test_launchtimes() {
	hypervisor="${1}"

	echo "Running launchtimes tests: "

	if [ "${hypervisor}" = 'qemu' ]; then
		echo "qemu"
		echo "Check kata installation"
		kata-runtime kata-env
		echo "Kata config:"
		cat $(kata-runtime kata-env  --json | jq .Runtime.Config.Path -r)
	elif [ "${hypervisor}" = 'clh' ]; then
		echo "clh"
	fi
}

function main() {
	action="${1:-}"
	case "${action}" in
		run-test-launchtimes-qemu) run_test_launchtimes "qemu" ;;
		run-test-launchtimes-clh) run_test_launchtimes "clh" ;;
		*) >&2 echo "Invalid argument"; exit 2 ;;
	esac
}

main "$@"
