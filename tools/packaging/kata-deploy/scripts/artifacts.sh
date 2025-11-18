#!/bin/sh
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# External dependencies (not present in bare minimum busybox image):
#   - tomlq
#

install_artifacts() {
	echo "copying kata artifacts onto host"

	mkdir -p ${host_install_dir}
	cp -au /opt/kata-artifacts/opt/kata/* ${host_install_dir}/
	chmod +x ${host_install_dir}/bin/*
	[ -d ${host_install_dir}/runtime-rs/bin ] && \
		chmod +x ${host_install_dir}/runtime-rs/bin/*

	local config_path

	for shim in ${shims}; do
		config_path="/host/$(get_kata_containers_config_path "${shim}")"
		mkdir -p "$config_path"

		local kata_config_file="${config_path}/configuration-${shim}.toml"
		# Till deprecation period is over, we need to support:
		# * "http://proxy:8080" (applies to all shims)
		# * per-shim format: "qemu-tdx=http://proxy:8080;qemu-snp=http://proxy2:8080"
		if [ -n "${AGENT_HTTPS_PROXY}" ]; then
			local https_proxy_value=""
			local proxy_mappings
			local mapping
			local key
			local value

			# Parse AGENT_HTTPS_PROXY - check if it contains "=" for per-shim format
			case "${AGENT_HTTPS_PROXY}" in
				*=*)
					# Per-shim format: parse semicolon-separated "shim=proxy" mappings
					proxy_mappings=$(echo "${AGENT_HTTPS_PROXY}" | tr ';' ' ')
					for mapping in ${proxy_mappings}; do
						key="${mapping%%=*}"
						value="${mapping#*=}"
						if [ "${key}" = "${shim}" ]; then
							https_proxy_value="${value}"
							break
						fi
					done
					;;
				*)
					# Simple format: apply to all shims
					https_proxy_value="${AGENT_HTTPS_PROXY}"
					;;
			esac

			if [ -n "${https_proxy_value}" ]; then
				if ! field_contains_value "${kata_config_file}" "kernel_params" "agent.https_proxy"; then
					sed -i -e 's|^kernel_params = "\(.*\)"|kernel_params = "\1 agent.https_proxy='"${https_proxy_value}"'"|g' "${kata_config_file}"
				fi
			fi
		fi

		# Till deprecation period is over, need to support:
		# * "localhost,127.0.0.1" (applies to all shims)
		# * per-shim format: "qemu-tdx=localhost,127.0.0.1;qemu-snp=192.168.0.0/16"
		if [ -n "${AGENT_NO_PROXY}" ]; then
			local no_proxy_value=""
			local noproxy_mappings
			local mapping
			local key
			local value

			# Parse AGENT_NO_PROXY - check if it contains "=" for per-shim format
			case "${AGENT_NO_PROXY}" in
				*=*)
					# Per-shim format: parse semicolon-separated "shim=no_proxy" mappings
					noproxy_mappings=$(echo "${AGENT_NO_PROXY}" | tr ';' ' ')
					for mapping in ${noproxy_mappings}; do
						key="${mapping%%=*}"
						value="${mapping#*=}"
						if [ "${key}" = "${shim}" ]; then
							no_proxy_value="${value}"
							break
						fi
					done
					;;
				*)
					# Simple format: apply to all shims
					no_proxy_value="${AGENT_NO_PROXY}"
					;;
			esac

			if [ -n "${no_proxy_value}" ]; then
				if ! field_contains_value "${kata_config_file}" "kernel_params" "agent.no_proxy"; then
					sed -i -e 's|^kernel_params = "\(.*\)"|kernel_params = "\1 agent.no_proxy='"${no_proxy_value}"'"|g' "${kata_config_file}"
				fi
			fi
		fi

		# Allow enabling debug for Kata Containers
		if [ "${DEBUG}" = "true" ]; then
			if ! config_is_true "${kata_config_file}" "enable_debug"; then
				sed -i -e 's/^#\{0,1\}\(enable_debug\).*=.*$/\1 = true/g' "${kata_config_file}"
			fi
			if ! config_is_true "${kata_config_file}" "debug_console_enabled"; then
				sed -i -e 's/^#\{0,1\}\(debug_console_enabled\).*=.*$/\1 = true/g' "${kata_config_file}"
			fi

			local debug_params=""
			if ! field_contains_value "${kata_config_file}" "kernel_params" "agent.log=debug"; then
				debug_params="${debug_params} agent.log=debug"
			fi
			if ! field_contains_value "${kata_config_file}" "kernel_params" "initcall_debug"; then
				debug_params="${debug_params} initcall_debug"
			fi
			if [ -n "${debug_params}" ]; then
				sed -i -e "s/^kernel_params = \"\(.*\)\"/kernel_params = \"\1${debug_params}\"/g" "${kata_config_file}"
			fi
		fi

		# Apply allowed_hypervisor_annotations:
		#   Here we need to support both cases of:
		#   * A list of annotations, which will be blindly applied to all shims
		#   * A per-shim list of annotations, which will only be applied to the specific shim
		if [ -n "${hypervisor_annotations}" ]; then
			local shim_specific_annotations=""
			local global_annotations=""
			local m
			local key
			local value
			local all_annotations
			local existing_annotations
			local combined_annotations
			local annotations
			local annotation
			local seen_list=""
			local unique_list=""
			local ann
			local final_annotations

			for m in ${hypervisor_annotations}; do
				# Check if this mapping has a colon (shim-specific) or not
				case "${m}" in
					*:*)
						# Shim-specific mapping like "qemu:foo,bar"
						key="${m%:*}"
						value="${m#*:}"

						if [ "${key}" != "${shim}" ]; then
							continue
						fi

						if [ -n "${shim_specific_annotations}" ]; then
							shim_specific_annotations="${shim_specific_annotations},"
						fi
						shim_specific_annotations="${shim_specific_annotations}${value}"
						;;
					*)
						# All shims annotations like "foo bar"
						if [ -n "${global_annotations}" ]; then
							global_annotations="${global_annotations},"
						fi
						global_annotations="${global_annotations}$(echo "${m}" | sed 's/ /,/g')"
						;;
				esac
			done

			# Combine shim-specific and non-shim-specific annotations
			all_annotations="${global_annotations}"
			if [ -n "${shim_specific_annotations}" ]; then
				if [ -n "${all_annotations}" ]; then
					all_annotations="${all_annotations},"
				fi
				all_annotations="${all_annotations}${shim_specific_annotations}"
			fi

			if [ -n "${all_annotations}" ]; then
				existing_annotations=$(get_field_array_values "${kata_config_file}" "enable_annotations")

				# Combine existing and new annotations
				combined_annotations="${existing_annotations}"
				if [ -n "${combined_annotations}" ] && [ -n "${all_annotations}" ]; then
					combined_annotations="${combined_annotations},${all_annotations}"
				elif [ -n "${all_annotations}" ]; then
					combined_annotations="${all_annotations}"
				fi

				# Deduplicate all annotations (ash-compatible approach)
				annotations=$(echo "${combined_annotations}" | tr ',' ' ')
				for annotation in ${annotations}; do
					# Trim whitespace
					annotation=$(echo "${annotation}" | sed 's/^[[:space:]]//;s/[[:space:]]$//')
					if [ -n "${annotation}" ]; then
						# Check if we've seen this annotation before
						if ! echo ",${seen_list}," | grep -q ",${annotation},"; then
							if [ -n "${unique_list}" ]; then
								unique_list="${unique_list} "
							fi
							unique_list="${unique_list}${annotation}"
							seen_list="${seen_list},${annotation}"
						fi
					fi
				done

				if [ -n "${unique_list}" ]; then
					final_annotations=""
					for ann in ${unique_list}; do
						if [ -n "${final_annotations}" ]; then
							final_annotations="${final_annotations}, "
						fi
						final_annotations="${final_annotations}\"${ann}\""
					done
					sed -i -e "s/^enable_annotations = \[.*\]/enable_annotations = [${final_annotations}]/" "${kata_config_file}"
				fi
			fi
		fi

		if echo "${experimental_force_guest_pull}" | grep -Fxq "${shim}"; then
			if ! config_is_true "${kata_config_file}" "experimental_force_guest_pull"; then
				sed -i -e 's/^#\{0,1\}\(experimental_force_guest_pull\).*=.*$/\1 = true/g' "${kata_config_file}"
			fi
		fi

		if echo "$shim" | grep -q "tdx"; then
  			VERSION_ID=version_unset # VERSION_ID may be unset, see https://www.freedesktop.org/software/systemd/man/latest/os-release.html#Notes
			source /host/etc/os-release || source /host/usr/lib/os-release
			case ${ID} in
				ubuntu)
					case ${VERSION_ID} in
						24.04|25.04|25.10)
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
			hypervisor="${shim}"
			case "${shim}" in
				qemu*)
					hypervisor="qemu"
					;;
			esac

			kernel_path=$(tomlq ".hypervisor.${hypervisor}.path" ${kata_config_file} | tr -d \")
			if echo $kernel_path | grep -q "${dest_dir}"; then
				# If we got to this point here, it means that we're dealing with
				# a kata containers configuration file that has already been changed
				# to support multi-install suffix, and we're here most likely due to
				# and update or container restart, and we simply should not try to
				# do anything else, thus just leave the conditional.
				break
			else
				# We could always do this sed, regardless, but I have a strong preference
				# on not touching the configuration files unless extremelly needed
				sed -i -e "s|${default_dest_dir}|${dest_dir}|g" "${kata_config_file}"

				# Let's only adjust qemu_cmdline for the QEMUs that we build and ship ourselves
				case "${shim}" in
					qemu|qemu-snp|qemu-nvidia-gpu|qemu-nvidia-gpu-snp|qemu-nvidia-gpu-tdx|qemu-se|qemu-coco-dev|qemu-cca)
						adjust_qemu_cmdline "${shim}" "${kata_config_file}"
						;;
				esac
			fi
		fi
	done

	# Allow Mariner to use custom configuration.
	if [ "${HOST_OS:-}" = "cbl-mariner" ]; then
		config_path="${host_install_dir}/share/defaults/kata-containers/configuration-clh.toml"

		if ! config_is_true "${config_path}" "static_sandbox_resource_mgmt"; then
			sed -i -E "s|(static_sandbox_resource_mgmt)\s*=\s*false|\1=true|" "${config_path}"
		fi

		clh_path="${dest_dir}/bin/cloud-hypervisor-glibc"

		if ! field_contains_value "${config_path}" "valid_hypervisor_paths" "${clh_path}"; then
			sed -i -E "s|(valid_hypervisor_paths) = .+|\1 = [\"${clh_path}\"]|" "${config_path}"
		fi

		if ! field_contains_value "${config_path}" "path" "${clh_path}"; then
			sed -i -E "s|(path) = \".+/cloud-hypervisor\"|\1 = \"${clh_path}\"|" "${config_path}"
		fi
	fi

	local expand_runtime_classes_for_nfd=$(setup_nfd_rules)

	if [ "${CREATE_RUNTIMECLASSES}" = "true" ]; then
		create_runtimeclasses "${expand_runtime_classes_for_nfd}"
	fi
}

remove_artifacts() {
	echo "deleting kata artifacts"

	rm -rf ${host_install_dir}

	remove_nfd_rules

	if [ "${CREATE_RUNTIMECLASSES}" = "true" ]; then
		delete_runtimeclasses
	fi
}

