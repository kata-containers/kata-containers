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
export PULL_TYPE="${PULL_TYPE:-default}"

declare -r kubernetes_dir=$(dirname "$(readlink -f "$0")")
declare -r runtimeclass_workloads_work_dir="${kubernetes_dir}/runtimeclass_workloads_work"
declare -r runtimeclass_workloads_dir="${kubernetes_dir}/runtimeclass_workloads"
declare -r kata_opa_dir="${kubernetes_dir}/../../../src/kata-opa"
source "${kubernetes_dir}/../../common.bash"
source "${kubernetes_dir}/tests_common.sh"


if [ -n "${K8S_TEST_POLICY_FILES:-}" ]; then
	K8S_TEST_POLICY_FILES=("${K8S_TEST_POLICY_FILES}")
else
	K8S_TEST_POLICY_FILES=( \
		"allow-all.rego" \
		"allow-all-except-exec-process.rego" \
		"allow-set-policy.rego" \
    )
fi

reset_workloads_work_dir() {
	rm -rf "${runtimeclass_workloads_work_dir}"
	cp -R "${runtimeclass_workloads_dir}" "${runtimeclass_workloads_work_dir}"
	setup_policy_files
}

setup_policy_files() {
	# Copy hard-coded policy files used for basic policy testing.
	for policy_file in "${K8S_TEST_POLICY_FILES[@]}"
	do
		cp "${kata_opa_dir}/${policy_file}" "${runtimeclass_workloads_work_dir}"
	done

	# For testing more sophisticated policies, create genpolicy settings that are common for all tests.
	# Some of the tests will make temporary copies of these common settings and customize them as needed.
	create_common_genpolicy_settings "${runtimeclass_workloads_work_dir}"
}

add_annotations_to_yaml() {
	local yaml_file="$1"
	local annotation_name="$2"
	local annotation_value="$3"

	# Previous version of yq was not ready to handle multiple objects in a single yaml.
	# By default was changing only the first object.
	# With yq>4 we need to make it explicit during the read and write.
	local resource_kind="$(yq .kind ${yaml_file} | head -1)"

	case "${resource_kind}" in

	Pod)
		info "Adding \"${annotation_name}=${annotation_value}\" to ${resource_kind} from ${yaml_file}"
		yq -i \
		  ".metadata.annotations.\"${annotation_name}\" = \"${annotation_value}\"" \
		  "${K8S_TEST_YAML}"
		;;

	Deployment|Job|ReplicationController)
		info "Adding \"${annotation_name}=${annotation_value}\" to ${resource_kind} from ${yaml_file}"
		yq -i \
		  ".spec.template.metadata.annotations.\"${annotation_name}\" = \"${annotation_value}\"" \
		  "${K8S_TEST_YAML}"
		;;

	CronJob)
		info "Adding \"${annotation_name}=${annotation_value}\" to ${resource_kind} from ${yaml_file}"
		yq -i \
		  ".spec.jobTemplate.spec.template.metadata.annotations.\"${annotation_name}\" = \"${annotation_value}\"" \
		  "${K8S_TEST_YAML}"
		;;

	List)
		info "Issue #7765: adding annotations to ${resource_kind} from ${yaml_file} is not implemented yet"
		;;

	ConfigMap|LimitRange|Namespace|PersistentVolume|PersistentVolumeClaim|PriorityClass|RuntimeClass|Secret|Service)
		info "Annotations are not required for ${resource_kind} from ${yaml_file}"
		;;

	*)
		info "k8s resource type ${resource_kind} from ${yaml_file} is not yet supported for annotations testing"
		return 1
		;;
	esac
}

add_cbl_mariner_specific_annotations() {
	if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
		info "Add kernel and image path and annotations for cbl-mariner"
		local mariner_annotation_kernel="io.katacontainers.config.hypervisor.kernel"
		local mariner_kernel_path="/usr/share/cloud-hypervisor/vmlinux.bin"

		local mariner_annotation_image="io.katacontainers.config.hypervisor.image"
		local mariner_image_path="/opt/kata/share/kata-containers/kata-containers-mariner.img"

		for K8S_TEST_YAML in runtimeclass_workloads_work/*.yaml
		do
			add_annotations_to_yaml "${K8S_TEST_YAML}" "${mariner_annotation_kernel}" "${mariner_kernel_path}"
			add_annotations_to_yaml "${K8S_TEST_YAML}" "${mariner_annotation_image}" "${mariner_image_path}"
		done
	fi
}

add_runtime_handler_annotations() {
	local handler_annotation="io.containerd.cri.runtime-handler"

	if [ "$PULL_TYPE" != "guest-pull" ]; then
		info "Not adding $handler_annotation annotation for $PULL_TYPE pull type"
		return
	fi

	case "${KATA_HYPERVISOR}" in
		qemu-coco-dev | qemu-sev | qemu-snp | qemu-tdx)
			info "Add runtime handler annotations for ${KATA_HYPERVISOR}"
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
	add_cbl_mariner_specific_annotations
	add_runtime_handler_annotations
}

main "$@"
