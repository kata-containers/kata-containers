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
containerd_conf_tmpl_file=""
use_containerd_drop_in_conf_file="false"

IFS=' ' read -a shims <<< "$SHIMS"
default_shim="$DEFAULT_SHIM"
ALLOWED_HYPERVISOR_ANNOTATIONS="${ALLOWED_HYPERVISOR_ANNOTATIONS:-}"

IFS=' ' read -a non_formatted_allowed_hypervisor_annotations <<< "$ALLOWED_HYPERVISOR_ANNOTATIONS"
allowed_hypervisor_annotations=""
for allowed_hypervisor_annotation in "${non_formatted_allowed_hypervisor_annotations[@]}"; do
	allowed_hypervisor_annotations+="\"$allowed_hypervisor_annotation\", "
done
allowed_hypervisor_annotations=$(echo $allowed_hypervisor_annotations | sed 's/,$//')

SNAPSHOTTER_HANDLER_MAPPING="${SNAPSHOTTER_HANDLER_MAPPING:-}"
IFS=',' read -a snapshotters <<< "$SNAPSHOTTER_HANDLER_MAPPING"
snapshotters_delimiter=':'

AGENT_HTTPS_PROXY="${AGENT_HTTPS_PROXY:-}"
AGENT_NO_PROXY="${AGENT_NO_PROXY:-}"

PULL_TYPE_MAPPING="${PULL_TYPE_MAPPING:-}"
IFS=',' read -a pull_types <<< "$PULL_TYPE_MAPPING"

INSTALLATION_PREFIX="${INSTALLATION_PREFIX:-}"
default_dest_dir="/opt/kata"
dest_dir="${default_dest_dir}"
if [ -n "${INSTALLATION_PREFIX}" ]; then
	# There's no `/` in between ${INSTALLATION_PREFIX} and ${default_dest_dir}
	# as, otherwise, we'd have it doubled there, as: `/foo/bar//opt/kata`
	dest_dir="${INSTALLATION_PREFIX}${default_dest_dir}"
fi

MULTI_INSTALL_SUFFIX="${MULTI_INSTALL_SUFFIX:-}"
if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
	dest_dir="${dest_dir}-${MULTI_INSTALL_SUFFIX}"
	crio_drop_in_conf_file="${crio_drop_in_conf_file}-${MULTI_INSTALL_SUFFIX}"
fi
containerd_drop_in_conf_file="${dest_dir}/containerd/config.d/kata-deploy.toml"

# Here, again, there's no `/` between /host and ${dest_dir}, otherwise we'd have it
# doubled here as well, as: `/host//opt/kata`
host_install_dir="/host${dest_dir}"

HELM_POST_DELETE_HOOK="${HELM_POST_DELETE_HOOK:-"false"}"

# If we fail for any reason a message will be displayed
die() {
        msg="$*"
        echo "ERROR: $msg" >&2
        exit 1
}

warn() {
        msg="$*"
        echo "WARN: $msg" >&2
}

info() {
	msg="$*"
	echo "INFO: $msg" >&2
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
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			sed -i -e "s|kata-${shim}|kata-${shim}-${MULTI_INSTALL_SUFFIX}|g" /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
		fi
		kubectl apply -f /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml

		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			# Move the file back to its original state, as the deletion is done
			# differently in the helm and in the kata-deploy daemonset case, meaning
			# that we should assume those files are always as they were during the
			# time the image was built
			sed -i -e "s|kata-${shim}-${MULTI_INSTALL_SUFFIX}|kata-${shim}|g" /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
		fi

	done

	if [[ "${CREATE_DEFAULT_RUNTIMECLASS}" == "true" ]]; then
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			warn "CREATE_DEFAULT_RUNTIMECLASS is being ignored!"
			warn "multi installation does not support creating a default runtime class"

			return
		fi

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
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			sed -i -e "s|kata-${shim}|kata-${shim}-${MULTI_INSTALL_SUFFIX}|g" /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
		fi
		kubectl delete -f /opt/kata-artifacts/runtimeclasses/kata-${shim}.yaml
	done


	if [[ "${CREATE_DEFAULT_RUNTIMECLASS}" == "true" ]]; then
		if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
			# There's nothing to be done here, as a default runtime class is never created
			# for multi installations
			return
		fi

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

function is_containerd_capable_of_using_drop_in_files() {
	local runtime="$1"

	if [ "$runtime" == "crio" ]; then
		# This should never happen but better be safe than sorry
		echo "false"
		return
	fi

	if [[ "$runtime" =~ ^(k0s-worker|k0s-controller)$ ]]; then
		# k0s does the work of using drop-in files better than any other "k8s distro", so
		# we don't mess up with what's being correctly done.
		echo "false"
		return
	fi

	local version_major=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}' | grep -oE '[0-9]+\.[0-9]+' | cut -d'.' -f1)
	if [ $version_major -lt 2 ]; then
		# Only containerd 2.0 does the merge of the plugins section from different snippets,
		# instead of overwritting the whole section, which makes things considerably more
		# complicated for us to deal with.
		#
		# It's been discussed with containerd community, and the patch needed will **NOT** be
		# backported to the release 1.7, as that breaks the behaviour from an existing release.
		echo "false"
		return
	fi

	echo "true"
}

