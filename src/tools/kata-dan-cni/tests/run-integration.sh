#!/usr/bin/env bash
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

fail() {
    echo "kata-dan-cni integration: $*" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Usage: bash tests/run-integration.sh

Builds the ignored library tests as the invoking user, then runs them with real
TAP devices and OVS in private network, mount, and PID namespaces.

Environment:
  CARGO                       Cargo executable (default: cargo)
  TRIPLE                      Optional Cargo target triple
  BUILD_TYPE                  release or debug (default: release)
  OVS_SCHEMA                  OVS schema (default: /usr/share/openvswitch/vswitch.ovsschema)
  KATA_DAN_TEST_LOG_DIR        Log parent directory (default: target/kata-dan-integration-logs)

Requires Linux, /dev/net/tun, the OVS kernel datapath, jq, iproute2, util-linux,
OVS tools, and root privileges. For a non-root caller, run sudo -v first.
EOF
}

stop_daemon() {
    local daemon_pid=$1
    [[ -n "${daemon_pid}" ]] || return 0
    kill -TERM "${daemon_pid}" 2>/dev/null || true
    for ((attempt = 0; attempt < 50; attempt++)); do
        kill -0 "${daemon_pid}" 2>/dev/null || break
        sleep 0.1
    done
    kill -KILL "${daemon_pid}" 2>/dev/null || true
    wait "${daemon_pid}" 2>/dev/null || true
}

