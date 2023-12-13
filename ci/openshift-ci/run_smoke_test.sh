#!/bin/bash
#
# Copyright (c) 2020 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# Run a smoke test.
#

script_dir=$(dirname $0)
source ${script_dir}/../lib.sh

pod='http-server'

# Create a pod.
#
info "Creating the ${pod} pod"
oc apply -f ${script_dir}/smoke/${pod}.yaml || \
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
rm -f hello_msg.txt
waitForProcess 60 1 "curl '${host}:${port}${hello_file}' -s -o hello_msg.txt"

grep "${hello_msg}" hello_msg.txt > /dev/null
test_status=$?
if [ $test_status -eq 0 ]; then
	info "HTTP server is working"
else
	info "HTTP server is unreachable"
fi

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
