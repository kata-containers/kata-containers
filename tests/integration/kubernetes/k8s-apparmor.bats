#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# BATS and tests_common.sh provide the test globals through load.
# shellcheck disable=SC2154

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	if [[ "${KATA_GUEST_APPARMOR_TEST:-no}" != yes ]]; then
		skip "Guest AppArmor integration tests are opt-in"
	fi

	profile="${KATA_GUEST_APPARMOR_PROFILE:-kata-apparmor-test}"
	expected_profile="${KATA_GUEST_APPARMOR_EXPECTED_PROFILE:-${profile}}"
	apparmor_request="${KATA_GUEST_APPARMOR_REQUEST:-localhost/${profile}}"
	image="${KATA_GUEST_APPARMOR_IMAGE:-quay.io/prometheus/busybox:latest}"
	protected_path="${KATA_GUEST_APPARMOR_PROTECTED_PATH:-/tmp/kata-apparmor-deny}"
	case "${KATA_K8S_APPARMOR_API:-annotation}" in
		annotation|field) ;;
		*) die "KATA_K8S_APPARMOR_API must be annotation or field" ;;
	esac
	[[ "${profile}" =~ ^[a-zA-Z0-9_.-]+$ && "${#profile}" -le 127 &&
		"${profile}" != . && "${profile}" != .. && "${profile}" != unconfined ]] || \
		die "KATA_GUEST_APPARMOR_PROFILE is invalid"
	[[ "${expected_profile}" =~ ^[a-zA-Z0-9_.-]+$ && "${#expected_profile}" -le 127 &&
		"${expected_profile}" != . && "${expected_profile}" != .. && "${expected_profile}" != unconfined ]] || \
		die "KATA_GUEST_APPARMOR_EXPECTED_PROFILE is invalid"
	if [[ "${apparmor_request}" != runtime/default &&
		! "${apparmor_request}" =~ ^localhost/[a-zA-Z0-9_.-]+$ ]]; then
		die "KATA_GUEST_APPARMOR_REQUEST must request an enforced profile"
	fi
	[[ "${protected_path}" =~ ^/[a-zA-Z0-9_./-]+$ ]] || die "KATA_GUEST_APPARMOR_PROTECTED_PATH is invalid"

	# Do not delete other tests' pods. Names, manifests and every evidence
	# snapshot are private to this invocation, even with a shared log parent.
	local evidence_parent="${KATA_GUEST_APPARMOR_EVIDENCE_DIR:-${BATS_TEST_TMPDIR:-${TMPDIR:-/tmp}}}"
	mkdir -p "${evidence_parent}"
	evidence_dir="$(mktemp -d "${evidence_parent}/apparmor-${BATS_TEST_NUMBER:-0}.XXXXXX")"
	pod_name="guest-apparmor-${BATS_TEST_NUMBER:-0}-$(basename "${evidence_dir}" | tr '[:upper:]' '[:lower:]' | tr '.' '-')"
	yaml_file="${evidence_dir}/workload.yaml"

	node="${KATA_GUEST_APPARMOR_NODE:-$(get_one_kata_node)}"
	[[ -n "${node}" ]] || die "No Kata node is available"
	kubectl get node "${node}" >/dev/null || die "Node ${node} is not available"
	node_start_time=""
	if [[ "${KATA_GUEST_APPARMOR_SKIP_NODE_DIAGNOSTICS:-no}" != yes ]]; then
		node_start_time="$(measure_node_time "${node}")"
	fi
	export node node_start_time
	runtime_class="${KATA_GUEST_APPARMOR_RUNTIME_CLASS:-$(get_test_runtime_class)}"
	kubectl get runtimeclass "${runtime_class}" >/dev/null || \
		die "RuntimeClass ${runtime_class} is not available"
	write_pod_manifest "${apparmor_request}" "${yaml_file}"
}

