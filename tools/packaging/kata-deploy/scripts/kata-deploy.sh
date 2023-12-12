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
crio_drop_in_conf_file_debug="${crio_drop_in_conf_dir}/100-debug"
containerd_conf_file="/etc/containerd/config.toml"
containerd_conf_file_backup="${containerd_conf_file}.bak"

IFS=' ' read -a shims <<< "$SHIMS"
default_shim="$DEFAULT_SHIM"
ALLOWED_HYPERVISOR_ANNOTATIONS="${ALLOWED_HYPERVISOR_ANNOTATIONS:-}"

IFS=' ' read -a non_formatted_allowed_hypervisor_annotations <<< "$ALLOWED_HYPERVISOR_ANNOTATIONS"
allowed_hypervisor_annotations=""
for allowed_hypervisor_annotation in "${non_formatted_allowed_hypervisor_annotations[@]}"; do
	allowed_hypervisor_annotations+="\"$allowed_hypervisor_annotation\", "
done
allowed_hypervisor_annotations=$(echo $allowed_hypervisor_annotations | sed 's/,$//')

# If we fail for any reason a message will be displayed
die() {
        msg="$*"
        echo "ERROR: $msg" >&2
        exit 1
}

function host_systemctl() {
	nsenter --target 1 --mount systemctl "${@}"
}

function print_usage() {
	echo "Usage: $0 [install/cleanup/reset]"
}

function create_runtimeclasses() {
	echo "Creating the runtime classes"

	for shim in "${shims[@]}"; do
		echo "Creating the kata-${shim} runtime class"
		kubectl apply -f /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
	done

	if [[ "${CREATE_DEFAULT_RUNTIMECLASS}" == "true" ]]; then
		echo "Creating the kata runtime class for the default shim (an alias for kata-${default_shim})"
		cp /opt/kata-artifacts/runtimeclasses/kata-${default_shim}.yaml /tmp/kata.yaml
		sed -i -e 's/name: kata-'${default_shim}'/name: kata/g' /tmp/kata.yaml
		kubectl apply -f /tmp/kata.yaml
		rm -f /tmp/kata.yaml
	fi
}

function delete_runtimeclasses() {
	echo "Deleting the runtime classes"

	for shim in "${shims[@]}"; do
		echo "Deleting the kata-${shim} runtime class"
		kubectl delete -f /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
	done


	if [[ "${CREATE_DEFAULT_RUNTIMECLASS}" == "true" ]]; then
		echo "Deleting the kata runtime class for the default shim (an alias for kata-${default_shim})"
		cp /opt/kata-artifacts/runtimeclasses/kata-${default_shim}.yaml /tmp/kata.yaml
		sed -i -e 's/name: kata-'${default_shim}'/name: kata/g' /tmp/kata.yaml
		kubectl delete -f /tmp/kata.yaml
		rm -f /tmp/kata.yaml
	fi
}

function get_container_runtime() {

	local runtime=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
	if [ "$?" -ne 0 ]; then
                die "invalid node name"
	fi

	if echo "$runtime" | grep -qE "cri-o"; then
		echo "cri-o"
	elif echo "$runtime" | grep -qE 'containerd.*-k3s'; then
		if host_systemctl is-active --quiet rke2-agent; then
			echo "rke2-agent"
		elif host_systemctl is-active --quiet rke2-server; then
			echo "rke2-server"
		elif host_systemctl is-active --quiet k3s-agent; then
			echo "k3s-agent"
		else
			echo "k3s"
		fi
	# Note: we assumed you used a conventional k0s setup and k0s will generate a systemd entry k0scontroller.service and k0sworker.service respectively    
	# and it is impossible to run this script without a kubelet, so this k0s controller must also have worker mode enabled 
	elif host_systemctl is-active --quiet k0scontroller; then
		echo "k0s-controller"
	elif host_systemctl is-active --quiet k0sworker; then
		echo "k0s-worker"
	else
		echo "$runtime" | awk -F '[:]' '{print $1}'
	fi
}

