#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

kata_tarball_dir=${2:-kata-artifacts}
metrics_dir="$(dirname "$(readlink -f "$0")")"

create_symbolic_links() {
	hypervisor="${1:-qemu}"
	local link_configuration_file="/opt/kata/share/defaults/kata-containers/configuration.toml"
	local source_configuration_file="/opt/kata/share/defaults/kata-containers/configuration-${hypervisor}.toml"

	if [ ${hypervisor} != 'qemu' ] && [ ${hypervisor} != 'clh' ]; then
		exit 2
	fi

	sudo ln -sf "${source_configuration_file}" "${link_configuration_file}"
}

install_kata() {
	local katadir="/opt/kata"
	local destdir="/"

	# Removing previous kata installation
	sudo rm -rf "${katadir}"

	pushd ${kata_tarball_dir}
	for c in kata-static-*.tar.xz; do
		echo "untarring tarball "${c}" into ${destdir}"
		sudo tar -xvf "${c}" -C "${destdir}"
	done
	popd
}

function run_test_launchtimes() {
	hypervisor="${1}"

	echo "Running launchtimes tests: "

	create_symbolic_links "${hypervisor}"

	if [ "${hypervisor}" = 'qemu' ]; then
		echo "qemu"
	elif [ "${hypervisor}" = 'clh' ]; then
		echo "clh"
	fi
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-kata) install_kata ;;
		run-test-launchtimes-qemu) run_test_launchtimes "qemu" ;;
		run-test-launchtimes-clh) run_test_launchtimes "clh" ;;
		*) >&2 echo "Invalid argument"; exit 2 ;;
	esac
}

main "$@"