# The same checks run at container startup and through kubectl exec. A
# nonzero exec, complain/unconfined mode, or an unrelated write error fails.
apparmor_probe_script() {
	cat <<'EOF'
export LC_ALL=C
current="$(cat /proc/self/attr/current)"
printf 'APPARMOR_CURRENT=%s\n' "$current"
[ "$current" = "$APPARMOR_EXPECTED_PROFILE (enforce)" ]
printf allowed > /tmp/kata-apparmor-allow
[ "$(cat /tmp/kata-apparmor-allow)" = allowed ]
echo APPARMOR_ALLOW_OK
if /bin/sh -c 'printf denied > "$1"' sh "$APPARMOR_PROTECTED_PATH" 2>/tmp/apparmor-write-error; then
  echo APPARMOR_DENY_FAILED
  exit 1
fi
grep -Eiq 'permission denied|operation not permitted' /tmp/apparmor-write-error
echo APPARMOR_DENY_OK
current="$(cat /proc/self/attr/current)"
[ "$current" = "$APPARMOR_EXPECTED_PROFILE (enforce)" ]
printf 'APPARMOR_AFTER=%s\n' "$current"
EOF
}

write_pod_manifest() {
	local request="$1" manifest="$2" container
	{
		cat <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: ${pod_name}
EOF
		if [[ "${KATA_K8S_APPARMOR_API:-annotation}" == annotation ]]; then
			cat <<EOF
  annotations:
    container.apparmor.security.beta.kubernetes.io/app: ${request}
    container.apparmor.security.beta.kubernetes.io/sidecar: ${request}
EOF
		fi
		cat <<EOF
spec:
  runtimeClassName: ${runtime_class}
  restartPolicy: Never
  nodeSelector:
    kubernetes.io/hostname: ${node}
EOF
		if [[ "${KATA_K8S_APPARMOR_API:-annotation}" == field ]]; then
			cat <<EOF
  securityContext:
    appArmorProfile:
EOF
			if [[ "${request}" == runtime/default ]]; then
				echo '      type: RuntimeDefault'
			else
				cat <<EOF
      type: Localhost
      localhostProfile: ${request#localhost/}
EOF
			fi
		fi
		echo '  containers:'
		for container in app sidecar; do
			cat <<EOF
  - name: ${container}
    image: ${image}
    env:
    - name: APPARMOR_EXPECTED_PROFILE
      value: "${expected_profile}"
    - name: APPARMOR_PROTECTED_PATH
      value: "${protected_path}"
    command: ["/bin/sh", "-ec"]
    readinessProbe:
      exec:
        command: ["/bin/sh", "-ec", "test -f /tmp/apparmor-ready"]
      periodSeconds: 1
    args:
    - |
EOF
			apparmor_probe_script | sed 's/^/      /'
			cat <<'EOF'
      : > /tmp/apparmor-ready
      while :; do sleep 5; done
EOF
		done
	} > "${manifest}"
}

collect_evidence() {
	[[ -n "${evidence_dir:-}" ]] || return 0
	local snapshot container
	snapshot="$(mktemp -d "${evidence_dir}/snapshot.XXXXXX")"
	kubectl get pod "${pod_name}" -o yaml > "${snapshot}/pod.yaml" 2> "${snapshot}/pod.stderr" || true
	kubectl describe pod "${pod_name}" > "${snapshot}/pod.describe" 2>&1 || true
	kubectl get events --field-selector "involvedObject.name=${pod_name}" --sort-by=.lastTimestamp \
		> "${snapshot}/events.txt" 2>&1 || true
	for container in app sidecar; do
		kubectl logs "${pod_name}" -c "${container}" > "${snapshot}/${container}.log" \
			2> "${snapshot}/${container}.stderr" || true
	done
}

check_probe_output() {
	grep -Fxq "APPARMOR_CURRENT=${expected_profile} (enforce)" <<< "${output}" &&
		grep -Fxq APPARMOR_ALLOW_OK <<< "${output}" &&
		grep -Fxq APPARMOR_DENY_OK <<< "${output}" &&
		grep -Fxq "APPARMOR_AFTER=${expected_profile} (enforce)" <<< "${output}"
}

@test "Kata guest AppArmor is enforced at startup and through kubectl exec" {
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	for container in app sidecar; do
		run kubectl logs "${pod_name}" -c "${container}"
		[[ "${status}" -eq 0 ]]
		check_probe_output
		run kubectl exec "${pod_name}" -c "${container}" -- /bin/sh -ec "$(apparmor_probe_script)"
		[[ "${status}" -eq 0 ]]
		check_probe_output
	done
	run kubectl get pod "${pod_name}" -o 'jsonpath={.status.conditions[?(@.type=="Ready")].status}'
	[[ "${status}" -eq 0 ]]
	[[ "${output}" == True ]]
	collect_evidence
}

# Accept only an error identifying this missing profile. Scheduling, image
# pulls, parser absence, missing extensions and API errors are not a pass.
is_missing_profile_error() {
	local message="$1"
	[[ "${message}" == *"AppArmor profile ${missing_profile} is missing from the extension"* ||
		"${message}" == *"failed to inspect AppArmor profile ${missing_profile}: No such file or directory"* ||
		"${message}" == *"Cannot enforce AppArmor: profile \"${missing_profile}\" is not loaded"* ||
		"${message}" == *"apparmor profile not found \"${missing_profile}\""* ||
		"${message}" =~ apparmor[[:space:]]profile[[:space:]]not[[:space:]]found[[:space:]]"${missing_profile}"([[:space:]]|$) ||
		( "${message}" == *"AppArmor profile ${missing_profile} is not loaded"* &&
			"${message}" == *"does not exist in the apparmor guest extension"* ) ]]
}

@test "missing AppArmor profile fails with the expected profile error" {
	missing_profile="${profile:0:90}-missing-${pod_name##*-}"
	missing_yaml_file="${evidence_dir}/workload-missing.yaml"
	write_pod_manifest "localhost/${missing_profile}" "${missing_yaml_file}"

	run kubectl create -f "${missing_yaml_file}"
	if [[ "${status}" -ne 0 ]]; then
		is_missing_profile_error "${output}"
	else
		run kubectl wait --for=condition=Ready --timeout=30s "pod/${pod_name}"
		[[ "${status}" -ne 0 ]]
		run kubectl get pod "${pod_name}" -o 'jsonpath={.status.conditions[?(@.type=="Ready")].status}'
		[[ "${status}" -eq 0 ]]
		[[ "${output}" != True ]]
		run kubectl get pod "${pod_name}" -o yaml
		[[ "${status}" -eq 0 ]]
		local pod_status="${output}"
		run kubectl get pod "${pod_name}" -o 'jsonpath={.metadata.uid}'
		[[ "${status}" -eq 0 ]]
		[[ -n "${output}" ]]
		local pod_uid="${output}"
		run kubectl get events --field-selector "involvedObject.uid=${pod_uid}" \
			-o 'jsonpath={range .items[*]}{.message}{"\n"}{end}'
		[[ "${status}" -eq 0 ]]
		is_missing_profile_error "${pod_status}"$'\n'"${output}"
	fi
	collect_evidence
}

teardown() {
	collect_evidence
	if [[ -n "${yaml_file:-}" ]]; then
		kubectl delete pod "${pod_name}" --ignore-not-found=true --timeout="${timeout}" || true
	fi
	if [[ -n "${node_start_time:-}" && -n "${evidence_dir:-}" ]]; then
		local snapshot node_end_time
		snapshot="$(mktemp -d "${evidence_dir}/node.XXXXXX")"
		node_end_time="$(measure_node_time "${node}")"
		print_node_journal_since_test_start "${node}" "${node_start_time}" "${BATS_TEST_COMPLETED:-}" \
			> "${snapshot}/journal.log" 2>&1 || true
		printf '%s\n%s\n' "${node_start_time}" "${node_end_time}" > "${snapshot}/interval.txt"
	fi
}
