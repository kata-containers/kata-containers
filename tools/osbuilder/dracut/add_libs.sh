#!/usr/bin/env bash
#
# Copyright (c) 2021 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

script_name="$(basename "$0")"
usage() {
	cat >&2 << EOF
Usage: ${script_name} [-h] <agent-binary>
Output dracut configuration with required additional libraries for the Kata agent.

Example:
${script_name} \$GOPATH/src/github.com/kata-containers/kata-containers/src/agent/target/x86_64-unknown-linux-gnu/release/kata-agent

Options:
-h: Show this help
EOF
	exit "$1"
}

if [[ $# != 1 || "$1" == "-h" ]]; then
	usage 0
fi

agent_binary="$1"
non_standard_libs=("libutil.so")
install_items=""

if [ ! -x "${agent_binary}" ]; then
	echo >&2 "ERROR: ${agent_binary} is not an executable file"
	usage 1
fi

# Cover both cases of "not a dynamic executable" being printed to STDERR
# and "statically linked" being printed to STDOUT
linked_libs="$(ldd "${agent_binary}" 2>&1)"
if [ "$(wc -l <<< "${linked_libs}")" == 1 ]; then
	echo >&2 "Agent appears to be linked statically, exiting"
	exit 0
fi

for lib in "${non_standard_libs[@]}"; do
	install_items+=" $(grep -F "${lib}" <<< "${linked_libs}" | cut -d" " -f3)"
done

cat << EOF
# add libraries that the Kata agent is linked against, but that are not included by default
install_items+="${install_items} "
EOF
