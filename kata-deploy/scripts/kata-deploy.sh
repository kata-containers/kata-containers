#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o pipefail
set -o nounset

crio_conf_file="/etc/crio/crio.conf"
crio_conf_file_backup="${crio_conf_file}.bak"
containerd_conf_file="/etc/containerd/config.toml"
containerd_conf_file_backup="${containerd_conf_file}.bak"
# If we fail for any reason a message will be displayed
die() {
        msg="$*"
        echo "ERROR: $msg" >&2
        exit 1
}

function print_usage() {
	echo "Usage: $0 [install/cleanup/reset]"
}

function get_container_runtime() {
	local runtime=$(kubectl describe node $NODE_NAME)
	if [ "$?" -ne 0 ]; then
                die "invalid node name"
	fi
	echo "$runtime" | awk -F'[:]' '/Container Runtime Version/ {print $2}' | tr -d ' '
}

function install_artifacts() {
	echo "copying kata artifacts onto host"
	cp -a /opt/kata-artifacts/opt/kata/* /opt/kata/
	chmod +x /opt/kata/bin/*
}

function configure_cri_runtime() {
	case $1 in
	crio)
		configure_crio
		;;
	containerd)
		configure_containerd
		;;
	esac
	systemctl daemon-reload
	systemctl restart $1
}

function configure_crio() {
	# Configure crio to use Kata:
	echo "Add Kata Containers as a supported runtime for CRIO:"

	# backup the CRIO.conf only if a backup doesn't already exist (don't override original)
	cp -n "$crio_conf_file" "$crio_conf_file_backup"

	cat <<EOT | tee -a "$crio_conf_file"
[crio.runtime.runtimes.kata-qemu]
  runtime_path = "/opt/kata/bin/kata-qemu"

[crio.runtime.runtimes.kata-fc]
  runtime_path = "/opt/kata/bin/kata-fc"
EOT

  # Replace if exists, insert otherwise
  grep -Fq 'manage_network_ns_lifecycle =' $crio_conf_file \
  && sed -i '/manage_network_ns_lifecycle =/c manage_network_ns_lifecycle = true' $crio_conf_file \
  || sed -i '/\[crio.runtime\]/a manage_network_ns_lifecycle = true' $crio_conf_file
}

function configure_containerd() {
	# Configure containerd to use Kata:
	echo "Add Kata Containers as a supported runtime for containerd"
	mkdir -p /etc/containerd/

	if [ -f "$containerd_conf_file" ]; then
		cp "$containerd_conf_file" "$containerd_conf_file_backup"
	fi
	# TODO: While there isn't a default here anyway, it'd probably be best to
	#  add sed magic to insert into appropriate location if config.toml already exists
	# https://github.com/kata-containers/packaging/issues/307
	cat <<EOT | tee "$containerd_conf_file"
[plugins]
    [plugins.cri.containerd]
      [plugins.cri.containerd.untrusted_workload_runtime]
        runtime_type = "io.containerd.runtime.v1.linux"
        runtime_engine = "/opt/kata/bin/kata-runtime"
        runtime_root = ""
EOT
}

function remove_artifacts() {
	echo "deleting kata artifacts"
	rm -rf /opt/kata/
}

function cleanup_cri_runtime() {
	case $1 in
	crio)
		cleanup_crio
		;;
	containerd)
		cleanup_containerd
		;;
	esac

}
function cleanup_crio() {
	if [ -f "$crio_conf_file_backup" ]; then
		cp "$crio_conf_file_backup" "$crio_conf_file"
	fi
}

function cleanup_containerd() {
	rm -f /etc/containerd/config.toml
	if [ -f "$containerd_conf_file_backup" ]; then
		mv "$containerd_conf_file_backup" "$containerd_conf_file"
	fi

}

function reset_runtime() {
	kubectl label node $NODE_NAME katacontainers.io/kata-runtime-
	systemctl daemon-reload
	systemctl restart $1
	systemctl restart kubelet
}

function main() {
	# script requires that user is root
	euid=`id -u`
	if [[ $euid -ne 0 ]]; then
	   die  "This script must be run as root"
	fi

	runtime=$(get_container_runtime)

	# CRI-O isn't consistent with the naming -- let's use crio to match the service file
	if [ "$runtime" == "cri-o" ]; then
		runtime="crio"
	fi

	action=${1:-}
	if [ -z $action ]; then
		print_usage
		die "invalid arguments"
	fi

	# only install / remove / update if we are dealing with CRIO or containerd
	if [ "$runtime" == "crio" ] || [ "$runtime" == "containerd" ]; then

		case $action in
		install)

			install_artifacts
			configure_cri_runtime $runtime
			;;
		cleanup)
			remove_artifacts
			cleanup_cri_runtime $runtime
			kubectl label node $NODE_NAME --overwrite katacontainers.io/kata-runtime=cleanup
			;;
		reset)
			reset_runtime $runtime
			;;
		*)
			echo invalid arguments
			print_usage
			;;
		esac
	fi

	#It is assumed this script will be called as a daemonset. As a result, do
        # not return, otherwise the daemon will restart and rexecute the script
	sleep infinity
}

main $@
