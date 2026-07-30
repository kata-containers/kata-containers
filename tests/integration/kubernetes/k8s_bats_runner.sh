#!/usr/bin/env bash
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Kubernetes bats runner: plain RuntimeClass by default, with an optional
# triage-only re-run on kata-<shim>-debug for each failed test case.
#

# The suites that cannot produce a verdict on the plain RuntimeClass, because
# they assert on guest output that only reaches the host journal when the
# hypervisor runs with enable_debug. Each entry is "<runtime>:<bats-file>",
# where runtime is "go", "runtime-rs", or "both".
#
# Keep this list short: needing it means the failure the test looks for is
# invisible to a user running without debug enabled.
K8S_TESTS_REQUIRING_GUEST_DEBUG=(
	# Both runtimes only forward the dm-verity guest-console message in debug.
	"both:k8s-measured-rootfs.bats"

	# runtime-rs only forwards these image-rs agent errors over agent.log_vport
	# when the hypervisor has debug enabled. The Go runtime exposes them without
	# needing the -debug RuntimeClass.
	"runtime-rs:k8s-guest-pull-image.bats"
	"runtime-rs:k8s-guest-pull-image-authenticated.bats"
	"runtime-rs:k8s-guest-pull-image-encrypted.bats"
	"runtime-rs:k8s-guest-pull-image-signature.bats"
)

# Whether a bats file requires guest debug for the current runtime.
bats_file_requires_guest_debug() {
	local bats_file="$1"
	local entry
	local runtime
	local listed_file

	for entry in "${K8S_TESTS_REQUIRING_GUEST_DEBUG[@]}"; do
		runtime="${entry%%:*}"
		listed_file="${entry#*:}"
		[[ "${listed_file}" == "${bats_file}" ]] || continue

		case "${runtime}" in
			both) return 0 ;;
			go)
				is_runtime_rs && return 1
				return 0
				;;
			runtime-rs)
				is_runtime_rs && return 0
				return 1
				;;
			*) die "Invalid runtime '${runtime}' in K8S_TESTS_REQUIRING_GUEST_DEBUG" ;;
		esac
	done

	return 1
}

# Escape a string for use as an extended-regex (bats --filter).
escape_bats_filter_regex() {
	# The bracket expression leads with ']' so that it is taken literally.
	printf '%s' "$1" | sed -E 's/[]\\.[^$*+?(){}|]/\\&/g'
}

# Extract failed bats test names from TAP output.
failed_bats_tests_from_tap() {
	local tap_file="$1"

	# TAP from bats --timing may use either:
	#   not ok N - test name in Xms
	#   not ok N test name in Xms
	awk '
		/^not ok / {
			sub(/^not ok [0-9]+ - /, "")
			sub(/^not ok [0-9]+ /, "")
			sub(/ in [0-9]+(\.[0-9]+)?(ms|s)$/, "")
			print
		}
	' "${tap_file}"
}

# Whether the debug RuntimeClass for the current shim is deployed.
debug_runtime_class_available() {
	# KATA_HYPERVISOR / MULTI_INSTALL_SUFFIX come from the k8s test environment.
	# shellcheck disable=SC2154
	local base="kata-${KATA_HYPERVISOR}${MULTI_INSTALL_SUFFIX:+-${MULTI_INSTALL_SUFFIX}}"

	kubectl get runtimeclass "${base}-debug" &>/dev/null
}

# Run one bats file under KATA_TEST_RUNTIME_CLASS_MODE, writing TAP to out_file.
# Returns the bats exit status.
run_bats_file_with_mode() {
	local test_entry="$1"
	local mode="$2"
	local out_file="$3"
	shift 3
	local -a extra_args=("$@")

	apply_test_runtime_class_mode "${mode}"

	# Prefer TAP so failed case names are machine-parseable; keep timing and
	# passing-test output for humans reading the CI log.
	bats --tap --timing --show-output-of-passing-tests \
		"${extra_args[@]}" "${test_entry}" | tee "${out_file}"
	return "${PIPESTATUS[0]}"
}

