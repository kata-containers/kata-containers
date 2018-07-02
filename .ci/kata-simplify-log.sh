#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

readonly script_name=${0##*/}

die()
{
    local -r msg="$*"
    echo >&2 "ERROR: $msg"
    exit 1
}

usage()
{
    cat <<EOT
Description: Simplify the specified logfile by replacing common fields with
fixed strings to make diff(1)-ing easier.

Usage: $script_name <log-file>
       $script_name [-h|--help|help]

Options:

  -h     : Show this help.
  --help :
  help   :

Limitations:

- This script uses simple heuristics and might break at any time.

EOT
}

# Use heuristics to convert patterns in the specified structured logfile into
# fixed strings to aid in comparision with other logs from the same system
# component.
simplify_log()
{
    local -r file="$1"

    # Pattern for a standard timestamp.
    #
    # Format: "YYYY-MM-DDTHH:MM:SS.NNNNNNNNNxZZ:ZZ" where "x" is "+" or "-"
    typeset -r timestamp_pattern="[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}.[0-9]{,9}[+-][0-9]{2}:[0-9]{2}"

    # Slightly different timestamp format used by the agent.
    #
    # Format: "YYYY-MM-DDTHH:MM:SS.NNNNNNNNNZ"
    typeset -r timestamp_pattern_agent="[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}.[0-9]{,9}Z"

    # Pattern used to detect the agent displaying total guest memory.
    #
    # Format: "\"DDDDDD kB"
    typeset -r memory_size_pattern="\"[0-9][0-9]* kB"

    # Pattern used to detect architectures (uses golang architecture names).
    typeset -r arch_pattern="(amd64|arm64|ppc64le)"

    # Pattern used to detect the prefix used when mounting resources into the
    # container.
    typeset -r mount_hash_pattern="[[:xdigit:]]{64}-[[:xdigit:]]{16}-"

    # Pattern for 64-byte hash values.
    typeset -r hash_pattern="[[:xdigit:]]{64}"

    # Pattern for detecting duration messages from the guest kernel modules.
    typeset -r duration_pattern="duration=[^ ][^ ]* "

    # Pattern for detecting memory addresses.
    typeset -r address_pattern="0x[[:xdigit:]]{,10}"

    # Pattern for detecting UUIDs (see uuidgen(1)).
    typeset -r uuid_pattern="[[:xdigit:]]{8}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{12}"

    # Pattern for detecting network MAC addresses.
    typeset -r mac_addr_pattern="[[:xdigit:]]{2}:[[:xdigit:]]{2}:[[:xdigit:]]{2}:[[:xdigit:]]{2}:[[:xdigit:]]{2}:[[:xdigit:]]{2}"

    # Pattern for detecting git(1) commits.
    typeset -r commit_pattern="[[:xdigit:]]{40}"

    # Pattern for detecting IPv4 address.
    #
    # Format: "XXX.XXX.XXX.XXX"
    typeset -r ip_addr_pattern="[0-9]{1,3}.[0-9]{1,3}.[0-9]{1,3}.[0-9]{1,3}"

    # Pattern for detecting IPv4 address with a netmask.
    #
    # Format: "XXX.XXX.XXX.XXX/XXX"
    typeset -r ip_addr_with_netmask_pattern="[0-9]{1,3}.[0-9]{1,3}.[0-9]{1,3}.[0-9]{1,3}/[0-9]{1,3}"

    # Pattern for detecting process IDs in the structured logs.
    typeset -r pid_pattern="pid=[0-9][0-9]*"

    # Pattern for detecting files in the proc(5) filesystem.
    typeset -r proc_fs_pattern="/proc/[0-9][0-9]*/"

    # Pattern used to detect kernel diagnostic messages that show how long it
    # took to load a kernel module.
    typeset -r kernel_modprobe_pattern="returned -*[0-9][0-9]* after [0-9][0-9]* usecs"

    # Pattern to detect numbers (currently just integers).
    typeset -r number_pattern="[0-9][0-9]*"

    # Notes:
    #
    # - Some of the patterns below use "!" as the delimiter as the patterns
    #   contain forward-slashes.
    #
    # - The patterns need to be in most-specific-to-least-specific order to
    #   ensure correct behaviour.
    #
    # - Patterns that anchor to a structured logging field need to ensure the
    #   replacement text is also a valid structured log field (for example
    #   duration and pid patterns).
    sed -r \
        -e "s/${timestamp_pattern}/TIMESTAMP/gI" \
        -e "s/${timestamp_pattern_agent}/TIMESTAMP/gI" \
        -e "s/${memory_size_pattern}/MEMORY-SIZE/gI" \
        -e "s/${arch_pattern}/ARCHITECTURE/gI" \
        -e "s/${mount_hash_pattern}/MOUNT-HASH/gI" \
        -e "s/${hash_pattern}/HASH/gI" \
        -e "s/${duration_pattern}/duration=DURATION /gI" \
        -e "s/${address_pattern}/HEX-ADDRESS/gI" \
        -e "s/${uuid_pattern}/UUID/gI" \
        -e "s/${mac_addr_pattern}/MAC-ADDRESS/gI" \
        -e "s/${commit_pattern}/COMMIT/gI" \
        -e "s!${ip_addr_with_netmask_pattern}!IP-ADDRESS-AND-MASK!gI" \
        -e "s/${ip_addr_pattern}/IP-ADDRESS/gI" \
        -e "s/${pid_pattern}/pid=PID/gI" \
        -e "s!${proc_fs_pattern}!/proc/PID/!gI" \
        -e "s/${kernel_modprobe_pattern}/returned VALUE after VALUE usecs/g" \
        -e "s/${number_pattern}/NUMBER/gI" \
        "$file"
}

[ $# -ne 1 ] && usage && die "need argument"

case "$1" in
    -h|--help|help)
        usage
        exit 0
        ;;

    *)
        file="$1"
        ;;
esac

simplify_log "$file"
