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
	"qemu"
	"nemu"
	"fc"
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

	local kata_qemu_path="/opt/kata/bin/kata-qemu"
	local kata_qemu_virtiofs_path="/opt/kata/bin/kata-qemu-virtiofs"
	local kata_nemu_path="/opt/kata/bin/kata-nemu"
	local kata_fc_path="/opt/kata/bin/kata-fc"
	local kata_qemu_conf="crio.runtime.runtimes.kata-qemu"
	local kata_qemu_virtiofs_conf="crio.runtime.runtimes.kata-qemu-virtiofs"
	local kata_nemu_conf="crio.runtime.runtimes.kata-nemu"
	local kata_fc_conf="crio.runtime.runtimes.kata-fc"

	# add kata-qemu config
	if grep -q "^\[$kata_qemu_conf\]" $crio_conf_file; then
		echo "Configuration exists $kata_qemu_conf, overwriting"
		sed -i "/^\[$kata_qemu_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_qemu_path}\"#" $crio_conf_file 
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the QEMU hypervisor.
[$kata_qemu_conf]
  runtime_path = "${kata_qemu_path}"
EOT
	fi

        # add kata-qemu-virtiofs config
	if grep -q "^\[$kata_qemu_virtiofs_conf\]" $crio_conf_file; then
		echo "Configuration exists $kata_qemu_virtiofs_conf, overwriting"
		sed -i "/^\[$kata_qemu_virtiofs_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_qemu_path}\"#" $crio_conf_file
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the QEMU hypervisor.
[$kata_qemu_conf]
  runtime_path = "${kata_qemu_virtiofs_path}"
EOT
        fi


	# add kata-nemu config
	if grep -q "^\[$kata_nemu_conf\]" $crio_conf_file; then
		echo "Configuration exists $kata_nemu_conf, overwriting"
		sed -i "/^\[$kata_nemu_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_nemu_path}\"#" $crio_conf_file
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the NEMU hypervisor.
[$kata_nemu_conf]
  runtime_path = "${kata_nemu_path}"
EOT
	fi

	# add kata-fc config
	if grep -q "^\[$kata_fc_conf\]" $crio_conf_file; then
		echo "Configuration exists for $kata_fc_conf, overwriting"
		sed -i "/^\[$kata_fc_conf\]/,+1s#runtime_path.*#runtime_path = \"${kata_fc_path}\"#" $crio_conf_file 
	else
		cat <<EOT | tee -a "$crio_conf_file"

# Path to the Kata Containers runtime binary that uses the firecracker hypervisor.
[$kata_fc_conf]
  runtime_path = "${kata_fc_path}"
EOT
	fi

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
  [plugins.cri]
   [plugins.cri.containerd]
     [plugins.cri.containerd.runtimes.kata]
        runtime_type = "io.containerd.kata.v2"
        [plugins.cri.containerd.runtimes.kata.options]
	      ConfigPath = "/opt/kata/share/defaults/kata-containers/configuration.toml"
     [plugins.cri.containerd.runtimes.kata-fc]
        runtime_type = "io.containerd.kata-fc.v2"
        [plugins.cri.containerd.runtimes.kata-fc.options]
	      ConfigPath = "/opt/kata/share/defaults/kata-containers/configuration-fc.toml"
     [plugins.cri.containerd.runtimes.kata-qemu]
        runtime_type = "io.containerd.kata-qemu.v2"
        [plugins.cri.containerd.runtimes.kata-qemu.options]
	      ConfigPath = "/opt/kata/share/defaults/kata-containers/configuration-qemu.toml"
     [plugins.cri.containerd.runtimes.kata-nemu]
        runtime_type = "io.containerd.kata-nemu.v2"
        [plugins.cri.containerd.runtimes.kata-nemu.options]
	      ConfigPath = "/opt/kata/share/defaults/kata-containers/configuration-nemu.toml"
EOT
	#Currently containerd has an assumption on the location of the shimv2 implementation
	#Until support is added (see https://github.com/containerd/containerd/issues/3073),
    #create a link in /usr/local/bin/ to the v2-shim implementation in /opt/kata/bin.

	mkdir -p /usr/local/bin

	for shim in ${shims[@]}; do
		local shim_binary="containerd-shim-kata-${shim}-v2"
		local shim_file="/usr/local/bin/${shim_binary}"
		local shim_backup="/usr/local/bin/${shim_binary}.bak"

		if [ -f ${shim_file} ]; then
			echo "warning: ${shim_binary} already exists" >&2
			if [ ! -f ${shim_backup} ]; then
				mv ${shim_file} ${shim_backup}
			else
				rm ${shim_file}
			fi
		fi
       cat << EOT | tee "$shim_file"
#!/bin/bash
KATA_CONF_FILE=/opt/kata/share/defaults/kata-containers/configuration-${shim}.toml /opt/kata/bin/containerd-shim-kata-v2 \$@
EOT
       chmod +x $shim_file
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

	#Currently containerd has an assumption on the location of the shimv2 implementation
	#Until support is added (see https://github.com/containerd/containerd/issues/3073), we manage
	# a reference to the v2-shim implementation

	for shim in ${shims[@]}; do
		local shim_binary="containerd-shim-kata-${shim}-v2"
		local shim_file="/usr/local/bin/${shim_binary}"
		local shim_backup="/usr/local/bin/${shim_binary}.bak"

		rm ${shim_file} || true

		if [ -f ${shim_backup} ]; then
			mv "$shim_backup" "$shim_file"
		fi
	done

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
			kubectl label node $NODE_NAME --overwrite katacontainers.io/kata-runtime=true
			;;
		cleanup)
			cleanup_cri_runtime $runtime
			kubectl label node $NODE_NAME --overwrite katacontainers.io/kata-runtime=cleanup
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

main $@