# Triage-only re-run of failed cases on the debug RuntimeClass.
#
# Never changes the caller's idea of pass/fail: triage exit codes are ignored.
triage_failed_tests_with_debug() {
	local test_entry="$1"
	local tap_file="$2"
	local report_dir="$3"
	local -a failed=()
	local name
	local safe_name
	local triage_out
	local filter

	mapfile -t failed < <(failed_bats_tests_from_tap "${tap_file}")
	[[ ${#failed[@]} -eq 0 ]] && return 0

	if ! debug_runtime_class_available; then
		info "Skipping debug triage retry: ${KATA_HYPERVISOR} debug RuntimeClass not deployed"
		return 0
	fi

	for name in "${failed[@]}"; do
		[[ -z "${name}" ]] && continue
		info "RETRY (triage only) on kata-${KATA_HYPERVISOR}-debug: ${name}; verdict remains FAIL"
		safe_name=$(echo "${name}" | tr -c 'A-Za-z0-9._-' '_')
		triage_out="${report_dir}/triage-debug-${test_entry}-${safe_name}.out"
		filter="^$(escape_bats_filter_regex "${name}")$"

		# Ignore triage status: the plain (or hard-require) run is the verdict.
		run_bats_file_with_mode "${test_entry}" "debug" "${triage_out}" \
			-f "${filter}" || true
	done

	# Restore plain fixtures for subsequent files in the union.
	apply_test_runtime_class_mode "plain"
}

# Run the k8s bats union with non-debug-first semantics.
#
# Parameters:
#	$1 - Test directory
#	$2 - Name of the array variable holding bats filenames
#
# Environment:
#	BATS_TEST_FAIL_FAST - "yes" to stop after the first failed file
#	K8S_TEST_DEBUG_RETRY - "true" (default) to re-run failed cases on -debug
run_kubernetes_bats_tests() {
	local test_dir="$1"
	local -n test_array=$2
	local fail_fast="${BATS_TEST_FAIL_FAST:-no}"
	local debug_retry="${K8S_TEST_DEBUG_RETRY:-true}"
	local required
	local required_runtime
	local required_file

	# An invalid scope or stale filename would silently downgrade a suite to
	# the plain class.
	for required in "${K8S_TESTS_REQUIRING_GUEST_DEBUG[@]}"; do
		required_runtime="${required%%:*}"
		required_file="${required#*:}"
		case "${required_runtime}" in
			go | runtime-rs | both) ;;
			*) die "Invalid runtime '${required_runtime}' in K8S_TESTS_REQUIRING_GUEST_DEBUG" ;;
		esac
		[[ -f "${test_dir}/${required_file}" ]] ||
			die "K8S_TESTS_REQUIRING_GUEST_DEBUG lists ${required_file}, which does not exist"
	done

	local report_dir
	report_dir="${test_dir}/reports/$(date +'%F-%T')"
	mkdir -p "${report_dir}"

	export K8S_TEST_DIR="${test_dir}"

	info "Running k8s tests with bats version: $(bats --version). Save outputs to ${report_dir}"
	info "RuntimeClass mode default=plain; debug triage retry=${debug_retry}"

	local tests_fail=()
	local test_entry
	local out_file
	local verdict=0
	local mode

	for test_entry in "${test_array[@]}"; do
		test_entry=$(echo "${test_entry}" | tr -d '[:space:][:cntrl:]')
		[[ -z "${test_entry}" ]] && continue

		info "Executing ${test_entry}"
		out_file="${report_dir}/${test_entry}.out"

		if [[ -n "${K8S_BATS_BEFORE_FILE:-}" ]] && declare -F "${K8S_BATS_BEFORE_FILE}" >/dev/null; then
			"${K8S_BATS_BEFORE_FILE}" || true
		fi

		pushd "${test_dir}" > /dev/null || return 1

		if bats_file_requires_guest_debug "${test_entry}"; then
			if ! debug_runtime_class_available; then
				popd > /dev/null || true
				die "${test_entry} requires kata-${KATA_HYPERVISOR}-debug RuntimeClass, but it is not deployed"
			fi
			mode="debug"
			info "${test_entry} requires guest debug; running on -debug only"
		else
			mode="plain"
		fi

		verdict=0
		run_bats_file_with_mode "${test_entry}" "${mode}" "${out_file}" || verdict=$?

		if [[ ${verdict} -ne 0 ]]; then
			tests_fail+=("${test_entry}")
			mv "${out_file}" "$(dirname "${out_file}")/not_ok-$(basename "${out_file}")"
			local not_ok_file
			not_ok_file="$(dirname "${out_file}")/not_ok-$(basename "${out_file}")"

			# Triage retry only for plain verdict failures (hard-require already ran debug).
			if [[ "${debug_retry}" == "true" ]] && [[ "${mode}" == "plain" ]]; then
				triage_failed_tests_with_debug "${test_entry}" "${not_ok_file}" "${report_dir}"
			fi

			popd > /dev/null || return 1
			[[ "${fail_fast}" == "yes" ]] && break
		else
			mv "${out_file}" "$(dirname "${out_file}")/ok-$(basename "${out_file}")"
			popd > /dev/null || return 1
		fi
	done

	# Leave workloads on plain for anything that inspects them after the suite.
	apply_test_runtime_class_mode "plain" || true

	if [[ ${#tests_fail[@]} -ne 0 ]]; then
		die "Tests FAILED from suites: ${tests_fail[*]}"
	fi

	info "All tests SUCCEEDED"
}
