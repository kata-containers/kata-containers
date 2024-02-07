#!/usr/bin/env bash
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

DEBUG="${DEBUG:-}"
[ -n "$DEBUG" ] && set -x

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

add_kernel_initrd_annotations_to_yaml() {
	local yaml_file="$1"
	local mariner_kernel_path="/usr/share/cloud-hypervisor/vmlinux.bin"
	local mariner_initrd_path="/opt/kata/share/kata-containers/kata-containers-initrd-mariner.img"
	local resource_kind="$(yq read ${yaml_file} kind)"

	case "${resource_kind}" in

	Pod)
		echo "Adding kernel and initrd annotations to ${resource_kind} from ${yaml_file}"
		yq write -i "${K8S_TEST_YAML}" 'metadata.annotations[io.katacontainers.config.hypervisor.kernel]' "${mariner_kernel_path}"
		yq write -i "${K8S_TEST_YAML}" 'metadata.annotations[io.katacontainers.config.hypervisor.initrd]' "${mariner_initrd_path}"
		;;

	Deployment|Job|ReplicationController)
		echo "Adding kernel and initrd annotations to ${resource_kind} from ${yaml_file}"
		yq write -i "${K8S_TEST_YAML}" 'spec.template.metadata.annotations[io.katacontainers.config.hypervisor.kernel]' "${mariner_kernel_path}"
		yq write -i "${K8S_TEST_YAML}" 'spec.template.metadata.annotations[io.katacontainers.config.hypervisor.initrd]' "${mariner_initrd_path}"
		;;

	List)
		echo "Issue #7765: adding kernel and initrd annotations to ${resource_kind} from ${yaml_file} is not implemented yet"
		;;

	ConfigMap|LimitRange|Namespace|PersistentVolume|PersistentVolumeClaim|RuntimeClass|Secret|Service)
		echo "Kernel and initrd annotations are not required for ${resource_kind} from ${yaml_file}"
		;;

	*)
		echo "k8s resource type ${resource_kind} from ${yaml_file} is not yet supported for kernel and initrd annotations testing"
		return 1
		;;
	esac
}

add_kernel_initrd_annotations() {
	if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
		info "Add kernel and initrd annotations"
		for K8S_TEST_YAML in runtimeclass_workloads_work/*.yaml
		do
			add_kernel_initrd_annotations_to_yaml "${K8S_TEST_YAML}"
		done
	fi
}

main() {
	ensure_yq
	reset_workloads_work_dir
	add_kernel_initrd_annotations
}

main "$@"