function get_kata_containers_config_path() {
	local shim="$1"

	# Directory holding pristine configuration files for the current default golang runtime.
	local golang_config_path="/opt/kata/share/defaults/kata-containers/"

	# Directory holding pristine configuration files for the new rust runtime.
	#
	# These are put into a separate directory since:
	#
	# - In some cases, the rust runtime configuration syntax is
	#   slightly different to the golang runtime configuration files
	#   so some hypervisors need two different configuration files,
	#   one for reach runtime type (for example Cloud Hypervisor which
	#   uses 'clh' for the golang runtime and 'cloud-hypervisor' for
	#   the rust runtime.
	#
	# - Some hypervisors only currently work with the golang runtime.
	#
	# - Some hypervisors only work with the rust runtime (dragonball).
	#
	# See: https://github.com/kata-containers/kata-containers/issues/6020
	local rust_config_path="${golang_config_path}/runtime-rs"

	local config_path

	# Map the runtime shim name to the appropriate configuration
	# file directory.
	case "$shim" in
		cloud-hypervisor | dragonball) config_path="$rust_config_path" ;;
		*) config_path="$golang_config_path" ;;
	esac

	echo "$config_path"
}

function install_artifacts() {
	echo "copying kata artifacts onto host"
	cp -au /opt/kata-artifacts/opt/kata/* /opt/kata/
	chmod +x /opt/kata/bin/*
	[ -d /opt/kata/runtime-rs/bin ] && \
		chmod +x /opt/kata/runtime-rs/bin/*

	local config_path

	for shim in "${shims[@]}"; do
		config_path=$(get_kata_containers_config_path "${shim}")
		mkdir -p "$config_path"

		local kata_config_file="${config_path}/configuration-${shim}.toml"
		# Allow enabling debug for Kata Containers
		if [[ "${DEBUG}" == "true" ]]; then
			sed -i -e 's/^#\(enable_debug\).*=.*$/\1 = true/g' "${kata_config_file}"
			sed -i -e 's/^#\(debug_console_enabled\).*=.*$/\1 = true/g' "${kata_config_file}"
			sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug initcall_debug"/g' "${kata_config_file}"
		fi

		if [ -n "${allowed_hypervisor_annotations}" ]; then
			sed -i -e "s/^enable_annotations = \[\(.*\)\]/enable_annotations = [\1, $allowed_hypervisor_annotations]/" "${kata_config_file}"
		fi
	done

	# Allow Mariner to use custom configuration.
	if [ "${HOST_OS:-}" == "cbl-mariner" ]; then
		config_path="/opt/kata/share/defaults/kata-containers/configuration-clh.toml"
		clh_path="/opt/kata/bin/cloud-hypervisor-glibc"
		sed -i -E "s|(valid_hypervisor_paths) = .+|\1 = [\"${clh_path}\"]|" "${config_path}"
		sed -i -E "s|(path) = \".+/cloud-hypervisor\"|\1 = \"${clh_path}\"|" "${config_path}"
	fi


	if [[ "${CREATE_RUNTIMECLASSES}" == "true" ]]; then
		create_runtimeclasses
	fi
}

function wait_till_node_is_ready() {
	local ready="False"

	while ! [[ "${ready}" == "True" ]]; do
		sleep 2s
		ready=$(kubectl get node $NODE_NAME -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}')
	done
}

function configure_cri_runtime() {
	configure_different_shims_base

	case $1 in
	crio)
		configure_crio
		;;
	containerd | k3s | k3s-agent | rke2-agent | rke2-server | k0s-controller | k0s-worker)
		configure_containerd "$1"
		;;
	esac
	if [ "$1" == "k0s-worker" ] || [ "$1" == "k0s-controller" ]; then
		# do nothing, k0s will automatically load the config on the fly
		:
	else
		host_systemctl daemon-reload
		host_systemctl restart "$1"
	fi

	wait_till_node_is_ready
}

function backup_shim() {
	local shim_file="$1"
	local shim_backup="${shim_file}.bak"

	if [ -f "${shim_file}" ]; then
		echo "warning: ${shim_file} already exists" >&2
		if [ ! -f "${shim_backup}" ]; then
			mv "${shim_file}" "${shim_backup}"
		else
			rm -f "${shim_file}"
		fi
	fi
}

function configure_different_shims_base() {
	# Currently containerd has an assumption on the location of the shimv2 implementation
	# This forces kata-deploy to create files in a well-defined location that's part of
	# the PATH, pointing to the containerd-shim-kata-v2 binary in /opt/kata/bin
	# Issues:
	#   https://github.com/containerd/containerd/issues/3073
	#   https://github.com/containerd/containerd/issues/5006

	local default_shim_file="/usr/local/bin/containerd-shim-kata-v2"

	mkdir -p /usr/local/bin

	for shim in "${shims[@]}"; do
		local shim_binary="containerd-shim-kata-${shim}-v2"
		local shim_file="/usr/local/bin/${shim_binary}"

		backup_shim "${shim_file}"

		# Map the runtime shim name to the appropriate
		# containerd-shim-kata-v2 binary
		case "$shim" in
			cloud-hypervisor | dragonball)
				ln -sf /opt/kata/runtime-rs/bin/containerd-shim-kata-v2 "${shim_file}" ;;
			*)
				ln -sf /opt/kata/bin/containerd-shim-kata-v2 "${shim_file}" ;;
		esac

		chmod +x "$shim_file"

		if [ "${shim}" == "${default_shim}" ]; then
			backup_shim "${default_shim_file}"

			echo "Creating the default shim-v2 binary"
			ln -sf "${shim_file}" "${default_shim_file}"
		fi
	done
}

function restore_shim() {
	local shim_file="$1"
	local shim_backup="${shim_file}.bak"

	if [ -f "${shim_backup}" ]; then
		mv "$shim_backup" "$shim_file"
	fi
}

function cleanup_different_shims_base() {
	local default_shim_file="/usr/local/bin/containerd-shim-kata-v2"

	for shim in "${shims[@]}"; do
		local shim_binary="containerd-shim-kata-${shim}-v2"
		local shim_file="/usr/local/bin/${shim_binary}"

		rm  -f "${shim_file}"

		restore_shim "${shim_file}"
	done

	rm  -f "${default_shim_file}"
	restore_shim "${default_shim_file}"

	if [[ "${CREATE_RUNTIMECLASSES}" == "true" ]]; then
		delete_runtimeclasses
	fi
}

function configure_crio_runtime() {
	local runtime="kata"
	local configuration="configuration"
	if [ -n "${1-}" ]; then
		runtime+="-$1"
		configuration+="-$1"
	fi

	local config_path=$(get_kata_containers_config_path "${1}")

	local kata_path="/usr/local/bin/containerd-shim-${runtime}-v2"
	local kata_conf="crio.runtime.runtimes.${runtime}"
	local kata_config_path="${config_path}/${configuration}.toml"

	cat <<EOF | tee -a "$crio_drop_in_conf_file"

# Path to the Kata Containers runtime binary that uses the $1
[$kata_conf]
	runtime_path = "${kata_path}"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	runtime_config_path = "${kata_config_path}"
	privileged_without_host_devices = true
EOF
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


	if [ "${DEBUG}" == "true" ]; then
		cat <<EOF | tee -a $crio_drop_in_conf_file_debug
[crio]
log_level = "debug"
EOF
	fi
}

function configure_containerd_runtime() {
	local runtime="kata"
	local configuration="configuration"
	if [ -n "${2-}" ]; then
		runtime+="-$2"
		configuration+="-$2"
	fi
	local pluginid=cri
	
	# if we are running k0s auto containerd.toml generation, the base template is by default version 2
	# we can safely assume to reference the newer version of cri
	if grep -q "version = 2\>" $containerd_conf_file || [ "$1" == "k0s-worker" ] || [ "$1" == "k0s-controller" ]; then
		pluginid=\"io.containerd.grpc.v1.cri\"
	fi
	local runtime_table=".plugins.${pluginid}.containerd.runtimes.\"${runtime}\""
	local runtime_options_table="${runtime_table}.options"
	local runtime_type=\"io.containerd."${runtime}".v2\"
	local runtime_config_path=\"$(get_kata_containers_config_path "${2-}")/${configuration}.toml\"
	
	tomlq -i -t $(printf '%s.runtime_type=%s' ${runtime_table} ${runtime_type}) ${containerd_conf_file}
	tomlq -i -t $(printf '%s.privileged_without_host_devices=true' ${runtime_table}) ${containerd_conf_file}
	tomlq -i -t $(printf '%s.pod_annotations=["io.katacontainers.*"]' ${runtime_table}) ${containerd_conf_file}
	tomlq -i -t $(printf '%s.ConfigPath=%s' ${runtime_options_table} ${runtime_config_path}) ${containerd_conf_file}
	
	if [ "${DEBUG}" == "true" ]; then
		tomlq -i -t '.debug.level = "debug"' ${containerd_conf_file}
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
	configure_containerd_runtime "$1" 

	for shim in "${shims[@]}"; do
		configure_containerd_runtime "$1" $shim
	done
}

function remove_artifacts() {
	echo "deleting kata artifacts"
	rm -rf /opt/kata/*
}

function cleanup_cri_runtime() {
	cleanup_different_shims_base

	case $1 in
	crio)
		cleanup_crio
		;;
	containerd | k3s | k3s-agent | rke2-agent | rke2-server | k0s-controller | k0s-worker)
		cleanup_containerd
		;;
	esac

}

function cleanup_crio() {
	rm -f $crio_drop_in_conf_file
	if [[ "${DEBUG}" == "true" ]]; then
		rm -f $crio_drop_in_conf_file_debug
	fi
}

function cleanup_containerd() {
	rm -f $containerd_conf_file
	if [ -f "$containerd_conf_file_backup" ]; then
		mv "$containerd_conf_file_backup" "$containerd_conf_file"
	fi
}

function reset_runtime() {
	kubectl label node "$NODE_NAME" katacontainers.io/kata-runtime-
	if [ "$1" == "k0s-worker" ] || [ "$1" == "k0s-controller" ]; then
		# do nothing, k0s will auto restart
		:
	else
		host_systemctl daemon-reload
		host_systemctl restart "$1"
	fi

	if [ "$1" == "crio" ] || [ "$1" == "containerd" ]; then
		host_systemctl restart kubelet
	fi

	wait_till_node_is_ready
}

function main() {
	echo "Environment variables passed to this script"
	echo "* NODE_NAME: ${NODE_NAME}"
	echo "* DEBUG: ${DEBUG}"
	echo "* SHIMS: ${SHIMS}"
	echo "* DEFAULT_SHIM: ${DEFAULT_SHIM}"
	echo "* CREATE_RUNTIMECLASSES: ${CREATE_RUNTIMECLASSES}"
	echo "* CREATE_DEFAULT_RUNTIMECLASS: ${CREATE_DEFAULT_RUNTIMECLASS}"
	echo "* ALLOWED_HYPERVISOR_ANNOTATIONS: ${ALLOWED_HYPERVISOR_ANNOTATIONS}"

	# script requires that user is root
	euid=$(id -u)
	if [[ $euid -ne 0 ]]; then
	   die  "This script must be run as root"
	fi

	runtime=$(get_container_runtime)

	# CRI-O isn't consistent with the naming -- let's use crio to match the service file
	if [ "$runtime" == "cri-o" ]; then
		runtime="crio"
	elif [ "$runtime" == "k3s" ] || [ "$runtime" == "k3s-agent" ] || [ "$runtime" == "rke2-agent" ] || [ "$runtime" == "rke2-server" ]; then
		containerd_conf_tmpl_file="${containerd_conf_file}.tmpl"
		if [ ! -f "$containerd_conf_tmpl_file" ]; then
			cp "$containerd_conf_file" "$containerd_conf_tmpl_file"
		fi

		containerd_conf_file="${containerd_conf_tmpl_file}"
		containerd_conf_file_backup="${containerd_conf_file}.bak"
	elif [ "$runtime" == "k0s-worker" ] || [ "$runtime" == "k0s-controller" ]; then
		# From 1.27.1 onwards k0s enables dynamic configuration on containerd CRI runtimes. 
		# This works by k0s creating a special directory in /etc/k0s/containerd.d/ where user can drop-in partial containerd configuration snippets.
		# k0s will automatically pick up these files and adds these in containerd configuration imports list.
		containerd_conf_file="/etc/containerd/kata-containers.toml"
	else
		# runtime == containerd
		if [ ! -f "$containerd_conf_file" ] && [ -d $(dirname "$containerd_conf_file") ] && \
			[ -x $(command -v containerd) ]; then
			containerd config default > "$containerd_conf_file"
		fi
	fi

	action=${1:-}
	if [ -z "$action" ]; then
		print_usage
		die "invalid arguments"
	fi

	# only install / remove / update if we are dealing with CRIO or containerd
	if [[ "$runtime" =~ ^(crio|containerd|k3s|k3s-agent|rke2-agent|rke2-server|k0s-worker|k0s-controller)$ ]]; then

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
