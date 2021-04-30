#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o pipefail
set -o nounset

crio_drop_in_conf_dir="/etc/crio/crio.conf.d/"
crio_drop_in_conf_file="${crio_drop_in_conf_dir}/99-kata-deploy"
containerd_conf_file="/etc/containerd/config.toml"
containerd_conf_file_backup="${containerd_conf_file}.bak"

shims=(
	"fc"
	"qemu"
	"clh"
)

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

	local runtime=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
	if [ "$?" -ne 0 ]; then
                die "invalid node name"
	fi
	if echo "$runtime" | grep -qE 'containerd.*-k3s'; then
		if systemctl is-active --quiet k3s-agent; then
			echo "k3s-agent"
		else
			echo "k3s"
		fi
	else
		echo "$runtime" | awk -F '[:]' '{print $1}'
	fi
}

function install_artifacts() {
	echo "copying kata artifacts onto host"
	cp -a /opt/kata-artifacts/opt/kata/* /opt/kata/
	chmod +x /opt/kata/bin/*
}

function configure_cri_runtime() {
	configure_different_shims_base

	case $1 in
	crio)
		configure_crio
		;;
	containerd | k3s | k3s-agent)
		configure_containerd
		;;
	esac
	systemctl daemon-reload
	systemctl restart "$1"
}

function configure_different_shims_base() {
	# Currently containerd has an assumption on the location of the shimv2 implementation
	# This forces kata-deploy to create files in a well-defined location that's part of
	# the PATH, pointing to the containerd-shim-kata-v2 binary in /opt/kata/bin
	# Issues:
	#   https://github.com/containerd/containerd/issues/3073
	#   https://github.com/containerd/containerd/issues/5006

	mkdir -p /usr/local/bin

	for shim in "${shims[@]}"; do
		local shim_binary="containerd-shim-kata-${shim}-v2"
		local shim_file="/usr/local/bin/${shim_binary}"
		local shim_backup="/usr/local/bin/${shim_binary}.bak"

		if [ -f "${shim_file}" ]; then
			echo "warning: ${shim_binary} already exists" >&2
			if [ ! -f "${shim_backup}" ]; then
				mv "${shim_file}" "${shim_backup}"
			else
				rm "${shim_file}"
			fi
		fi

		cat << EOT | tee "$shim_file"
#!/bin/bash
KATA_CONF_FILE=/opt/kata/share/defaults/kata-containers/configuration-${shim}.toml /opt/kata/bin/containerd-shim-kata-v2 "\$@"
EOT
		chmod +x "$shim_file"
	done
}

function cleanup_different_shims_base() {
	for shim in "${shims[@]}"; do
		local shim_binary="containerd-shim-kata-${shim}-v2"
		local shim_file="/usr/local/bin/${shim_binary}"
		local shim_backup="/usr/local/bin/${shim_binary}.bak"

		rm "${shim_file}" || true

		if [ -f "${shim_backup}" ]; then
			mv "$shim_backup" "$shim_file"
		fi
	done
}

function configure_crio_runtime() {
	local runtime="kata"
	if [ -n "${1-}" ]; then
		runtime+="-$1"
	fi

	local kata_path="/usr/local/bin/containerd-shim-${runtime}-v2"
	local kata_conf="crio.runtime.runtimes.${runtime}"

	cat <<EOT | tee -a "$crio_drop_in_conf_file"

# Path to the Kata Containers runtime binary that uses the $1
[$kata_conf]
	runtime_path = "${kata_path}"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	privileged_without_host_devices = true
EOT
}

function configure_crio() {
	# Configure crio to use Kata:
	echo "Add Kata Containers as a supported runtime for CRIO:"

	# As we don't touch the original configuration file in any way,
	# let's just ensure we remove any exist configuration from a
	# previous deployment.
	mkdir -p "$crio_drop_in_conf_dir"
	rm -f "$crio_drop_in_conf_file"
	touch "$crio_drop_in_conf_file"

	for shim in "${shims[@]}"; do
		configure_crio_runtime $shim
	done
}

function configure_containerd_runtime() {
	local runtime="kata"
	local configuration="configuration"
	if [ -n "${1-}" ]; then
		runtime+="-$1"
		configuration+="-$1"
	fi
	local pluginid=cri
	if grep -q "version = 2\>" $containerd_conf_file; then
		pluginid=\"io.containerd.grpc.v1.cri\"
	fi
	local runtime_table="plugins.${pluginid}.containerd.runtimes.$runtime"
	local runtime_type="io.containerd.$runtime.v2"
	local options_table="$runtime_table.options"
	local config_path="/opt/kata/share/defaults/kata-containers/$configuration.toml"
	if grep -q "\[$runtime_table\]" $containerd_conf_file; then
		echo "Configuration exists for $runtime_table, overwriting"
		sed -i "/\[$runtime_table\]/,+1s#runtime_type.*#runtime_type = \"${runtime_type}\"#" $containerd_conf_file
	else
		cat <<EOT | tee -a "$containerd_conf_file"
[$runtime_table]
  runtime_type = "${runtime_type}"
  privileged_without_host_devices = true
  pod_annotations = ["io.katacontainers.*"]
EOT
	fi

	if grep -q "\[$options_table\]" $containerd_conf_file; then
		echo "Configuration exists for $options_table, overwriting"
		sed -i "/\[$options_table\]/,+1s#ConfigPath.*#ConfigPath = \"${config_path}\"#" $containerd_conf_file
	else
		cat <<EOT | tee -a "$containerd_conf_file"
  [$options_table]
    ConfigPath = "${config_path}"
EOT
	fi
}

function configure_containerd() {
	# Configure containerd to use Kata:
	echo "Add Kata Containers as a supported runtime for containerd"

	mkdir -p /etc/containerd/

	if [ -f "$containerd_conf_file" ]; then
		# backup the config.toml only if a backup doesn't already exist (don't override original)
		cp -n "$containerd_conf_file" "$containerd_conf_file_backup"
	fi

	# Add default Kata runtime configuration
	configure_containerd_runtime

	for shim in "${shims[@]}"; do
		configure_containerd_runtime $shim
	done
}

function remove_artifacts() {
	echo "deleting kata artifacts"
	rm -rf /opt/kata/
}

function cleanup_cri_runtime() {
	cleanup_different_shims_base

	case $1 in
	crio)
		cleanup_crio
		;;
	containerd | k3s | k3s-agent)
		cleanup_containerd
		;;
	esac

}

function cleanup_crio() {
	rm $crio_drop_in_conf_file
}

function cleanup_containerd() {
	rm -f $containerd_conf_file
	if [ -f "$containerd_conf_file_backup" ]; then
		mv "$containerd_conf_file_backup" "$containerd_conf_file"
	fi
}

function reset_runtime() {
	kubectl label node "$NODE_NAME" katacontainers.io/kata-runtime-
	systemctl daemon-reload
	systemctl restart "$1"
	if [ "$1" == "crio" ] || [ "$1" == "containerd" ]; then
		systemctl restart kubelet
	fi
}

function main() {
	# script requires that user is root
	euid=$(id -u)
	if [[ $euid -ne 0 ]]; then
	   die  "This script must be run as root"
	fi

	runtime=$(get_container_runtime)

	# CRI-O isn't consistent with the naming -- let's use crio to match the service file
	if [ "$runtime" == "cri-o" ]; then
		runtime="crio"
	elif [ "$runtime" == "k3s" ] || [ "$runtime" == "k3s-agent" ]; then
		containerd_conf_tmpl_file="${containerd_conf_file}.tmpl"
		if [ ! -f "$containerd_conf_tmpl_file" ]; then
			cp "$containerd_conf_file" "$containerd_conf_tmpl_file"
		fi

		containerd_conf_file="${containerd_conf_tmpl_file}"
		containerd_conf_file_backup="${containerd_conf_file}.bak"
	fi

	action=${1:-}
	if [ -z "$action" ]; then
		print_usage
		die "invalid arguments"
	fi

	# only install / remove / update if we are dealing with CRIO or containerd
	if [[ "$runtime" =~ ^(crio|containerd|k3s|k3s-agent)$ ]]; then

		case "$action" in
		install)

			install_artifacts
			configure_cri_runtime "$runtime"
			kubectl label node "$NODE_NAME" --overwrite katacontainers.io/kata-runtime=true
			;;
		cleanup)
			cleanup_cri_runtime "$runtime"
			kubectl label node "$NODE_NAME" --overwrite katacontainers.io/kata-runtime=cleanup
			remove_artifacts
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

main "$@"
