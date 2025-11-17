#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - kubectl
#   - nsenter (via host_exec function from utils.sh)
#

set -o errexit
set -o pipefail
set -o nounset

# Source all the modular components
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/utils.sh"
source "${SCRIPT_DIR}/config.sh"
source "${SCRIPT_DIR}/runtime.sh"
source "${SCRIPT_DIR}/runtimeclasses.sh"
source "${SCRIPT_DIR}/nfd.sh"
source "${SCRIPT_DIR}/artifacts.sh"
source "${SCRIPT_DIR}/cri-o.sh"
source "${SCRIPT_DIR}/containerd.sh"
source "${SCRIPT_DIR}/snapshotters.sh"
source "${SCRIPT_DIR}/lifecycle.sh"

function print_usage() {
	echo "Usage: $0 [install/cleanup/reset]"
}

function main() {
	action=${1:-}
	if [ -z "$action" ]; then
		print_usage
		die "invalid arguments"
	fi

	echo "Action:"
	echo "* $action"
	echo ""
	echo "Environment variables passed to this script"
	echo "* NODE_NAME: ${NODE_NAME}"
	echo "* DEBUG: ${DEBUG}"
	echo "* SHIMS: ${SHIMS_FOR_ARCH}"
	echo "  * x86_64: ${SHIMS_X86_64}"
	echo "  * aarch64: ${SHIMS_AARCH64}"
	echo "  * s390x: ${SHIMS_S390X}"
	echo "  * ppc64le: ${SHIMS_PPC64LE}"
	echo "* DEFAULT_SHIM: ${DEFAULT_SHIM_FOR_ARCH}"
	echo "  * x86_64: ${DEFAULT_SHIM_X86_64}"
	echo "  * aarch64: ${DEFAULT_SHIM_AARCH64}"
	echo "  * s390x: ${DEFAULT_SHIM_S390X}"
	echo "  * ppc64le: ${DEFAULT_SHIM_PPC64LE}"
	echo "* CREATE_RUNTIMECLASSES: ${CREATE_RUNTIMECLASSES}"
	echo "* CREATE_DEFAULT_RUNTIMECLASS: ${CREATE_DEFAULT_RUNTIMECLASS}"
	echo "* ALLOWED_HYPERVISOR_ANNOTATIONS: ${ALLOWED_HYPERVISOR_ANNOTATIONS_FOR_ARCH}"
	echo "  * x86_64: ${ALLOWED_HYPERVISOR_ANNOTATIONS_X86_64}"
	echo "  * aarch64: ${ALLOWED_HYPERVISOR_ANNOTATIONS_AARCH64}"
	echo "  * s390x: ${ALLOWED_HYPERVISOR_ANNOTATIONS_S390X}"
	echo "  * ppc64le: ${ALLOWED_HYPERVISOR_ANNOTATIONS_PPC64LE}"
	echo "* SNAPSHOTTER_HANDLER_MAPPING: ${SNAPSHOTTER_HANDLER_MAPPING_FOR_ARCH}"
	echo "  * x86_64: ${SNAPSHOTTER_HANDLER_MAPPING_X86_64}"
	echo "  * aarch64: ${SNAPSHOTTER_HANDLER_MAPPING_AARCH64}"
	echo "  * s390x: ${SNAPSHOTTER_HANDLER_MAPPING_S390X}"
	echo "  * ppc64le: ${SNAPSHOTTER_HANDLER_MAPPING_PPC64LE}"
	echo "* AGENT_HTTPS_PROXY: ${AGENT_HTTPS_PROXY}"
	echo "* AGENT_NO_PROXY: ${AGENT_NO_PROXY}"
	echo "* PULL_TYPE_MAPPING: ${PULL_TYPE_MAPPING_FOR_ARCH}"
	echo "  * x86_64: ${PULL_TYPE_MAPPING_X86_64}"
	echo "  * aarch64: ${PULL_TYPE_MAPPING_AARCH64}"
	echo "  * s390x: ${PULL_TYPE_MAPPING_S390X}"
	echo "  * ppc64le: ${PULL_TYPE_MAPPING_PPC64LE}"
	echo "* INSTALLATION_PREFIX: ${INSTALLATION_PREFIX}"
	echo "* MULTI_INSTALL_SUFFIX: ${MULTI_INSTALL_SUFFIX}"
	echo "* HELM_POST_DELETE_HOOK: ${HELM_POST_DELETE_HOOK}"
	echo "* EXPERIMENTAL_SETUP_SNAPSHOTTER: ${EXPERIMENTAL_SETUP_SNAPSHOTTER}"
	echo "* EXPERIMENTAL_FORCE_GUEST_PULL: ${EXPERIMENTAL_FORCE_GUEST_PULL_FOR_ARCH}"
	echo "  * x86_64: ${EXPERIMENTAL_FORCE_GUEST_PULL_X86_64}"
	echo "  * aarch64: ${EXPERIMENTAL_FORCE_GUEST_PULL_AARCH64}"
	echo "  * s390x: ${EXPERIMENTAL_FORCE_GUEST_PULL_S390X}"
	echo "  * ppc64le: ${EXPERIMENTAL_FORCE_GUEST_PULL_PPC64LE}"

	# script requires that user is root
	euid=$(id -u)
	if [[ $euid -ne 0 ]]; then
	   die  "This script must be run as root"
	fi

	runtime=$(get_container_runtime)

	# CRI-O isn't consistent with the naming -- let's use crio to match the service file
	if [ "$runtime" == "cri-o" ]; then
		runtime="crio"
	elif [ "$runtime" == "microk8s" ]; then
		containerd_conf_file="/etc/containerd/containerd-template.toml"
		containerd_conf_file_backup="${containerd_conf_file}.bak"
	elif [[ "$runtime" =~ ^(k3s|k3s-agent|rke2-agent|rke2-server)$ ]]; then
		containerd_conf_tmpl_file="${containerd_conf_file}.tmpl"
		containerd_conf_file_backup="${containerd_conf_tmpl_file}.bak"
	elif [[ "$runtime" =~ ^(k0s-worker|k0s-controller)$ ]]; then
		# From 1.27.1 onwards k0s enables dynamic configuration on containerd CRI runtimes.
		# This works by k0s creating a special directory in /etc/k0s/containerd.d/ where user can drop-in partial containerd configuration snippets.
		# k0s will automatically pick up these files and adds these in containerd configuration imports list.
		containerd_conf_file="/etc/containerd/containerd.d/kata-containers.toml"
		if [ -n "$MULTI_INSTALL_SUFFIX" ]; then
			containerd_conf_file="/etc/containerd/containerd.d/kata-containers-$MULTI_INSTALL_SUFFIX.toml"
		fi
		containerd_conf_file_backup="${containerd_conf_tmpl_file}.bak"
	fi

	# only install / remove / update if we are dealing with CRIO or containerd
	if [[ "$runtime" =~ ^(crio|containerd|k3s|k3s-agent|rke2-agent|rke2-server|k0s-worker|k0s-controller|microk8s)$ ]]; then
		if [ "$runtime" != "crio" ]; then
			containerd_snapshotter_version_check
			snapshotter_handler_mapping_validation_check

			use_containerd_drop_in_conf_file=$(is_containerd_capable_of_using_drop_in_files "$runtime")
			echo "Using containerd drop-in files: $use_containerd_drop_in_conf_file"

			if [[ ! "$runtime" =~ ^(k0s-worker|k0s-controller)$ ]]; then
				# We skip this check for k0s, as they handle things differently on their side
				if [ -n "$MULTI_INSTALL_SUFFIX" ] && [ $use_containerd_drop_in_conf_file = "false" ]; then
					die "Multi installation can only be done if $runtime supports drop-in configuration files"
				fi
			fi
		fi

		case "$action" in
		install)
			# Let's fail early on this, so we don't need to do a rollback
			# in case we reach this situation.
			if [[ -n "${EXPERIMENTAL_SETUP_SNAPSHOTTER}" ]]; then
				if [[ "${runtime}" == "cri-o" ]]; then
					warn "EXPERIMENTAL_SETUP_SNAPSHOTTER is being ignored!"
					warn "Snapshotter is a containerd specific option."
				else
					for snapshotter in "${experimental_setup_snapshotter[@]}"; do
						case "${snapshotter}" in
							erofs)
								containerd_erofs_snapshotter_version_check
								;;
							nydus)
								;;
							*)
								die "${EXPERIMENTAL_SETUP_SNAPSHOTTER} is not a supported snapshotter by kata-deploy"
								;;
						esac
					done
				fi
			fi

			if [[ "$runtime" =~ ^(k3s|k3s-agent|rke2-agent|rke2-server)$ ]]; then
			       if [ ! -f "$containerd_conf_tmpl_file" ] && [ -f "$containerd_conf_file" ]; then
				       cp "$containerd_conf_file" "$containerd_conf_tmpl_file"
			       fi
			       # Only set the containerd_conf_file to its new value after
			       # copying the file to the template location
			       containerd_conf_file="${containerd_conf_tmpl_file}"
			       containerd_conf_file_backup="${containerd_conf_tmpl_file}.bak"
			elif [[ "$runtime" =~ ^(k0s-worker|k0s-controller)$ ]]; then
			       mkdir -p $(dirname "$containerd_conf_file")
			       touch "$containerd_conf_file"
			elif [[ "$runtime" == "containerd" ]]; then
				if [ ! -f "$containerd_conf_file" ] && [ -d $(dirname "$containerd_conf_file") ]; then
					host_exec containerd config default > "$containerd_conf_file"
				fi
			fi

			if [ $use_containerd_drop_in_conf_file = "true" ]; then
				mkdir -p $(dirname "/host$containerd_drop_in_conf_file")
				touch "/host$containerd_drop_in_conf_file"
			fi

			install_artifacts
			configure_cri_runtime "$runtime"

			for snapshotter in "${experimental_setup_snapshotter[@]}"; do
				install_snapshotter "${snapshotter}"
				configure_snapshotter "${snapshotter}"
			done
			restart_runtime "${runtime}"
			kubectl label node "$NODE_NAME" --overwrite katacontainers.io/kata-runtime=true
			;;
		cleanup)
			if [[ "$runtime" =~ ^(k3s|k3s-agent|rke2-agent|rke2-server)$ ]]; then
			       containerd_conf_file_backup="${containerd_conf_tmpl_file}.bak"
			       containerd_conf_file="${containerd_conf_tmpl_file}"
			fi

			local kata_deploy_installations=$(kubectl -n kube-system get ds | grep kata-deploy | wc -l)

			if [ "${HELM_POST_DELETE_HOOK}" == "true" ]; then
				# Remove the label as the first thing, so we ensure no more kata-containers
				# pods would be scheduled here.
				#
				# If we still have any other installation here, it means we'll break them
				# removing the label, so we just don't do it.
				if [ $kata_deploy_installations -eq 0 ]; then
					kubectl label node "$NODE_NAME" katacontainers.io/kata-runtime-
				fi
			fi

			for snapshotter in "${experimental_setup_snapshotter[@]}"; do
				# Here we don't need to do any cleanup on the config, as kata-deploy
				# will revert the configuration to the state it was before the deployment,
				# which is also before the snapshotter configuration. :-)
				uninstall_snapshotter "${EXPERIMENTAL_SETUP_SNAPSHOTTER}"
			done

			cleanup_cri_runtime "$runtime"
			if [ "${HELM_POST_DELETE_HOOK}" == "false" ]; then
				# If we still have any other installation here, it means we'll break them
				# removing the label, so we just don't do it.
				if [ $kata_deploy_installations -eq 0 ]; then
					# The Confidential Containers operator relies on this label
					kubectl label node "$NODE_NAME" --overwrite katacontainers.io/kata-runtime=cleanup
				fi
			fi
			remove_artifacts

			if [ "${HELM_POST_DELETE_HOOK}" == "true" ]; then
				# After everything was cleaned up, there's no reason to continue
				# and sleep forever.  Let's just return success..
				exit 0
			fi
			;;
		reset)
			reset_runtime $runtime
			;;
		*)
			print_usage
			die "invalid arguments"
			;;
		esac
	fi

	#It is assumed this script will be called as a daemonset. As a result, do
        # not return, otherwise the daemon will restart and rexecute the script
	sleep infinity
}

main "$@"
