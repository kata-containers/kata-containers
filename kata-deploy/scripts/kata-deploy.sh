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

shims=(
	"fc"
	"qemu"
	"qemu-virtiofs"
	"cloud-hypervisor"
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

	local runtime=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}' | awk -F '[:]' '{print $1}')
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
		echo "$runtime"
	fi
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
	containerd | k3s | k3s-agent)
		configure_containerd
		;;
	esac
	systemctl daemon-reload
	systemctl restart "$1"
}

function configure_crio() {
	# Configure crio to use Kata:
	echo "Add Kata Containers as a supported runtime for CRIO:"

	# backup the CRIO.conf only if a backup doesn't already exist (don't override original)
	cp -n "$crio_conf_file" "$crio_conf_file_backup"

	local kata_clh_path="/opt/kata/bin/kata-clh"
	local kata_clh_conf="crio.runtime.runtimes.kata-clh"

	local kata_fc_path="/opt/kata/bin/kata-fc"
	local kata_fc_conf="crio.runtime.runtimes.kata-fc"

	local kata_qemu_path="/opt/kata/bin/kata-qemu"
	local kata_qemu_conf="crio.runtime.runtimes.kata-qemu"

	local kata_qemu_virtiofs_path="/opt/kata/bin/kata-qemu-virtiofs"
	local kata_qemu_virtiofs_conf="crio.runtime.runtimes.kata-qemu-virtiofs"

	# add kata-qemu config
	if grep -q "\[$kata_qemu_conf\]" $crio_conf_file; then
		echo "Configuration exists $kata_qemu_conf, overwriting"
		sed -i "/\[$kata_qemu_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_qemu_path}\"#" $crio_conf_file
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the QEMU hypervisor.
[$kata_qemu_conf]
  runtime_path = "${kata_qemu_path}"
EOT
	fi

        # add kata-qemu-virtiofs config
	if grep -q "\[$kata_qemu_virtiofs_conf\]" $crio_conf_file; then
		echo "Configuration exists $kata_qemu_virtiofs_conf, overwriting"
		sed -i "/\[$kata_qemu_virtiofs_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_qemu_virtiofs_path}\"#" $crio_conf_file
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the QEMU hypervisor with virtiofs support.
[$kata_qemu_virtiofs_conf]
  runtime_path = "${kata_qemu_virtiofs_path}"
EOT
        fi

	# add kata-fc config
	if grep -q "\[$kata_fc_conf\]" $crio_conf_file; then
		echo "Configuration exists for $kata_fc_conf, overwriting"
		sed -i "/\[$kata_fc_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_fc_path}\"#" $crio_conf_file
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the firecracker hypervisor.
[$kata_fc_conf]
  runtime_path = "${kata_fc_path}"
EOT
	fi

	# add kata-clh config
	if grep -q "\[$kata_clh_conf\]" $crio_conf_file; then
		echo "Configuration exists $kata_clh_conf, overwriting"
		sed -i "/\[$kata_clh_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_clh_path}\"#" $crio_conf_file
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the Cloud Hypervisor.
[$kata_clh_conf]
  runtime_path = "${kata_clh_path}"
EOT
	fi

	# Replace if exists, insert otherwise
	grep -Fq 'manage_network_ns_lifecycle =' $crio_conf_file \
		&& sed -i '/manage_network_ns_lifecycle =/c manage_network_ns_lifecycle = true' $crio_conf_file \
		|| sed -i '/\[crio.runtime\]/a manage_network_ns_lifecycle = true' $crio_conf_file
}

function configure_containerd_runtime() {
	local runtime="kata"
	local configuration="configuration"
	if [ -n "${1-}" ]; then
		if [ "$1" == "cloud-hypervisor" ]; then
			runtime+="-clh"
			configuration+="-clh"
		else
			runtime+="-$1"
			configuration+="-$1"
		fi
	fi
	local runtime_table="plugins.cri.containerd.runtimes.$runtime"
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

	#Currently containerd has an assumption on the location of the shimv2 implementation
	#Until support is added (see https://github.com/containerd/containerd/issues/3073),
	#create a link in /usr/local/bin/ to the v2-shim implementation in /opt/kata/bin.

	mkdir -p /usr/local/bin

	for shim in "${shims[@]}"; do
		configure_containerd_runtime $shim

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
KATA_CONF_FILE=/opt/kata/share/defaults/kata-containers/configuration-${shim}.toml /opt/kata/bin/containerd-shim-kata-v2 \$@
EOT
	chmod +x "$shim_file"
	done
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
	containerd | k3s | k3s-agent)
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
	rm -f $containerd_conf_file
	if [ -f "$containerd_conf_file_backup" ]; then
		mv "$containerd_conf_file_backup" "$containerd_conf_file"
	fi

	#Currently containerd has an assumption on the location of the shimv2 implementation
	#Until support is added (see https://github.com/containerd/containerd/issues/3073), we manage
	# a reference to the v2-shim implementation

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
