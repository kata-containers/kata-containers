#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "${work}"' EXIT
mkdir -p "${work}/bin" "${work}/checkout/hack"
cp "${script_dir}/e2e-ci.sh" "${work}/e2e-ci.sh"
cat > "${work}/bin/go" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
cat > "${work}/verify-artifacts.sh" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "$2" >> "${TEST_E2E_CALLS}"
case "$2" in
	before) exit "${TEST_BEFORE_STATUS}" ;;
	after) exit "${TEST_AFTER_STATUS}" ;;
esac
MOCK
cat > "${work}/checkout/hack/run-e2e-kind.sh" <<'MOCK'
#!/usr/bin/env bash
printf 'suite\n' >> "${TEST_E2E_CALLS}"
exit "${TEST_SUITE_STATUS}"
MOCK
cat > "${work}/driver.sh" <<'DRIVER'
#!/usr/bin/env bash
set -euo pipefail
# Load the functions through the report entry point with a mocked Go command.
source "$1/e2e-ci.sh" report "$1/checkout"
run_e2e
DRIVER
chmod +x "${work}/bin/go"
export PATH="${work}/bin:${PATH}" TEST_E2E_CALLS="${work}/calls" ARTIFACTS="${work}/results"
export TEST_BEFORE_STATUS TEST_SUITE_STATUS TEST_AFTER_STATUS
while read -r TEST_BEFORE_STATUS TEST_SUITE_STATUS TEST_AFTER_STATUS expected_status expected_calls; do
	: > "${TEST_E2E_CALLS}"
	status=0
	bash "${work}/driver.sh" "${work}" || status=$?
	[[ ${status} == "${expected_status}" ]] || { echo "Expected status ${expected_status}, got ${status}" >&2; exit 1; }
	[[ $(paste -sd, "${TEST_E2E_CALLS}") == "${expected_calls}" ]] || { cat "${TEST_E2E_CALLS}" >&2; exit 1; }
done <<'CASES'
0 0 0 0 before,suite,after
0 7 0 7 before,suite,after
0 0 9 9 before,suite,after
0 7 9 7 before,suite,after
5 0 0 5 before
CASES
echo 'E2e failures preserve suite status and still verify the candidate guest'
