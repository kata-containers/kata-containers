# Copyright (c) 2022-2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Error handling functions.

# Write specified message to stderr.
stderr()
{
	local msg="$*"

	echo >&2 "$msg"
}

# Simplified version of die that should be called by functions that
# could fail as part of the normal "die()" code path. Required to
# avoid infinite recursion.
_fatal()
{
	local msg="$*"

	echo >&2 "FATAL: $msg"
	exit 1
}

# Canonicalize the specified path (which must be valid).
resolve_path()
{
	local file="${1:-}"
	[ -z "$file" ] && _fatal "need file to resolve"

	local path

	path=$(readlink --canonicalize-existing "$file" || \
		_fatal "failed to resolve file '$file'")

	echo "$path"
}

show_proc_hierarchy()
{
	local pid="${$}"

	local details

	stderr "process-hierarchy:"

	local -i i

	for ((i=0; ; i++))
	do
		local details
		local msg

		local current=''

		[ "${pid}" = "${$}" ] && current=", current='yes'"

		details=$(ps --no-headers -p "$pid" -o ppid,cmd)

		# The parent PID is always the first column due to the
		# format specifier used above.
		local ppid=$(echo "$details" | awk '{print $1}')

		# But the command part has a variable number of fields
		# (since it could contain any number of spaces).
		# Hence, delete the first field (PPID) and what
		# remains is the entire command line.
		local cmd=$(echo "$details" |\
			awk '{$1=""; print $0}' |\
			sed \
			-e 's/^ *//g' \
			-e 's/ *$//g')

		msg=$(printf "  %d: {pid: %d, command: '%s'%s}" \
			"${i}" \
			"${pid}" \
			"${cmd}" \
			"${current}")
		stderr "$msg"

		[ "$pid" = 1 ] && break

		pid="$ppid"
	done
}

show_stacktrace()
{
	local err_line="${1:-}"
	local err_func="${2:-}"
	local err_path="${3:-}"

	[ -z "$err_line" ] && _fatal "need error location line number"
	[ -z "$err_func" ] && _fatal "need error location func"
	[ -z "$err_path" ] && _fatal "need error location file path"

	local line
	local func
	local file

	local -i i

	stderr "stacktrace:"

	for ((i = 0; ; i++))
	do
		local result
		result=$(caller "$i" || true)

		[ -z "$result" ] && break

		line=$(echo "$result"|awk '{print $1}')
		func=$(echo "$result"|awk '{print $2}')
		file=$(echo "$result"|awk '{print $3}')

		local path
		path=$(resolve_path "$file")

		local msg

		local current=''

		# Add a visual marker showing where the original error was
		# detected.
		[ "${line}" = "${err_line}" ] && \
		[ "${func}" = "${err_func}" ] && \
		[ "${path}" = "${err_path}" ] && \
		current=", current='yes'"

		msg=$(printf "  %d: {function: '%s', file: '%s', line: %d%s}\n" \
			"${i}" \
			"${func}" \
			"${path}" \
			"${line}" \
			"${current}" )

		stderr "$msg"
	done
}

# Function to be called by die() or a trap/signal handler to dump all
# details of the environment (in YAML format), to help with debugging.
dump_details()
{
	set +x

	local err_line="${1:-}"
	local err_func="${2:-}"
	local err_path="${3:-}"

	[ -z "$err_line" ] && _fatal "need error location line number"
	[ -z "$err_func" ] && _fatal "need error location func"
	[ -z "$err_path" ] && _fatal "need error location file path"

	# Spacer
	stderr

	stderr "script:"
	stderr "  name: '$0'"
	stderr "  pid: $$"
	stderr "  directory: '$PWD'"
	stderr "  details: '$(ls -dlZ "$PWD")'"
	stderr "failure:"
	stderr "  function: '$func'"
	stderr "  file: '$path'"
	stderr "  line: $line"
	stderr "  name: '$0'"

	show_stacktrace \
		"${err_line}" \
		"${err_func}" \
		"${err_path}"

	show_proc_hierarchy

	stderr "time: '$(date -Isec)'"
	stderr "runtime-seconds: ${SECONDS}"
	stderr "host:"
	stderr "  name: '$(hostname)'"
	stderr "  uname: '$(uname -a)'"

	stderr "locale:"
	locale 2>/dev/null | sed \
		-e 's/^/  /g' \
		-e 's/=/: '\''/g' \
		-e 's/$/'\''/g' \
		>&2

	stderr "user:"
	stderr "  uid: {value: $UID, name: '$(getent passwd "$UID"|cut -d: -f1)'}"
	stderr "  euid: {value: $EUID , name: '$(getent passwd "$EUID"|cut -d: -f1)'}"
	stderr "  groups: '$(id)'"

	stderr "bash:"
	stderr "  version: '${BASH_VERSION}'"
	stderr "  version-info: '${BASH_VERSINFO[*]}'"

	stderr "environment:"

	# Remove bash functions (that can span multiple lines)
	env |\
		grep -v "^BASH_FUNC" |\
		grep -Ei "^[a-z_][a-z0-9_]+=" |\
		sort -t '=' -k1 |\
		sed \
		-e 's/^/  /g' \
		-e 's/=/: '\''/' \
		-e 's/$/'\''/g' \
		>&2

	stderr "mounts: |"
	mount | sed 's/^/  /g' >&2

	stderr "processes: |"
	ps -eF | sed 's/^/  /g' >&2

	# Spacer
	stderr
}
