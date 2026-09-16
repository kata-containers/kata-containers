#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	if [[ "${KATA_GUEST_APPARMOR_TEST:-no}" != "yes" ]]; then
		skip "Guest AppArmor integration tests are opt-in"
	fi

	profile="${KATA_GUEST_APPARMOR_PROFILE:-}"
	apparmor_request="${KATA_GUEST_APPARMOR_REQUEST:-runtime/default}"
	image="${KATA_GUEST_APPARMOR_IMAGE:-quay.io/prometheus/busybox:latest}"
	protected_path="${KATA_GUEST_APPARMOR_PROTECTED_PATH:-/tmp/kata-apparmor-deny}"
	case "${KATA_K8S_APPARMOR_API:-annotation}" in
		annotation|field)
			;;
		*)
			die "KATA_K8S_APPARMOR_API must be annotation or field"
			;;
	esac
	[[ "${profile}" =~ ^[[:alnum:]_.-]+$ ]] || die "KATA_GUEST_APPARMOR_PROFILE is invalid"
	case "${apparmor_request}" in
		runtime/default|unconfined|localhost/[[:alnum:]_.-]*)
			;;
		*)
			die "KATA_GUEST_APPARMOR_REQUEST must be runtime/default, unconfined, or localhost/<profile>"
			;;
	esac
	[[ "${protected_path}" =~ ^/[[:alnum:]_./-]+$ ]] || die "KATA_GUEST_APPARMOR_PROTECTED_PATH is invalid"

	if [[ -n "${KATA_GUEST_APPARMOR_NODE:-}" ]]; then
		node="${KATA_GUEST_APPARMOR_NODE}"
		kubectl get node "${node}" >/dev/null || die "Node ${node} is not available"
		if [[ "${KATA_GUEST_APPARMOR_SKIP_NODE_DIAGNOSTICS:-no}" == "yes" ]]; then
			node_start_time=""
		else
			node_start_time="$(measure_node_time "${node}")"
		fi
		export node
		export node_start_time
		k8s_delete_all_pods_if_any_exists || true
		get_pod_config_dir
	else
		setup_common || die "setup_common failed"
	fi
	runtime_class="${KATA_GUEST_APPARMOR_RUNTIME_CLASS:-$(get_test_runtime_class)}"
	kubectl get runtimeclass "${runtime_class}" >/dev/null || \
		die "RuntimeClass ${runtime_class} is not available"
	yaml_file="${pod_config_dir}/pod-apparmor.yaml"
	mkdir -p "${pod_config_dir}"
	evidence_dir="${KATA_GUEST_APPARMOR_EVIDENCE_DIR:-}"

	{
		cat <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: guest-apparmor-test
EOF
		if [[ "${KATA_K8S_APPARMOR_API:-annotation}" == "annotation" ]]; then
			cat <<EOF
  annotations:
    container.apparmor.security.beta.kubernetes.io/app: ${apparmor_request}
    container.apparmor.security.beta.kubernetes.io/sidecar: ${apparmor_request}
EOF
		fi
		cat <<EOF
spec:
  runtimeClassName: ${runtime_class}
EOF
		if [[ -n "${KATA_GUEST_APPARMOR_NODE:-}" ]]; then
			cat <<EOF
  nodeSelector:
    kubernetes.io/hostname: ${node}
EOF
		fi
		if [[ "${KATA_K8S_APPARMOR_API:-annotation}" == "field" ]]; then
			if [[ "${apparmor_request}" == "runtime/default" ]]; then
				cat <<EOF
  securityContext:
    appArmorProfile:
      type: RuntimeDefault
EOF
			elif [[ "${apparmor_request}" == "unconfined" ]]; then
				cat <<EOF
  securityContext:
    appArmorProfile:
      type: Unconfined
EOF
			else
				cat <<EOF
  securityContext:
    appArmorProfile:
      type: Localhost
      localhostProfile: ${apparmor_request#localhost/}
EOF
			fi
		fi
		cat <<EOF
  containers:
  - name: app
    image: ${image}
    command: ["/bin/sh", "-c"]
    args:
    - |
      echo "APPARMOR_CURRENT=\$(cat /proc/self/attr/current)"
      if /bin/sh -c 'echo denied > ${protected_path}' 2>/tmp/apparmor-write-error; then
        echo "APPARMOR_DENY_FAILED"
        exit 1
      fi
      if ! grep -Eiq "permission denied|operation not permitted" /tmp/apparmor-write-error; then
        cat /tmp/apparmor-write-error
        exit 1
      fi
      echo "APPARMOR_DENY_OK"
      sleep 30
  - name: sidecar
    image: ${image}
    command: ["/bin/sh", "-c"]
    args:
    - |
      echo "APPARMOR_SIDECAR_CURRENT=\$(cat /proc/self/attr/current)"
      sleep 30
EOF
	} > "${yaml_file}"
}