function get_kata_containers_config_path() {
	local shim="$1"

	# Directory holding pristine configuration files for the current default golang runtime.
	local golang_config_path="${dest_dir}/share/defaults/kata-containers/"

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
		cloud-hypervisor | dragonball | qemu-runtime-rs) config_path="$rust_config_path" ;;
		*) config_path="$golang_config_path" ;;
	esac

	echo "$config_path"
}

function get_kata_containers_runtime_path() {
	local shim="$1"

	local runtime_path
	case "$shim" in
		cloud-hypervisor | dragonball | qemu-runtime-rs)
			runtime_path="${dest_dir}/runtime-rs/bin/containerd-shim-kata-v2"
			;;
		*)
			runtime_path="${dest_dir}/bin/containerd-shim-kata-v2"
			;;
	esac

	echo "$runtime_path"
}

function tdx_not_supported() {
	distro="${1}"
	version="${2}"

	warn "Distro ${distro} ${version} does not support TDX and the TDX related runtime classes will not work in your cluster!"
}

function tdx_supported() {
	distro="${1}"
	version="${2}"
	config="${3}"

	sed -i -e "s|PLACEHOLDER_FOR_DISTRO_QEMU_WITH_TDX_SUPPORT|$(get_tdx_qemu_path_from_distro ${distro})|g" ${config}
	sed -i -e "s|PLACEHOLDER_FOR_DISTRO_OVMF_WITH_TDX_SUPPORT|$(get_tdx_ovmf_path_from_distro ${distro})|g" ${config}

	info "In order to use the tdx related runtime classes, ensure TDX is properly configured for ${distro} ${version} by following the instructions provided at: $(get_tdx_distro_instructions ${distro})"
}

function get_tdx_distro_instructions() {
	distro="${1}"

	case ${distro} in
		ubuntu)
			echo "https://github.com/canonical/tdx/tree/noble-24.04"
			;;
		centos)
			echo "https://sigs.centos.org/virt/tdx"
			;;
	esac
}

function get_tdx_qemu_path_from_distro() {
	distro="${1}"

	case ${distro} in
		ubuntu)
			echo "/usr/bin/qemu-system-x86_64"
			;;
		centos)
			echo "/usr/libexec/qemu-kvm"
			;;
	esac
}

function get_tdx_ovmf_path_from_distro() {
	distro="${1}"

	case ${distro} in
		ubuntu)
			echo "/usr/share/ovmf/OVMF.fd"
			;;
		centos)
			echo "/usr/share/edk2/ovmf/OVMF.inteltdx.fd"
			;;
	esac
}

