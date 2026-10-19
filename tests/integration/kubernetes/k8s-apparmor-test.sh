#!/usr/bin/env bash
# Copyright (c) 2026 Kata Containers contributors
# SPDX-License-Identifier: Apache-2.0
# The dynamically sourced BATS bodies read and assign the test globals.
# shellcheck disable=SC2034,SC2154

# Exercise the BATS assertions with an in-process kubectl mock. This is a
# shell regression test, not a Kubernetes/AppArmor end-to-end test.
set -euo pipefail
test_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
test_tmp="$(mktemp -d)"
trap 'rm -rf -- "${test_tmp:?}"' EXIT
# Only BATS load/@test syntax is translated; the actual setup, probe,
# manifest generator and test bodies below are those in k8s-apparmor.bats.
# shellcheck source=/dev/null
source <(awk '
	/^load / { next }
	/^@test / { print "apparmor_test_" ++n "() {"; next }
	{ print }
' "${test_dir}/k8s-apparmor.bats")
case_number=0
failed=0

die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
skip() { printf 'SKIP: %s\n' "$*"; exit 0; }
get_one_kata_node() { printf '%s\n' unit-node; }
get_test_runtime_class() { printf '%s\n' unit-runtime; }
run() {
	if output="$("$@" 2>&1)"; then status=0; else status=$?; fi
}
must_not() {
	if "$@"; then
		printf 'Unexpected success: %s\n' "$*" >&2
		return 1
	fi
}

probe_output() {
	local mode=enforce
	if [[ "${mock_bad_stage}" == "$1" && "${mock_bad_container}" == "$2" ]]; then
		case "${mock_bad_probe}" in
			complain) mode=complain ;;
			unconfined) printf '%s\n' 'APPARMOR_CURRENT=unconfined'; return 0 ;;
			wrong-profile) printf '%s\n' "APPARMOR_CURRENT=prefix-${expected_profile} (enforce)"; return 0 ;;
			missing-allow) printf '%s\n' "APPARMOR_CURRENT=${expected_profile} (enforce)" APPARMOR_DENY_OK;
				printf 'APPARMOR_AFTER=%s (enforce)\n' "${expected_profile}"; return 0 ;;
			missing-deny) printf '%s\n' "APPARMOR_CURRENT=${expected_profile} (enforce)" APPARMOR_ALLOW_OK; return 0 ;;
			mode-changed) printf '%s\n' "APPARMOR_CURRENT=${expected_profile} (enforce)" APPARMOR_ALLOW_OK APPARMOR_DENY_OK;
				printf 'APPARMOR_AFTER=%s (complain)\n' "${expected_profile}"; return 0 ;;
			command-failed) printf '%s\n' 'exec command failed'; return 1 ;;
		esac
	fi
	printf 'APPARMOR_CURRENT=%s (%s)\n' "${expected_profile}" "${mode}"
	printf '%s\n' APPARMOR_ALLOW_OK APPARMOR_DENY_OK
	printf 'APPARMOR_AFTER=%s (%s)\n' "${expected_profile}" "${mode}"
}

missing_error() { printf 'AppArmor profile %s is missing from the extension\n' "${missing_profile}"; }

