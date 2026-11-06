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
	if [[ "${KATA_GUEST_APPARMOR_TEST:-no}" != yes && "${KATA_GUEST_APPARMOR_BASELINE:-no}" != yes ]]; then
		skip "Guest AppArmor integration tests require an explicit CI mode"
	fi
	mode=disabled
	[[ "${KATA_GUEST_APPARMOR_TEST:-no}" != yes ]] || mode=enabled
	profile="${KATA_GUEST_APPARMOR_PROFILE:-kata-apparmor-test}"
	[[ "${profile}" =~ ^[a-zA-Z0-9_.-]+$ && "${#profile}" -le 127 &&
		"${profile}" != . && "${profile}" != .. && "${profile}" != unconfined ]] || die "invalid test profile"
	image="${KATA_GUEST_APPARMOR_IMAGE:-quay.io/prometheus/busybox:latest}"
	case "${KATA_K8S_APPARMOR_API:-field}" in
		annotation|field) ;;
		*) die "KATA_K8S_APPARMOR_API must be annotation or field" ;;
	esac
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
	kubectl get runtimeclass "${runtime_class}" >/dev/null || die "RuntimeClass ${runtime_class} is not available"
	select_case "localhost/${profile}"
}

require_enabled() { [[ "${mode}" == enabled ]] || skip "requires enabled mode and the test extension"; }

