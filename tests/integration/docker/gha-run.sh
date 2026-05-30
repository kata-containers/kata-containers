#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

# shellcheck disable=SC2034
kata_tarball_dir="${2:-kata-artifacts}"
docker_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
source "${docker_dir}/../../common.bash"
image="${image:-instrumentisto/nmap:latest}"

function dump_kata_debug() {
	local -r kata_runtime="$1"

	info "Collecting debug data for ${kata_runtime}"

	info "Docker runtime view"
	sudo docker info --format '{{json .Runtimes}}' || true
	sudo docker info --format 'Default runtime: {{.DefaultRuntime}}' || true
	[[ -f /etc/docker/daemon.json ]] && sudo cat /etc/docker/daemon.json || true

	info "Containerd runtime configuration"
	sudo sed -n '/containerd.runtimes/,+50p' /etc/containerd/config.toml || true

	info "Kata runtime config symlinks"
	[[ -e /opt/kata/share/defaults/kata-containers/configuration.toml ]] && \
		sudo ls -l /opt/kata/share/defaults/kata-containers/configuration.toml || true
	[[ -e /opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml ]] && \
		sudo ls -l /opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml || true

	info "Recent containerd/kata logs"
	sudo journalctl -u containerd --no-pager -n 200 || true
	sudo journalctl --no-pager -n 400 | grep -E 'kata|containerd-shim-kata|io.containerd.kata' || true
}

function install_dependencies() {
	info "Installing the dependencies needed for running the docker smoke test"

	sudo -E docker pull "${image}"
}

function run() {
	# shellcheck disable=SC2154
	local -r kata_runtime="io.containerd.kata-${KATA_HYPERVISOR}.v2"

	info "Running docker smoke test tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor

	info "Running docker with runc"
	sudo docker run --rm --entrypoint nping "${image}" --tcp-connect -c 2 -p 80 www.github.com

	info "Running docker with Kata Containers (${KATA_HYPERVISOR})"
	if ! sudo docker run --rm --runtime "${kata_runtime}" --entrypoint nping "${image}" --tcp-connect -c 2 -p 80 www.github.com; then
		dump_kata_debug "${kata_runtime}"
		return 1
	fi
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
