#!/usr/bin/env bash
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

DEBUG="${DEBUG:-}"
[ -n "$DEBUG" ] && set -x

export AUTO_GENERATE_POLICY="${AUTO_GENERATE_POLICY:-no}"
export KATA_HOST_OS="${KATA_HOST_OS:-}"
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-}"

if [ -n "${K8S_TEST_POLICY_FILES:-}" ]; then
	K8S_TEST_POLICY_FILES=($K8S_TEST_POLICY_FILES)
else
	K8S_TEST_POLICY_FILES=( \
		"allow-all.rego" \
		"allow-all-except-exec-process.rego" \
    )
fi

declare -r kubernetes_dir=$(dirname "$(readlink -f "$0")")
source "${kubernetes_dir}/../../common.bash"
source "${kubernetes_dir}/tests_common.sh"

reset_workloads_work_dir() {
	rm -rf ${kubernetes_dir}/runtimeclass_workloads_work
	cp -R ${kubernetes_dir}/runtimeclass_workloads ${kubernetes_dir}/runtimeclass_workloads_work
	setup_policy_files
}

setup_policy_files() {
	declare -r kata_opa_dir="${kubernetes_dir}/../../../src/kata-opa"
	declare -r workloads_work_dir="${kubernetes_dir}/runtimeclass_workloads_work"

	# Copy hard-coded policy files used for basic policy testing.
	for policy_file in ${K8S_TEST_POLICY_FILES[@]}
	do
		cp "${kata_opa_dir}/${policy_file}" ${kubernetes_dir}/runtimeclass_workloads_work/
	done

	# For testing more sophisticated policies, create genpolicy settings that are common for all tests.
	# Some of the tests will make temporary copies of these common settings and customize them as needed.
	create_common_genpolicy_settings "${workloads_work_dir}"
}

add_annotations_to_yaml() {
	local yaml_file="$1"
	local annotation_name="$2"
	local annotation_value="$3"
	local resource_kind="$(yq read ${yaml_file} kind)"

	case "${resource_kind}" in

	Pod)
		info "Adding \"${annotation_name}=${annotation_value}\" to ${resource_kind} from ${yaml_file}"
		yq write -i \
		  "${K8S_TEST_YAML}" \
		  "metadata.annotations[${annotation_name}]" \
		  "${annotation_value}"
		;;

	Deployment|Job|ReplicationController)
		info "Adding \"${annotation_name}=${annotation_value}\" to ${resource_kind} from ${yaml_file}"
		yq write -i \
		  "${K8S_TEST_YAML}" \
		  "spec.template.metadata.annotations[${annotation_name}]" \
		  "${annotation_value}"
		;;

	List)
		info "Issue #7765: adding annotations to ${resource_kind} from ${yaml_file} is not implemented yet"
		;;

	ConfigMap|LimitRange|Namespace|PersistentVolume|PersistentVolumeClaim|RuntimeClass|Secret|Service)
		info "Annotations are not required for ${resource_kind} from ${yaml_file}"
		;;

	*)
		info "k8s resource type ${resource_kind} from ${yaml_file} is not yet supported for annotations testing"
		return 1
		;;
	esac
}

add_cbl_mariner_kernel_initrd_annotations() {
	if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
		info "Add kernel and initrd path and annotations for cbl-mariner"
		local mariner_annotation_kernel="io.katacontainers.config.hypervisor.kernel"
		local mariner_kernel_path="/usr/share/cloud-hypervisor/vmlinux.bin"

		local mariner_annotation_initrd="io.katacontainers.config.hypervisor.initrd"
		local mariner_initrd_path="/opt/kata/share/kata-containers/kata-containers-initrd-mariner.img"

		for K8S_TEST_YAML in runtimeclass_workloads_work/*.yaml
		do
			add_annotations_to_yaml "${K8S_TEST_YAML}" "${mariner_annotation_initrd}" "${mariner_initrd_path}"
		done
	fi
}

add_runtime_handler_annotations() {
	case "${KATA_HYPERVISOR}" in
		qemu-tdx)
			info "Add runtime handler annotations for ${KATA_HYPERVISOR}"
			local handler_annotation="io.containerd.cri.runtime-handler"
			local handler_value="kata-${KATA_HYPERVISOR}"
			for K8S_TEST_YAML in runtimeclass_workloads_work/*.yaml
			do
				add_annotations_to_yaml "${K8S_TEST_YAML}" "${handler_annotation}" "${handler_value}"
			done
			;;
	esac
}

main() {
	ensure_yq
	reset_workloads_work_dir
	add_cbl_mariner_kernel_initrd_annotations
	add_runtime_handler_annotations
}

main "$@"