function adjust_qemu_cmdline() {
	shim="${1}"
	config_path="${2}"
	qemu_share="${shim}"

	# The paths on the kata-containers tarball side look like:
	# ${dest_dir}/opt/kata/share/kata-qemu/qemu
	# ${dest_dir}/opt/kata/share/kata-qemu-snp-experimnental/qemu
	[[ "${shim}" =~ ^(qemu-snp|qemu-nvidia-snp)$ ]] && qemu_share=${shim}-experimental

	# Both qemu and qemu-coco-dev use exactly the same QEMU, so we can adjust
	# the shim on the qemu-coco-dev case to qemu
	[[ "${shim}" =~ ^(qemu|qemu-coco-dev)$ ]] && qemu_share="qemu"
		
	qemu_binary=$(tomlq '.hypervisor.qemu.path' ${config_path} | tr -d \")
	qemu_binary_script="${qemu_binary}-installation-prefix"
	qemu_binary_script_host_path="/host/${qemu_binary_script}"

	if [[ ! -f ${qemu_binary_script_host_path} ]]; then
		# From the QEMU man page:
		# ```
		# -L  path
		# 	Set the directory for the BIOS, VGA BIOS and keymaps.
		# 	To list all the data directories, use -L help.
		# ```
		#
		# The reason we have to do this here, is because otherwise QEMU
		# will only look for those files in specific paths, which are
		# tied to the location of the PREFIX used during build time
		# (/opt/kata, in our case).
		cat <<EOF >${qemu_binary_script_host_path}
#!/usr/bin/env bash

exec ${qemu_binary} "\$@" -L ${dest_dir}/share/kata-${qemu_share}/qemu/
EOF
		chmod +x ${qemu_binary_script_host_path}
	fi

	sed -i -e "s|${qemu_binary}|${qemu_binary_script}|" ${config_path}
}

function install_artifacts() {
	echo "copying kata artifacts onto host"

	mkdir -p ${host_install_dir}
	cp -au /opt/kata-artifacts/opt/kata/* ${host_install_dir}/
	chmod +x ${host_install_dir}/bin/*
	[ -d ${host_install_dir}/runtime-rs/bin ] && \
		chmod +x ${host_install_dir}/runtime-rs/bin/*

	local config_path

	for shim in "${shims[@]}"; do
		config_path="/host/$(get_kata_containers_config_path "${shim}")"
		mkdir -p "$config_path"

		local kata_config_file="${config_path}/configuration-${shim}.toml"
		# Properly set https_proxy and no_proxy for Kata Containers
		if [ -n "${AGENT_HTTPS_PROXY}" ]; then
			sed -i -e 's|^kernel_params = "\(.*\)"|kernel_params = "\1 agent.https_proxy='${AGENT_HTTPS_PROXY}'"|g' "${kata_config_file}"
		fi

		if [ -n "${AGENT_NO_PROXY}" ]; then
			sed -i -e 's|^kernel_params = "\(.*\)"|kernel_params = "\1 agent.no_proxy='${AGENT_NO_PROXY}'"|g' "${kata_config_file}"
		fi

		# Allow enabling debug for Kata Containers
		if [[ "${DEBUG}" == "true" ]]; then
			sed -i -e 's/^#\(enable_debug\).*=.*$/\1 = true/g' "${kata_config_file}"
			sed -i -e 's/^#\(debug_console_enabled\).*=.*$/\1 = true/g' "${kata_config_file}"
			sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug initcall_debug"/g' "${kata_config_file}"
		fi

		if [ -n "${allowed_hypervisor_annotations}" ]; then
			sed -i -e "s/^enable_annotations = \[\(.*\)\]/enable_annotations = [\1, $allowed_hypervisor_annotations]/" "${kata_config_file}"
		fi

		if grep -q "tdx" <<< "$shim"; then
  			VERSION_ID=version_unset # VERSION_ID may be unset, see https://www.freedesktop.org/software/systemd/man/latest/os-release.html#Notes
			source /host/etc/os-release || source /host/usr/lib/os-release
			case ${ID} in
				ubuntu)
					case ${VERSION_ID} in
						24.04)
							tdx_supported ${ID} ${VERSION_ID} ${kata_config_file}
							;;
						*)
							tdx_not_supported ${ID} ${VERSION_ID}
							;;
					esac
					;;
				centos)
					case ${VERSION_ID} in
						9)
							tdx_supported ${ID} ${VERSION_ID} ${kata_config_file}
							;;
						*)
							tdx_not_supported ${ID} ${VERSION_ID}
							;;
					esac
					;;
				*)
					tdx_not_supported ${ID} ${VERSION_ID}
					;;
			esac	
		fi

		if [ "${dest_dir}" != "${default_dest_dir}" ]; then
			# We could always do this sed, regardless, but I have a strong preference
			# on not touching the configuration files unless extremelly needed
			sed -i -e "s|${default_dest_dir}|${dest_dir}|g" "${kata_config_file}"

			# Let's only adjust qemu_cmdline for the QEMUs that we build and ship ourselves
			[[ "${shim}" =~ ^(qemu|qemu-snp|qemu-nvidia-gpu|qemu-nvidia-gpu-snp|qemu-sev|qemu-se|qemu-coco-dev)$ ]] && \
				adjust_qemu_cmdline "${shim}" "${kata_config_file}"
		fi
	done

	# Allow Mariner to use custom configuration.
	if [ "${HOST_OS:-}" == "cbl-mariner" ]; then
		config_path="${host_install_dir}/share/defaults/kata-containers/configuration-clh.toml"
		sed -i -E "s|(static_sandbox_resource_mgmt)=false|\1=true|" "${config_path}"

		clh_path="${dest_dir}/bin/cloud-hypervisor-glibc"
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

function configure_crio_runtime() {
	local shim="${1}"
	local adjusted_shim_to_multi_install="${shim}"
	if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
		adjusted_shim_to_multi_install="${shim}-${MULTI_INSTALL_SUFFIX}"
	fi
	local runtime="kata-${adjusted_shim_to_multi_install}"
	local configuration="configuration-${shim}"

	local config_path=$(get_kata_containers_config_path "${shim}")

	local kata_path=$(get_kata_containers_runtime_path "${shim}")
	local kata_conf="crio.runtime.runtimes.${runtime}"
	local kata_config_path="${config_path}/${configuration}.toml"

	cat <<EOF | tee -a "$crio_drop_in_conf_file"

[$kata_conf]
	runtime_path = "${kata_path}"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	runtime_config_path = "${kata_config_path}"
	privileged_without_host_devices = true
EOF

	local key
	local value
	if [ -n "${PULL_TYPE_MAPPING}" ]; then
		for m in "${pull_types[@]}"; do
			key="${m%"$snapshotters_delimiter"*}"
			value="${m#*"$snapshotters_delimiter"}"

			if [[ "${value}" = "default" || "${key}" != "${shim}" ]]; then
				continue
			fi

			if [ "${value}" == "guest-pull" ]; then
				echo -e "\truntime_pull_image = true" | \
					tee -a "$crio_drop_in_conf_file"
			else
				die "Unsupported pull type '$value' for ${shim}"
			fi
			break
		done
	fi
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
	rm -f "$crio_drop_in_conf_file_debug"
	touch "$crio_drop_in_conf_file_debug"

	# configure storage option for crio
	cat <<EOF | tee -a "$crio_drop_in_conf_file"
[crio]
  storage_option = [
	"overlay.skip_mount_home=true",
  ]
EOF

	# configure runtimes for crio
	for shim in "${shims[@]}"; do
		configure_crio_runtime $shim
	done

	if [ "${DEBUG}" == "true" ]; then
		cat <<EOF | tee $crio_drop_in_conf_file_debug
[crio.runtime]
log_level = "debug"
EOF
	fi
}

function configure_containerd_runtime() {
	local shim="$2"
	local adjusted_shim_to_multi_install="${shim}"
	if [ -n "${MULTI_INSTALL_SUFFIX}" ]; then
		adjusted_shim_to_multi_install="${shim}-${MULTI_INSTALL_SUFFIX}"
	fi
	local runtime="kata-${adjusted_shim_to_multi_install}"
	local configuration="configuration-${shim}"
	local pluginid=cri
	local configuration_file="${containerd_conf_file}"

	# Properly set the configuration file in case drop-in files are supported
	if [ $use_containerd_drop_in_conf_file = "true" ]; then
		configuration_file="/host${containerd_drop_in_conf_file}"
	fi

	local containerd_root_conf_file="$containerd_conf_file"
	if [[ "$1" =~ ^(k0s-worker|k0s-controller)$ ]]; then
		containerd_root_conf_file="/etc/containerd/containerd.toml"
	fi

	if grep -q "version = 2\>" $containerd_root_conf_file; then
		pluginid=\"io.containerd.grpc.v1.cri\"
	fi

	if grep -q "version = 3\>" $containerd_root_conf_file; then
		pluginid=\"io.containerd.cri.v1.runtime\"
	fi

	local runtime_table=".plugins.${pluginid}.containerd.runtimes.\"${runtime}\""
	local runtime_options_table="${runtime_table}.options"
	local runtime_type=\"io.containerd."${runtime}".v2\"
	local runtime_config_path=\"$(get_kata_containers_config_path "${shim}")/${configuration}.toml\"
	local runtime_path=\"$(get_kata_containers_runtime_path "${shim}")\"
	
	tomlq -i -t $(printf '%s.runtime_type=%s' ${runtime_table} ${runtime_type}) ${configuration_file}
	tomlq -i -t $(printf '%s.runtime_path=%s' ${runtime_table} ${runtime_path}) ${configuration_file}
	tomlq -i -t $(printf '%s.privileged_without_host_devices=true' ${runtime_table}) ${configuration_file}
	tomlq -i -t $(printf '%s.pod_annotations=["io.katacontainers.*"]' ${runtime_table}) ${configuration_file}
	tomlq -i -t $(printf '%s.ConfigPath=%s' ${runtime_options_table} ${runtime_config_path}) ${configuration_file}
	
	if [ "${DEBUG}" == "true" ]; then
		tomlq -i -t '.debug.level = "debug"' ${configuration_file}
	fi

	if [ -n "${SNAPSHOTTER_HANDLER_MAPPING}" ]; then
		for m in ${snapshotters[@]}; do
			key="${m%$snapshotters_delimiter*}"

			if [ "${key}" != "${shim}" ]; then
				continue
			fi

			value="${m#*$snapshotters_delimiter}"
			tomlq -i -t $(printf '%s.snapshotter="%s"' ${runtime_table} ${value}) ${configuration_file}
			break
		done
	fi
}

function configure_containerd() {
	# Configure containerd to use Kata:
	echo "Add Kata Containers as a supported runtime for containerd"

	mkdir -p /etc/containerd/

	if [ $use_containerd_drop_in_conf_file = "false" ] && [ -f "$containerd_conf_file" ]; then
		# only backup in case drop-in files are not supported, and when doing the backup
		# only do it if a backup doesn't already exist (don't override original)
		cp -n "$containerd_conf_file" "$containerd_conf_file_backup"
	fi

	if [ $use_containerd_drop_in_conf_file = "true" ]; then
		tomlq -i -t $(printf '.imports|=.+["%s"]' ${containerd_drop_in_conf_file}) ${containerd_conf_file}
	fi

	for shim in "${shims[@]}"; do
		configure_containerd_runtime "$1" $shim
	done
}

function remove_artifacts() {
	echo "deleting kata artifacts"

	rm -rf ${host_install_dir}

	if [[ "${CREATE_RUNTIMECLASSES}" == "true" ]]; then
		delete_runtimeclasses
	fi
}

function restart_cri_runtime() {
	local runtime="${1}"

	if [ "${runtime}" == "k0s-worker" ] || [ "${runtime}" == "k0s-controller" ]; then
		# do nothing, k0s will automatically unload the config on the fly
		:
	else
		host_systemctl daemon-reload
		host_systemctl restart "${runtime}"
	fi
}

function cleanup_cri_runtime() {
	case $1 in
	crio)
		cleanup_crio
		;;
	containerd | k3s | k3s-agent | rke2-agent | rke2-server | k0s-controller | k0s-worker)
		cleanup_containerd
		;;
	esac

	[ "${HELM_POST_DELETE_HOOK}" == "false" ] && return

	# Only run this code in the HELM_POST_DELETE_HOOK
	restart_cri_runtime "$1"
}

function cleanup_crio() {
	rm -f $crio_drop_in_conf_file
	if [[ "${DEBUG}" == "true" ]]; then
		rm -f $crio_drop_in_conf_file_debug
	fi
}

function cleanup_containerd() {
	if [ $use_containerd_drop_in_conf_file = "true" ]; then
		# There's no need to remove the drop-in file, as it'll be removed as
		# part of the artefacts removal.  Thus, simply remove the file from
		# the imports line of the containerd configuration and return.
		tomlq -i -t $(printf '.imports|=.-["%s"]' ${containerd_drop_in_conf_file}) ${containerd_conf_file}
		return
	fi

	rm -f $containerd_conf_file
	if [ -f "$containerd_conf_file_backup" ]; then
		mv "$containerd_conf_file_backup" "$containerd_conf_file"
	fi
}

function reset_runtime() {
	kubectl label node "$NODE_NAME" katacontainers.io/kata-runtime-
	restart_cri_runtime "$1"

	if [ "$1" == "crio" ] || [ "$1" == "containerd" ]; then
		host_systemctl restart kubelet
	fi

	wait_till_node_is_ready
}

function containerd_snapshotter_version_check() {
	local container_runtime_version=$(kubectl get node $NODE_NAME -o jsonpath='{.status.nodeInfo.containerRuntimeVersion}')
	local containerd_prefix="containerd://"
	local containerd_version_to_avoid="1.6"
	local containerd_version=${container_runtime_version#$containerd_prefix}

	if grep -q ^$containerd_version_to_avoid <<< $containerd_version; then
		if [ -n "${SNAPSHOTTER_HANDLER_MAPPING}" ]; then
			die "kata-deploy only supports snapshotter configuration with containerd 1.7 or newer"
		fi
	fi
}

function snapshotter_handler_mapping_validation_check() {
	echo "Validating the snapshotter-handler mapping: \"${SNAPSHOTTER_HANDLER_MAPPING}\""
	if [ -z "${SNAPSHOTTER_HANDLER_MAPPING}" ]; then
		echo "No snapshotter has been requested, using the default value from containerd"
		return
	fi

	for m in ${snapshotters[@]}; do
		shim="${m%$snapshotters_delimiter*}"
		snapshotter="${m#*$snapshotters_delimiter}"

		if [ -z "$shim" ]; then
			die "The snapshotter must follow the \"shim:snapshotter,shim:snapshotter,...\" format, but at least one shim is empty"
		fi

		if [ -z "$snapshotter" ]; then
			die "The snapshotter must follow the \"shim:snapshotter,shim:snapshotter,...\" format, but at least one snapshotter is empty"
		fi

		if ! grep -q " $shim " <<< " $SHIMS "; then
			die "\"$shim\" is not part of \"$SHIMS\""
		fi

		matches=$(grep -o "$shim$snapshotters_delimiter" <<< "${SNAPSHOTTER_HANDLER_MAPPING}" | wc -l)
		if [ $matches -ne 1 ]; then
			die "One, and only one, entry per shim is required"
		fi
	done
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
	echo "* SHIMS: ${SHIMS}"
	echo "* DEFAULT_SHIM: ${DEFAULT_SHIM}"
	echo "* CREATE_RUNTIMECLASSES: ${CREATE_RUNTIMECLASSES}"
	echo "* CREATE_DEFAULT_RUNTIMECLASS: ${CREATE_DEFAULT_RUNTIMECLASS}"
	echo "* ALLOWED_HYPERVISOR_ANNOTATIONS: ${ALLOWED_HYPERVISOR_ANNOTATIONS}"
	echo "* SNAPSHOTTER_HANDLER_MAPPING: ${SNAPSHOTTER_HANDLER_MAPPING}"
	echo "* AGENT_HTTPS_PROXY: ${AGENT_HTTPS_PROXY}"
	echo "* AGENT_NO_PROXY: ${AGENT_NO_PROXY}"
	echo "* PULL_TYPE_MAPPING: ${PULL_TYPE_MAPPING}"
	echo "* INSTALLATION_PREFIX: ${INSTALLATION_PREFIX}"
	echo "* MULTI_INSTALL_SUFFIX: ${MULTI_INSTALL_SUFFIX}"
	echo "* HELM_POST_DELETE_HOOK: ${HELM_POST_DELETE_HOOK}"

	# script requires that user is root
	euid=$(id -u)
	if [[ $euid -ne 0 ]]; then
	   die  "This script must be run as root"
	fi

	runtime=$(get_container_runtime)

	# CRI-O isn't consistent with the naming -- let's use crio to match the service file
	if [ "$runtime" == "cri-o" ]; then
		runtime="crio"
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
	if [[ "$runtime" =~ ^(crio|containerd|k3s|k3s-agent|rke2-agent|rke2-server|k0s-worker|k0s-controller)$ ]]; then
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
			       if [ ! -f "$containerd_conf_file" ] && [ -d $(dirname "$containerd_conf_file") ] && [ -x $(command -v containerd) ]; then
					containerd config default > "$containerd_conf_file"
			       fi
			fi

			if [ $use_containerd_drop_in_conf_file = "true" ]; then
				mkdir -p $(dirname "/host$containerd_drop_in_conf_file")
				touch "/host$containerd_drop_in_conf_file"
			fi

			install_artifacts
			configure_cri_runtime "$runtime"
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