kubectl() {
	printf '%q ' "$@" >> "${trace}"
	printf '\n' >> "${trace}"
	local args="$*"
	case "$1" in
		create)
			case "${mock_scenario}" in
				create-missing) missing_error; return 1 ;;
				create-unrelated) printf '%s\n' 'connection refused'; return 1 ;;
			esac
			printf 'pod/%s created\n' "${pod_name}"
			;;
		wait)
			if [[ "${mock_scenario}" == positive ]]; then return 0; fi
			printf '%s\n' 'error: timed out waiting for the condition'
			return 1
			;;
		logs|exec)
			[[ "$3" == -c ]] || return 91
			probe_output "$1" "$4"
			;;
		get)
			case "$2" in
				node|runtimeclass) return 0 ;;
				pod)
					if [[ "${mock_scenario}" == api-failure ]]; then return 1; fi
					if [[ "${args}" == *jsonpath=*Ready* ]]; then
						if [[ "${mock_scenario}" == positive || "${mock_scenario}" == became-ready ]]; then
							printf '%s' True
						else printf '%s' False; fi
					elif [[ "${args}" == *jsonpath=*metadata.uid* ]]; then
						printf '%s' unit-pod-uid
					elif [[ "${mock_scenario}" == missing ]]; then missing_error
					elif [[ "${mock_scenario}" == unrelated ]]; then printf '%s\n' ImagePullBackOff
					elif [[ "${mock_scenario}" == wrong-profile ]]; then
						printf '%s\n' 'AppArmor profile a-different-profile is missing from the extension'
					fi
					;;
				events)
					case "${mock_scenario}" in
						missing|events-only|became-ready) missing_error ;;
						unrelated) printf '%s\n' 'FailedScheduling: insufficient memory' ;;
						wrong-profile) printf '%s\n' 'AppArmor profile a-different-profile is missing from the extension' ;;
					esac
					;;
				*) return 92 ;;
			esac
			;;
		describe|delete) return 0 ;;
		*) printf 'Unexpected mock kubectl call: %s\n' "$*" >&2; return 93 ;;
	esac
}

fixture() {
	trace="${test_tmp}/trace-${case_number}"
	: > "${trace}"
	KATA_GUEST_APPARMOR_TEST=yes
	KATA_GUEST_APPARMOR_PROFILE=unit-test
	KATA_GUEST_APPARMOR_EXPECTED_PROFILE=unit-test
	KATA_GUEST_APPARMOR_REQUEST=localhost/unit-test
	KATA_GUEST_APPARMOR_NODE=unit-node
	KATA_GUEST_APPARMOR_RUNTIME_CLASS=unit-runtime
	KATA_GUEST_APPARMOR_SKIP_NODE_DIAGNOSTICS=yes
	KATA_GUEST_APPARMOR_EVIDENCE_DIR="${test_tmp}/shared-evidence"
	KATA_K8S_APPARMOR_API=annotation
	BATS_TEST_NUMBER="${case_number}"
	timeout=1s
	mock_scenario=positive
	mock_bad_stage=none
	mock_bad_container=app
	mock_bad_probe=none
	unset evidence_dir yaml_file missing_yaml_file node_start_time
}

check() {
	local description="$1" expected="$2" result
	shift 2
	case_number=$((case_number + 1))
	set +e
	(set -e; fixture; "$@") > "${test_tmp}/stdout" 2> "${test_tmp}/stderr"
	result=$?
	set -e
	if [[ "${expected}" == pass && "${result}" -eq 0 ]] ||
		{ [[ "${expected}" == fail:* && "${result}" -ne 0 ]] &&
			grep -q "^${expected#fail:} " "${test_tmp}/trace-${case_number}"; }; then
		printf 'ok %s - %s\n' "${case_number}" "${description}"
	else
		printf 'not ok %s - %s (exit %s)\n' "${case_number}" "${description}" "${result}"
		cat "${test_tmp}/stdout" "${test_tmp}/stderr"
		failed=$((failed + 1))
	fi
}

