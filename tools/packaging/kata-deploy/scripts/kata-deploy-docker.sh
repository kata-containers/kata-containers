#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o pipefail
set -o nounset

conf_file="/etc/docker/daemon.json"
conf_file_backup="${conf_file}.bak"
snippet="${conf_file}.snip"
tmp_file="${conf_file}.tmp"

# If we fail for any reason a message will be displayed
die() {
        msg="$*"
        echo "ERROR: $msg" >&2
        exit 1
}

function print_usage() {
	echo "Usage: $0 [install/remove]"
}

function install_artifacts() {
	echo "copying kata artifacts onto host"
	cp -a /opt/kata-artifacts/opt/kata/* /opt/kata/
	chmod +x /opt/kata/bin/*
}

function configure_docker() {
	echo "configuring docker"

	cat <<EOT | tee -a "$snippet"
{
  "runtimes": {
    "kata-qemu": {
      "path": "/opt/kata/bin/kata-runtime",
      "runtimeArgs": [ "--kata-config", "/opt/kata/share/defaults/kata-containers/configuration-qemu.toml" ]
    },
    "kata-qemu-virtiofs": {
      "path": "/opt/kata/bin/kata-runtime",
      "runtimeArgs": [ "--kata-config", "/opt/kata/share/defaults/kata-containers/configuration-qemu-virtiofs.toml" ]
    },
     "kata-fc": {
      "path": "/opt/kata/bin/kata-runtime",
      "runtimeArgs": [ "--kata-config", "/opt/kata/share/defaults/kata-containers/configuration-fc.toml" ]
    },
     "kata-clh": {
      "path": "/opt/kata/bin/kata-runtime",
      "runtimeArgs": [ "--kata-config", "/opt/kata/share/defaults/kata-containers/configuration-clh.toml" ]
    },
     "kata-acrn": {
      "path": "/opt/kata/bin/kata-runtime",
      "runtimeArgs": [ "--kata-config", "/opt/kata/share/defaults/kata-containers/configuration-acrn.toml" ]
    }
  }
}
EOT
	if [ -f ${conf_file} ]; then
		cp -n "$conf_file" "$conf_file_backup"

		# Merge in the json snippet:
		jq -s '[.[] | to_entries] | flatten | reduce .[] as $dot ({}; .[$dot.key] += $dot.value)' "${conf_file}" "${snippet}" > "${tmp_file}"
		mv "${tmp_file}" "${conf_file}"
		rm "${snippet}"
	else
		mv "${snippet}" "${conf_file}"
	fi

	systemctl daemon-reload
	systemctl reload docker
}

function remove_artifacts() {
	echo "deleting kata artifacts"
	rm -rf /opt/kata/
}

function cleanup_runtime() {
	echo "cleanup docker"
	rm -f "${conf_file}"

	if [ -f "${conf_file_backup}" ]; then
		cp "${conf_file_backup}"  "${conf_file}"
	fi
	systemctl daemon-reload
	systemctl reload docker
}

function main() {
	# script requires that user is root
	euid=`id -u`
	if [[ $euid -ne 0 ]]; then
	   die  "This script must be run as root"
	fi

	action=${1:-}
	if [ -z $action ]; then
		print_usage
		die "invalid arguments"
	fi

		case $action in
		install)
			install_artifacts
			configure_docker
			;;
		remove)
			cleanup_runtime
			remove_artifacts
			;;
		*)
			echo invalid arguments
			print_usage
			;;
		esac
}


main $@
