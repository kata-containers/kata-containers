#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

DEBUG=${DEBUG:-}
[ -n "$DEBUG" ] && set -o xtrace

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

RUNTIME=${RUNTIME:-kata-runtime}

TRACE_LOG_DIR=${TRACE_LOG_DIR:-${KATA_TESTS_LOGDIR}/traces}

jaeger_server=${jaeger_server:-localhost}
jaeger_ui_port=${jaeger_ui_port:-16686}
jaeger_docker_container_name="jaeger"

# Cleanup will remove Jaeger container and
# disable tracing.
cleanup(){
	stop_jaeger 2>/dev/null || true
	"${SCRIPT_PATH}/../.ci/configure_tracing_for_kata.sh" disable
}

trap cleanup EXIT

# Run an operation to generate Jaeger trace spans
create_trace()
{
	sudo docker run -i --runtime "$RUNTIME" --net=none --rm busybox true
}

start_jaeger()
{
	local jaeger_docker_image="jaegertracing/all-in-one:latest"

	# Defaults - see https://www.jaegertracing.io/docs/getting-started/
	docker run -d --runtime runc --name "${jaeger_docker_container_name}" \
		-e COLLECTOR_ZIPKIN_HTTP_PORT=9411 \
		-p 5775:5775/udp \
		-p 6831:6831/udp \
		-p 6832:6832/udp \
		-p 5778:5778 \
		-p "${jaeger_ui_port}:${jaeger_ui_port}" \
		-p 14268:14268 \
		-p 9411:9411 \
		"$jaeger_docker_image"

	sudo mkdir -m 0750 -p "$TRACE_LOG_DIR"
}

stop_jaeger()
{
	docker stop "${jaeger_docker_container_name}"
	docker rm -f "${jaeger_docker_container_name}"
}

# Returns status from Jaeger web UI
get_jaeger_status()
{
	local service="$1"

	[ -z "$service" ] && die "need jaeger service name"

	local attempt=0
	local status=""

	while [ $attempt -lt 10 ]
	do
		status=$(curl -s "http://${jaeger_server}:${jaeger_ui_port}/api/traces?service=${service}" 2>/dev/null)
		local ret=$?

		[ "$ret" -eq 0 ] && [ -n "$status" ] && break

		attempt=$((attempt+1))
		sleep 1
	done

	echo "$status"
}

# Look for any "dangling" spans that have not been reported to the Jaeger
# agent.
check_missing_spans()
{
	local service="$1"
	local min_spans="$2"

	[ -z "$service" ] && die "need jaeger service name"
	[ -z "$min_spans" ] && die "need minimum trace span count"

	local logfile=$(mktemp)

	sudo journalctl -q -o cat -a -t "$service" > "$logfile"
	sudo chown "$USER:" "$logfile"

	# This message needs to be logged by each Kata component, generally when
	# debug is enabled.
	local component_prefix="created span"

	# Message prefix added by Jaeger when LogSpans=true
	# (see
	# https://godoc.org/github.com/uber/jaeger-client-go/config#ReporterConfig).
	local jaeger_reporter_prefix="Reporting span"

	local logged_spans
	if ! logged_spans=$(grep -E -o "${component_prefix} [^ ][^ ]*" "$logfile" | awk '{print $3}')
	then
		info "failed to check logged spans"
		rm -f "$logfile"
		return
	fi

	if [ -z "$logged_spans" ]
	then
		info "No logged spans to check"
		rm -f "$logfile"
		return
	fi

	local count=0

	for span in $logged_spans
	do
		count=$((count+1))

		# Remove quotes
		span=$(echo $span|tr -d '"')

		grep -E -q "${jaeger_reporter_prefix} \<$span\>" "$logfile" || \
			die "span $count ($span) not reported"
	done

	[ "$count" -lt "$min_spans" ] && \
		die "expected >= $min_spans reported spans, got $count"

	info "All $count spans reported"

	rm -f "$logfile"
}

# Check Jaeger spans for the specified service.
check_jaeger_status()
{
	local service="$1"
	local min_spans="$2"

	[ -z "$service" ] && die "need jaeger service name"
	[ -z "$min_spans" ] && die "need minimum trace span count"

	local status
	local errors=0

	local attempt=0
	local attempts=3

	local trace_logfile=$(printf "%s/%s-traces.json" "$TRACE_LOG_DIR" "$service")

	info "Checking Jaeger status (and logging traces to ${trace_logfile})"

	while [ "$attempt" -lt "$attempts" ]
	do
		status=$(get_jaeger_status "$service")

		#------------------------------
		# Basic sanity checks
		[ -z "$status" ] && die "failed to query status via HTTP"

		local span_lines=$(echo "$status"|jq -S '.data[].spans | length')
		[ -z "$span_lines" ] && die "no span status"

		# Log the spans to allow for analysis in case the test fails
		echo "$status"|jq -S .|sudo tee "$trace_logfile" >/dev/null

		local span_lines_count=$(echo "$span_lines"|wc -l)

		# Total up all span counts
		local spans=$(echo "$span_lines"|paste -sd+ -|bc)
		[ -z "$spans" ] && die "no spans"

		# Ensure total span count is numeric
		echo "$spans"|grep -q "^[0-9][0-9]*$"
		[ $? -eq 0 ] || die "invalid span count: '$spans'"

		info "found $spans spans (across $span_lines_count traces)"

		# Validate
		[ "$spans" -lt "$min_spans" ] && die "expected >= $min_spans spans, got $spans"

		# Look for common errors in span data
		local errors1
		if errors=$(echo "$status"|jq -S . 2>/dev/null|grep "invalid parent span")
		then
			errors=$((errors+1))
			warn "Found invalid parent span errors (attempt $attempt): $errors1"
			attempt=$((attempt+1))
			continue
		else
			errors=$((errors-1))
			[ "$errors" -lt 0 ] && errors=0
		fi

		# Crude but it works
		local errors2
		if errors2=$(echo "$status"|jq -S . 2>/dev/null|grep "\"warnings\""|grep -E -v "\<null\>")
		then
			errors=$((errors+1))
			warn "Found warnings (attempt $attempt): $errors2"
			attempt=$((attempt+1))
			continue
		else
			errors=$((errors-1))
			[ "$errors" -lt 0 ] && errors=0
		fi

		attempt=$((attempt+1))

		[ "$errors" -eq 0 ] && break
	done

	[ "$errors" -eq 0 ] || die "errors still detected after $attempts attempts"
}

run_test()
{
	local min_spans="$1"
	local service="$2"

	[ -z "$min_spans" ] && die "need minimum span count"
	[ -z "$service" ] && die "need service name"

	create_trace

	check_jaeger_status "$service" "$min_spans"

	check_missing_spans "$service" "$min_spans"

	info "test passed"
}

main()
{
	runtime_min_spans=10
	shim_min_spans=5

	# Name of Jaeger "services" (aka Kata components) to check trace for.
	runtime_service="kata-runtime"
	shim_service="kata-shim"

	start_jaeger

	"${SCRIPT_PATH}/../.ci/configure_tracing_for_kata.sh" enable

	info "Checking runtime spans"
	run_test "$runtime_min_spans" "$runtime_service"

	info "Checking shim spans"
	run_test "$shim_min_spans" "$shim_service"

	# The tests worked so remove the logs
	if sudo [ -d "$TRACE_LOG_DIR" ] && [ "$TRACE_LOG_DIR" != "/" ]
	then
		info "Removing cached trace logs from $TRACE_LOG_DIR"
		sudo rm -rf "$TRACE_LOG_DIR"
	fi
}

main "$@"
