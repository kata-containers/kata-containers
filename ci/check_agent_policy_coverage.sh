#!/usr/bin/env bash
#
# Copyright (c) 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# check_agent_policy_coverage.sh - Static check that every RPC method in
# AgentService has a policy gate in rpc.rs and a default rule in rules.rego.
#
# Usage: ./ci/check_agent_policy_coverage.sh

set -o errexit
set -o nounset
set -o pipefail

[[ -n "${DEBUG:-}" ]] && set -o xtrace

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

PROTO="${repo_root}/src/libs/protocols/protos/agent.proto"
RPC_RS="${repo_root}/src/agent/src/rpc.rs"
RULES_REGO="${repo_root}/src/tools/genpolicy/rules.rego"

# ---------------------------------------------------------------------------
# 1. Extract unique request types from AgentService in the proto file
# ---------------------------------------------------------------------------
request_types=()

while IFS= read -r req_type; do
    [[ -n "${req_type}" ]] && request_types+=("${req_type}")
done < <(
    awk '
    /^service AgentService/,/^}/ {
        if ($0 ~ /^[[:space:]]*rpc[[:space:]]+/) {
            match($0, /\([A-Za-z0-9_]+\)/)
            if (RSTART > 0) {
                print substr($0, RSTART + 1, RLENGTH - 2)
            }
        }
    }
    ' "${PROTO}" | sort -u
)

if [[ ${#request_types[@]} -eq 0 ]]; then
    echo "ERROR: failed to extract any request types from ${PROTO}" >&2
    exit 1
fi

echo "Checking ${#request_types[@]} unique request types from AgentService..."

# ---------------------------------------------------------------------------
# 2. Pre-process rpc.rs into an in-memory string cache
#    This isolates the impl block and flattens newlines so multi-line
#    method headers match perfectly on both Mac and Linux.
# ---------------------------------------------------------------------------
impl_start=$(grep -m 1 -n 'impl agent_ttrpc::AgentService for AgentService' "${RPC_RS}" | cut -d: -f1 || true)
if [[ -z "${impl_start}" ]]; then
    echo "ERROR: could not find impl block in ${RPC_RS}" >&2
    exit 1
fi

# Read from impl_start onwards, flatten all whitespace into spaces for easy matching
rpc_cache=$(tail -n +"${impl_start}" "${RPC_RS}" | tr '\n' ' ' | tr -s ' ')

# ---------------------------------------------------------------------------
# 3. Check each request type for policy gates and rules
# ---------------------------------------------------------------------------
missing_rpc=()
missing_rego=()

for req_type in "${request_types[@]}"; do
    # --- Check rpc.rs ---
    # Match the handler function that takes this request type as its parameter,
    # extracting from "async fn" up to the closing "}" of that method body.
    method_block=$(grep -oE "async fn [A-Za-z0-9_]+[^}]+(req|config)[[:space:]]*:[[:space:]]*(protocols::agent::)?${req_type}[^}]+}" <<< "${rpc_cache}" | head -1 || true)

    if [[ -z "${method_block}" ]]; then
        missing_rpc+=("${req_type}  [no handler found in rpc.rs impl block]")
        continue
    fi

    # Ensure the method body actually invokes the required security assertions
    if ! grep -qE 'is_allowed|do_set_policy' <<< "${method_block}"; then
        missing_rpc+=("${req_type}  [handler found, but has no is_allowed/do_set_policy call]")
    fi

    # --- Check rules.rego ---
    if ! grep -q "default ${req_type} " "${RULES_REGO}"; then
        missing_rego+=("${req_type}")
    fi
done

# ---------------------------------------------------------------------------
# 4. Report results
# ---------------------------------------------------------------------------
failed=0

if (( ${#missing_rpc[@]} > 0 )); then
    echo -e "\nFAIL: The following request types are missing a policy gate in ${RPC_RS}:"
    for entry in "${missing_rpc[@]}"; do
        echo "  - ${entry}"
    done
    failed=1
fi

if (( ${#missing_rego[@]} > 0 )); then
    echo -e "\nFAIL: The following request types are missing a 'default <TypeName>' rule in ${RULES_REGO}:"
    for entry in "${missing_rego[@]}"; do
        echo "  - ${entry}"
    done
    echo -e "\n  Add entries like:"
    for entry in "${missing_rego[@]}"; do
        echo "    default ${entry} := false"
    done
    failed=1
fi

if (( failed == 0 )); then
    echo "OK: all ${#request_types[@]} AgentService request types have policy gates and rules.rego entries."
fi

exit "${failed}"
