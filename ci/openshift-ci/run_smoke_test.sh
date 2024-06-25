#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# Run a smoke test.
#

script_dir=$(dirname $0)
source ${script_dir}/lib.sh

pod='http-server'

# Create a pod.
#
info "Creating the ${pod} pod"
[ -z "$KATA_RUNTIME" ] && die "Please set the KATA_RUNTIME first"
envsubst < "${script_dir}/smoke/${pod}.yaml.in" | \
	oc apply -f - || \
	die "failed to create ${pod} pod"

# Check it eventually goes to 'running'
#
wait_time=600
sleep_time=5
cmd="oc get pod/${pod} -o jsonpath='{.status.containerStatuses[0].state}' | \
	grep running > /dev/null"
info "Wait until the pod gets running"
waitForProcess $wait_time $sleep_time "$cmd" || timed_out=$?
if [ -n "$timed_out" ]; then
	oc describe pod/${pod}
	oc delete pod/${pod}
	die "${pod} not running"
fi
info "${pod} is running"

# Add a file with the hello message
#
hello_file=/tmp/hello
hello_msg='Hello World'
oc exec ${pod} -- sh -c "echo $hello_msg > $hello_file"

info "Creating the service and route"
if oc apply -f ${script_dir}/smoke/service.yaml; then
    # Likely on OCP, use service
    is_ocp=1
    host=$(oc get route/http-server-route -o jsonpath={.spec.host})
    port=80
else
    # Likely on plain kubernetes, test using another container
    is_ocp=0
    info "Failed to create service, likely not on OCP, trying via NodePort"
    oc apply -f "${script_dir}/smoke/service_kubernetes.yaml"
    # For some reason kcli's cluster lists external IP as internal IP, try both
    host=$(oc get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="ExternalIP")].address}')
    [ -z "$host"] && host=$(oc get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}')
    port=$(oc get service/http-server-service -o jsonpath='{.spec.ports[0].nodePort}')
fi

info "Wait for the HTTP server to respond"
tempfile=$(mktemp)
check_cmd="curl -vvv '${host}:${port}${hello_file}' 2>&1 | tee -a '$tempfile' | grep -q '$hello_msg'"
if waitForProcess 60 1 "${check_cmd}"; then
    test_status=0
    info "HTTP server is working"
else
    test_status=1
    echo "::error:: HTTP server not working"
    echo "::group::Output of the \"curl -vvv '${host}:${port}${hello_file}'\""
    cat "${tempfile}"
    echo "::endgroup::"
    echo "::group::Describe kube-system namespace"
    oc describe -n kube-system all
    echo "::endgroup::"
    echo "::group::Descibe current namespace"
    oc describe all
    echo "::endgroup::"
    info "HTTP server is unreachable"
fi
rm -f "$tempfile"

# Delete the resources.
#
info "Deleting the service/route"
if [ "$is_ocp" -eq 0 ]; then
    oc delete -f ${script_dir}/smoke/service_kubernetes.yaml
else
    oc delete -f ${script_dir}/smoke/service.yaml
fi
info "Deleting the ${pod} pod"
oc delete pod/${pod} || test_status=$?

exit $test_status