positive() {
	setup
	apparmor_test_1
	[[ "$(grep -c '^exec ' "${trace}")" -eq 2 ]]
	grep -q '^exec .* -c app ' "${trace}"
	grep -q '^exec .* -c sidecar ' "${trace}"
	[[ "$(grep -c 'readinessProbe:' "${yaml_file}")" -eq 2 ]]
	grep -q 'while :; do sleep 5; done' "${yaml_file}"
	must_not grep -Eq '^delete .*--all' "${trace}"
}
bad_positive() {
	mock_bad_stage="$1"; mock_bad_container="$2"; mock_bad_probe="$3"
	setup
	apparmor_test_1
}
negative() { mock_scenario="$1"; setup; apparmor_test_2; }
events_only() {
	negative events-only
	grep -q 'involvedObject.uid=unit-pod-uid' "${trace}"
}
unique_evidence() {
	setup
	local first_dir="${evidence_dir}" first_pod="${pod_name}" snapshot
	collect_evidence
	snapshot="$(find "${evidence_dir}" -maxdepth 1 -type d -name 'snapshot.*')"
	printf preserved > "${snapshot}/sentinel"
	collect_evidence
	[[ "$(find "${evidence_dir}" -maxdepth 1 -type d -name 'snapshot.*' | wc -l)" -eq 2 ]]
	[[ "$(cat "${snapshot}/sentinel")" == preserved ]]
	setup
	[[ "${first_dir}" != "${evidence_dir}" && "${first_pod}" != "${pod_name}" ]]
	[[ -s "${first_dir}/workload.yaml" ]]
	teardown
	grep -q "^delete pod ${pod_name} " "${trace}"
	must_not grep -Eq '^delete .*--all' "${trace}"
}
field_manifest() {
	KATA_K8S_APPARMOR_API=field
	setup
	grep -q 'type: Localhost' "${yaml_file}"
	grep -q 'localhostProfile: unit-test' "${yaml_file}"
	must_not grep -q 'container.apparmor.security.beta' "${yaml_file}"
	write_pod_manifest runtime/default "${evidence_dir}/runtime-default.yaml"
	grep -q 'type: RuntimeDefault' "${evidence_dir}/runtime-default.yaml"
}
probe_syntax() {
	apparmor_probe_script | /bin/sh -n
	apparmor_probe_script | grep -Fq 'permission denied|operation not permitted'
}
exact_errors() {
	missing_profile=unit-test-missing
	is_missing_profile_error 'Cannot enforce AppArmor: profile "unit-test-missing" is not loaded'
	is_missing_profile_error 'apparmor profile not found "unit-test-missing"'
	is_missing_profile_error 'failed to generate apparmor spec opts: apparmor profile not found unit-test-missing'
	missing_profile=unit.test-missing
	is_missing_profile_error 'apparmor profile not found unit.test-missing'
	must_not is_missing_profile_error 'apparmor profile not found unitXtest-missing'
	must_not is_missing_profile_error 'apparmor profile not found unit.test-missing-other'
	missing_profile=unit-test-missing
	must_not is_missing_profile_error 'No such file or directory (os error 2)'
	must_not is_missing_profile_error 'AppArmor profile unit-test-missing requested but no apparmor guest extension is configured'
	must_not is_missing_profile_error 'apparmor_parser is missing from the guest base image'
}

check 'positive test requires Ready and both containers exec successfully' pass positive
check 'startup complain mode fails' fail:logs bad_positive logs app complain
check 'startup unconfined mode fails' fail:logs bad_positive logs app unconfined
check 'profile substrings do not pass as the expected profile' fail:logs bad_positive logs app wrong-profile
check 'an absent allow marker fails' fail:logs bad_positive logs app missing-allow
check 'an absent deny marker fails' fail:logs bad_positive logs app missing-deny
check 'exec complain mode fails' fail:exec bad_positive exec app complain
check 'exec errors fail' fail:exec bad_positive exec app command-failed
check 'the sidecar must also preserve exec confinement' fail:exec bad_positive exec sidecar unconfined
check 'enforcement must remain enabled after file checks' fail:exec bad_positive exec app mode-changed
check 'a concrete missing profile error passes the negative test' pass negative missing
check 'events are matched by this pod UID' pass events_only
check 'an expected admission error passes the negative test' pass negative create-missing
check 'an unrelated admission error fails' fail:create negative create-unrelated
check 'unrelated NotReady and scheduling/image errors fail' 'fail:get events' negative unrelated
check 'an error naming another profile fails' 'fail:get events' negative wrong-profile
check 'API failures cannot pass as a missing profile' 'fail:get pod' negative api-failure
check 'a pod that becomes Ready cannot pass the negative test' 'fail:get pod' negative became-ready
check 'evidence snapshots and repeated setups never overwrite each other' pass unique_evidence
check 'structured API manifests include Localhost and RuntimeDefault' pass field_manifest
check 'the generated in-container probe has valid POSIX shell syntax' pass probe_syntax
check 'only specific profile errors are accepted' pass exact_errors
printf '%s checks; %s failures\n' "${case_number}" "${failed}"
[[ "${failed}" -eq 0 ]]