select_case() {
	local request="$1"
	case "${request}" in
		omitted|runtime/default|localhost/cri-containerd.apparmor.d)
			expected_profile=cri-containerd.apparmor.d; probe_kind=default ;;
		unconfined) expected_profile=unconfined; probe_kind=unconfined ;;
		localhost/*) expected_profile="${request#localhost/}"; probe_kind=fixture ;;
		*) die "unsupported test request: ${request}" ;;
	esac
	if [[ "${mode}" == disabled ]]; then expected_profile=unconfined; probe_kind=baseline; fi
	write_pod_manifest "${request}" "${yaml_file}"
}

# kubectl exec supplies no AppArmor profile field: this probes inheritance only.
# Native OCI/RPC tests must cover explicit same/different/downgrade requests.
apparmor_probe_script() {
	cat <<'EOF'
export LC_ALL=C
current="$(cat /proc/self/attr/current)"
printf 'APPARMOR_CURRENT=%s\n' "$current"
expected="$APPARMOR_EXPECTED_PROFILE (enforce)"
[ "$APPARMOR_EXPECTED_PROFILE" != unconfined ] || expected=unconfined
[ "$current" = "$expected" ]
printf allowed > /tmp/kata-apparmor-allow
[ "$(cat /tmp/kata-apparmor-allow)" = allowed ]
echo APPARMOR_ALLOW_OK
case "$APPARMOR_PROBE_KIND" in
  fixture)
    if /bin/sh -c 'printf denied > /tmp/kata-apparmor-deny' 2>/tmp/apparmor-error; then
      echo APPARMOR_DENY_FAILED; exit 1
    fi
    grep -Eiq 'permission denied|operation not permitted' /tmp/apparmor-error
    echo APPARMOR_DENY_OK ;;
  default|unconfined)
    # Both cases get SYS_ADMIN. The Unconfined case must successfully perform
    # this exact mount, providing a control against missing caps/tools/kernel.
    command -v mount
    command -v umount
    mkdir -p /tmp/kata-apparmor-mount
    if mount -t tmpfs -o size=64k tmpfs /tmp/kata-apparmor-mount 2>/tmp/apparmor-error; then
      umount /tmp/kata-apparmor-mount
      [ "$APPARMOR_PROBE_KIND" = unconfined ]
      echo APPARMOR_MOUNT_ALLOWED
    else
      [ "$APPARMOR_PROBE_KIND" = default ]
      grep -Eiq 'permission denied|operation not permitted' /tmp/apparmor-error
      echo APPARMOR_DENY_OK
    fi ;;
  baseline) echo APPARMOR_UNCONFINED_OK ;;
  *) exit 1 ;;
esac
current="$(cat /proc/self/attr/current)"
[ "$current" = "$expected" ]
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
		if [[ "${request}" != omitted && "${KATA_K8S_APPARMOR_API:-field}" == annotation ]]; then
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
		if [[ "${request}" != omitted && "${KATA_K8S_APPARMOR_API:-field}" == field ]]; then
			printf '  securityContext:\n    appArmorProfile:\n'
			case "${request}" in
				runtime/default) echo '      type: RuntimeDefault' ;;
				unconfined) echo '      type: Unconfined' ;;
				localhost/*) printf '      type: Localhost\n      localhostProfile: %s\n' "${request#localhost/}" ;;
				*) die "invalid manifest request" ;;
			esac
		fi
		echo '  containers:'
		for container in app sidecar; do
			cat <<EOF
  - name: ${container}
    image: ${image}
EOF
			if [[ "${probe_kind}" == default || "${probe_kind}" == unconfined ]]; then
				printf '    securityContext:\n      capabilities:\n        add: ["SYS_ADMIN"]\n'
			fi
			cat <<EOF
    env:
    - name: APPARMOR_EXPECTED_PROFILE
      value: "${expected_profile}"
    - name: APPARMOR_PROBE_KIND
      value: "${probe_kind}"
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
	local label="${expected_profile} (enforce)" marker=APPARMOR_DENY_OK
	[[ "${expected_profile}" != unconfined ]] || label=unconfined
	case "${probe_kind}" in
		unconfined) marker=APPARMOR_MOUNT_ALLOWED ;;
		baseline) marker=APPARMOR_UNCONFINED_OK ;;
	esac
	grep -Fxq "APPARMOR_CURRENT=${label}" <<< "${output}" &&
		grep -Fxq APPARMOR_ALLOW_OK <<< "${output}" &&
		grep -Fxq "${marker}" <<< "${output}" &&
		grep -Fxq "APPARMOR_AFTER=${label}" <<< "${output}"
}

assert_ready_and_inheritance() {
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"
	local container
	for container in app sidecar; do
		run kubectl logs "${pod_name}" -c "${container}"
		[[ "${status}" -eq 0 ]]
		check_probe_output
		run kubectl exec "${pod_name}" -c "${container}" -- /bin/sh -ec "$(apparmor_probe_script)"
		printf '%s\n' "${output}" > "${evidence_dir}/exec-${container}.log"
		[[ "${status}" -eq 0 ]]
		check_probe_output
	done
	run kubectl get pod "${pod_name}" -o 'jsonpath={.status.conditions[?(@.type=="Ready")].status}'
	[[ "${status}" -eq 0 && "${output}" == True ]]
	collect_evidence
}

# Separate exact-name, exact-layer oracles. Host CRI rejections never prove an
# agent rejection. No-extension, parser, image and unrelated errors fail here.
is_missing_profile_error() {
	local message="$1" layer="${2:-guest}"
	case "${layer}" in
		guest)
			[[ "${message}" == *"AppArmor profile ${missing_profile} is missing from the extension:"* ||
				( "${message}" == *"AppArmor profile ${missing_profile} is not loaded"* &&
				"${message}" == *"does not exist in the apparmor guest extension"* ) ]] ;;
		host)
			[[ "${message}" == *"Cannot enforce AppArmor: profile \"${missing_profile}\" is not loaded"* ||
				"${message}" == *"apparmor profile not found \"${missing_profile}\""* ||
				"${message}" =~ apparmor[[:space:]]profile[[:space:]]not[[:space:]]found[[:space:]]"${missing_profile}"([[:space:]]|$) ]] ;;
		*) return 1 ;;
	esac
}

assert_missing_profile() {
	local layer="$1"
	missing_yaml_file="${evidence_dir}/workload-missing.yaml"
	write_pod_manifest "localhost/${missing_profile}" "${missing_yaml_file}"
	# A rejected API request has not established the expected runtime layer.
	kubectl create -f "${missing_yaml_file}"
	run kubectl wait --for=condition=Ready --timeout=30s "pod/${pod_name}"
	[[ "${status}" -ne 0 ]]
	run kubectl get pod "${pod_name}" -o 'jsonpath={.status.conditions[?(@.type=="Ready")].status}'
	[[ "${status}" -eq 0 && "${output}" != True ]]
	# CRI may allocate an ID before the agent rejects task creation. Check that
	# no workload started, rather than confusing an allocated ID with execution.
	run kubectl get pod "${pod_name}" -o json
	[[ "${status}" -eq 0 ]]
	jq -e 'all(.status.containerStatuses[]?;
		.started != true and .state.running == null and
		((.restartCount // 0) == 0) and ((.lastState // {}) == {}) and
		(.state.terminated == null or
		 (.state.terminated.reason == "StartError" and
		  .state.terminated.startedAt == "1970-01-01T00:00:00Z")))' <<< "${output}"
	run kubectl get pod "${pod_name}" -o yaml
	[[ "${status}" -eq 0 ]]
	local pod_status="${output}"
	run kubectl get pod "${pod_name}" -o 'jsonpath={.metadata.uid}'
	[[ "${status}" -eq 0 && -n "${output}" ]]
	local pod_uid="${output}"
	run kubectl get events --field-selector "involvedObject.uid=${pod_uid}" \
		-o 'jsonpath={range .items[*]}{.message}{"\n"}{end}'
	[[ "${status}" -eq 0 ]]
	is_missing_profile_error "${pod_status}"$'\n'"${output}" "${layer}"
	collect_evidence
}

@test "Localhost fixture is enforced at startup and inherited by kubectl exec" {
	require_enabled
	assert_ready_and_inheritance
}

@test "host-accepted profile missing from guest is rejected by the agent" {
	require_enabled
	# guest-apparmor-ci.sh loads this renamed real fixture on the host only.
	missing_profile=kata-apparmor-guest-missing
	assert_missing_profile guest
}

@test "omitted AppArmor field uses the literal containerd default in the guest" {
	require_enabled
	select_case omitted
	assert_ready_and_inheritance
}

@test "RuntimeDefault uses the literal containerd default in the guest" {
	require_enabled
	select_case runtime/default
	assert_ready_and_inheritance
}

@test "Unconfined runs unconfined and permits the default-policy mount control" {
	require_enabled
	select_case unconfined
	assert_ready_and_inheritance
}

@test "Localhost default-name collision selects the same guest default policy" {
	require_enabled
	select_case localhost/cri-containerd.apparmor.d
	assert_ready_and_inheritance
}

@test "host-missing Localhost profile is rejected at host CRI, not counted as guest enforcement" {
	require_enabled
	missing_profile="kata-apparmor-host-missing-${pod_name##*-}"
	assert_missing_profile host
}

@test "flag false without an extension preserves ordinary default Pod startup" {
	[[ "${mode}" == disabled ]] || skip "requires the ordinary no-extension job"
	select_case omitted
	assert_ready_and_inheritance
}

@test "flag false preserves RuntimeDefault startup without guest policy lookup" {
	[[ "${mode}" == disabled ]] || skip "requires the ordinary no-extension job"
	select_case runtime/default
	assert_ready_and_inheritance
}

@test "flag false preserves Unconfined startup and exec without an extension" {
	[[ "${mode}" == disabled ]] || skip "requires the ordinary no-extension job"
	select_case unconfined
	assert_ready_and_inheritance
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