collect_evidence() {
	[[ -n "${evidence_dir:-}" ]] || return 0
	mkdir -p "${evidence_dir}"
	kubectl get pod guest-apparmor-test -o yaml > "${evidence_dir}/pod.yaml" || true
	kubectl describe pod guest-apparmor-test > "${evidence_dir}/pod.describe" || true
	kubectl get events --sort-by=.lastTimestamp > "${evidence_dir}/events.txt" || true

	local container_id
	container_id="$(kubectl get pod guest-apparmor-test \
		-o jsonpath='{.status.containerStatuses[0].containerID}' 2>/dev/null \
		| sed 's#^[^:]*://##')"
	if [[ -n "${container_id}" ]] && command -v crictl >/dev/null 2>&1; then
		crictl inspect "${container_id}" > "${evidence_dir}/cri-inspect.json" 2>/dev/null || \
			rm -f "${evidence_dir}/cri-inspect.json"
	fi
}

@test "Kata guest AppArmor denies a protected write" {
	pod_name="guest-apparmor-test"
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	run kubectl logs "${pod_name}" -c app
	[ "${status}" -eq 0 ]
	[[ "${output}" == *"APPARMOR_DENY_OK"* ]]
	[[ "${output}" == *"${KATA_GUEST_APPARMOR_EXPECTED_PROFILE:-${profile}}"* ]]
	run kubectl logs "${pod_name}" -c sidecar
	[ "${status}" -eq 0 ]
	[[ "${output}" == *"APPARMOR_SIDECAR_CURRENT"* ]]
	[[ "${output}" == *"${KATA_GUEST_APPARMOR_EXPECTED_PROFILE:-${profile}}"* ]]
	collect_evidence
}

@test "missing or Host-unavailable AppArmor profile never reaches Ready" {
	missing_profile="${profile}-missing"
	missing_yaml_file="${pod_config_dir}/pod-apparmor-missing.yaml"
	missing_request="localhost/${missing_profile}"
	if [[ "${KATA_K8S_APPARMOR_API:-annotation}" == "annotation" ]]; then
		sed "s#${apparmor_request}#${missing_request}#g" \
			"${yaml_file}" > "${missing_yaml_file}"
	else
		sed -e "s/type: RuntimeDefault/type: Localhost\\n      localhostProfile: ${missing_profile}/" \
			-e "s/type: Unconfined/type: Localhost\\n      localhostProfile: ${missing_profile}/" \
			-e "s/localhostProfile: .*/localhostProfile: ${missing_profile}/" \
			"${yaml_file}" > "${missing_yaml_file}"
	fi

	run kubectl create -f "${missing_yaml_file}"
	if [[ "${status}" -eq 0 ]]; then
		run kubectl wait --for=condition=Ready --timeout=30s pod/guest-apparmor-test
		[ "${status}" -ne 0 ]
	fi
	collect_evidence
}

teardown() {
	collect_evidence
	if [[ -n "${yaml_file:-}" ]]; then
		kubectl delete -f "${yaml_file}" --ignore-not-found=true || true
	fi
	if [[ -n "${missing_yaml_file:-}" ]]; then
		kubectl delete -f "${missing_yaml_file}" --ignore-not-found=true || true
	fi
	if [[ -n "${node:-}" && "${KATA_GUEST_APPARMOR_SKIP_NODE_DIAGNOSTICS:-no}" != "yes" ]]; then
		teardown_common "${node}" "${node_start_time:-}"
	fi
}