run_inside_namespace() {
    local test_binary=$1 log_dir=$2 schema=$3
    local original_mount_ns=$4 original_net_ns=$5
    local ovsdb_pid='' vswitchd_pid=''
    local socket=/run/openvswitch/db.sock
    local vswitchd_ctl=/run/openvswitch/ovs-vswitchd.ctl
    local state_dir=/run/kata-dan-integration

    [[ ${EUID} -eq 0 && ${BASHPID} -eq 1 ]] || fail "inner runner requires an isolated root PID namespace"
    [[ $(readlink /proc/self/ns/mnt) != "${original_mount_ns}" ]] || fail "mount namespace was not isolated"
    [[ $(readlink /proc/self/ns/net) != "${original_net_ns}" ]] || fail "network namespace was not isolated"

    # Hide every host OVS socket, pidfile, and control socket before starting OVS.
    mount --make-rprivate /
    mount -t tmpfs -o mode=0755 tmpfs /run
    mkdir -p /run/openvswitch "${state_dir}/tmp"
    export TMPDIR="${state_dir}/tmp"
    export KATA_DAN_INTEGRATION=1
    export KATA_DAN_TEST_LOG_DIR="${log_dir}"

    cleanup() {
        local status=$?
        trap - EXIT INT TERM
        set +e
        if ((status != 0)); then
            ip -details -json link show >"${log_dir}/links.json" 2>&1
            for table in Bridge Port Interface QoS Queue; do
                ovs-vsctl --db="unix:${socket}" --timeout=2 --format=json list "${table}" \
                    >"${log_dir}/ovs-${table}.json" 2>&1
            done
            ovs-appctl --timeout=2 -t "${vswitchd_ctl}" dpif/show \
                >"${log_dir}/datapath.log" 2>&1
            cp -a "${TMPDIR}" "${log_dir}/temporary-files"
            echo "Integration failed; logs and resources: ${log_dir}" >&2
            if [[ -f "${log_dir}/ovs-vswitchd.log" ]]; then
                tail -n 40 "${log_dir}/ovs-vswitchd.log" >&2
            fi
        fi
        stop_daemon "${vswitchd_pid}"
        stop_daemon "${ovsdb_pid}"
        # The caller created log_dir; make root-owned logs and private DAN
        # directories readable by that same caller for artifact collection.
        if ! chown -hR --reference="${log_dir}" -- "${log_dir}"; then
            echo "Could not restore integration log ownership: ${log_dir}" >&2
            ((status != 0)) || status=1
        fi
        exit "${status}"
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    ip link set lo up
    ip tuntap add dev kdt-ci-probe mode tap || fail "cannot create TAP devices; check CAP_NET_ADMIN and /dev/net/tun"
    ip tuntap del dev kdt-ci-probe mode tap

    ovsdb-tool create "${state_dir}/conf.db" "${schema}"
    ovsdb-server "${state_dir}/conf.db" \
        --remote="punix:${socket}" \
        --pidfile=/run/openvswitch/ovsdb-server.pid \
        --unixctl=/run/openvswitch/ovsdb-server.ctl \
        --no-chdir --log-file="${log_dir}/ovsdb-server.log" \
        >"${log_dir}/ovsdb-server.stderr.log" 2>&1 &
    ovsdb_pid=$!

    local ready=0
    for ((attempt = 0; attempt < 100; attempt++)); do
        kill -0 "${ovsdb_pid}" 2>/dev/null || fail "private ovsdb-server exited; see ${log_dir}"
        if [[ -S "${socket}" ]] && ovs-vsctl --db="unix:${socket}" --timeout=1 --no-wait init; then
            ready=1
            break
        fi
        sleep 0.1
    done
    ((ready == 1)) || fail "private OVSDB did not become ready"

    ovs-vswitchd "unix:${socket}" \
        --pidfile=/run/openvswitch/ovs-vswitchd.pid \
        --unixctl="${vswitchd_ctl}" \
        --no-chdir --log-file="${log_dir}/ovs-vswitchd.log" \
        >"${log_dir}/ovs-vswitchd.stderr.log" 2>&1 &
    vswitchd_pid=$!

    ovs-vsctl --db="unix:${socket}" --timeout=15 add-br br-int \
        -- set Bridge br-int datapath_type=system fail_mode=secure
    local ofport
    ready=0
    for ((attempt = 0; attempt < 100; attempt++)); do
        kill -0 "${vswitchd_pid}" 2>/dev/null || fail "private ovs-vswitchd exited; see ${log_dir}"
        ofport=$(ovs-vsctl --db="unix:${socket}" --timeout=1 get Interface br-int ofport)
        if [[ "${ofport}" =~ ^[0-9]+$ ]] && ((ofport > 0)) && ip link show dev br-int >/dev/null 2>&1; then
            ready=1
            break
        fi
        sleep 0.1
    done
    ((ready == 1)) || fail "system OVS datapath is unavailable; check the openvswitch kernel module"

    echo "Running real TAP/OVS tests in private namespaces; logs: ${log_dir}"
    local test_status=0
    "${test_binary}" 'dispatch::integration_tests::' --ignored --test-threads=1 --nocapture \
        2>&1 | tee "${log_dir}/tests.log" || test_status=$?
    # Exit while this function's daemon handles are still in scope for the trap.
    exit "${test_status}"
}

if [[ ${1:-} == --inside ]]; then
    [[ $# -eq 6 ]] || fail "invalid internal runner arguments"
    run_inside_namespace "$2" "$3" "$4" "$5" "$6"
    exit 0
fi
if [[ ${1:-} == --help || ${1:-} == -h ]]; then
    usage
    exit 0
fi
[[ $# -eq 0 ]] || fail "unexpected arguments; use --help"
[[ $(uname -s) == Linux ]] || fail "Linux is required"

script_path=$(readlink -f -- "${BASH_SOURCE[0]}")
repo_root=$(cd -- "$(dirname -- "${script_path}")/../../../.." && pwd)
cd "${repo_root}"

for tool in "${CARGO:-cargo}" jq unshare mount ip ovsdb-tool ovsdb-server ovs-vswitchd ovs-vsctl ovs-appctl; do
    command -v "${tool}" >/dev/null 2>&1 || fail "required executable not found: ${tool}"
done
[[ -c /dev/net/tun ]] || fail "/dev/net/tun is required"
schema=${OVS_SCHEMA:-/usr/share/openvswitch/vswitch.ovsschema}
[[ -r "${schema}" ]] || fail "OVS schema not readable: ${schema} (set OVS_SCHEMA if needed)"
schema=$(readlink -f -- "${schema}")

log_parent=${KATA_DAN_TEST_LOG_DIR:-${repo_root}/target/kata-dan-integration-logs}
mkdir -p -- "${log_parent}"
log_parent=$(cd -- "${log_parent}" && pwd -P)
case "${log_parent}/" in
    /run/* | /var/run/*) fail "log directory must remain accessible when /run is isolated" ;;
esac
log_dir=$(mktemp -d -- "${log_parent}/run.XXXXXX")
echo "Integration logs: ${log_dir}"

build=("${CARGO:-cargo}" test -p kata-dan-cni --lib --no-run --locked --message-format=json)
[[ -z ${TRIPLE:-} ]] || build+=(--target "${TRIPLE}")
case ${BUILD_TYPE:-release} in
    release) build+=(--release) ;;
    debug) ;;
    *) fail "BUILD_TYPE must be release or debug" ;;
esac
echo "Building integration tests as $(id -un)"
if "${build[@]}" >"${log_dir}/cargo-build.json" 2> >(tee "${log_dir}/cargo-build.log" >&2); then
    :
else
    status=$?
    echo "Test build failed; logs: ${log_dir}" >&2
    exit "${status}"
fi
test_binary=$(jq -rs '
    [.[] | select(.reason == "compiler-artifact" and .profile.test and .executable != null)]
    | if length == 1 then .[0].executable else error("expected one library test executable") end
' "${log_dir}/cargo-build.json")
[[ -x "${test_binary}" ]] || fail "Cargo test executable is unavailable: ${test_binary}"

# A misspelled filter must not turn this job into a successful zero-test run.
"${test_binary}" 'dispatch::integration_tests::' --ignored --list >"${log_dir}/test-list.log"
test_count=0
while IFS= read -r line; do
    if [[ "${line}" == dispatch::integration_tests::*': test' ]]; then
        test_count=$((test_count + 1))
    fi
done <"${log_dir}/test-list.log"
((test_count > 0)) || fail "no ignored dispatch integration tests found in ${test_binary}"

root_command=()
if ((EUID != 0)); then
    command -v sudo >/dev/null 2>&1 || fail "sudo is required for namespace setup"
    sudo -n true || fail "root privileges are required; run sudo -v and retry"
    root_command=(sudo -n -E)
fi
mount_ns=$(readlink /proc/self/ns/mnt)
net_ns=$(readlink /proc/self/ns/net)
if "${root_command[@]}" env PATH="${PATH}" \
    unshare --net --mount --pid --fork --mount-proc --kill-child --propagation private \
    bash "${script_path}" --inside "${test_binary}" "${log_dir}" "${schema}" "${mount_ns}" "${net_ns}" \
    2>&1 | tee "${log_dir}/runner.log"; then
    echo "Integration tests passed; logs: ${log_dir}"
else
    status=$?
    echo "Integration target failed (exit ${status}); see logs: ${log_dir}" >&2
    exit "${status}"
fi
