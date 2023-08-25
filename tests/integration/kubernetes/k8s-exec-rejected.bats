#!/usr/bin/env bats
#
# Copyright (c) 2023 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	pod_name="busybox"
	pod_yaml="${pod_config_dir}/busybox-pod.yaml"

	# String generated using "base64 -w 0 kata-containers/src/kata-opa/allow-all-except-exec-process.rego"
	allow_all_except_exec_policy="cGFja2FnZSBhZ2VudF9wb2xpY3kKCmRlZmF1bHQgQWRkQVJQTmVpZ2hib3JzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgQWRkU3dhcFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENsb3NlU3RkaW5SZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBDb3B5RmlsZVJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENyZWF0ZUNvbnRhaW5lclJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IENyZWF0ZVNhbmRib3hSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBEZXN0cm95U2FuZGJveFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IEdldE1ldHJpY3NSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBHZXRPT01FdmVudFJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IEd1ZXN0RGV0YWlsc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IExpc3RJbnRlcmZhY2VzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgTGlzdFJvdXRlc1JlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IE1lbUhvdHBsdWdCeVByb2JlUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgT25saW5lQ1BVTWVtUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUGF1c2VDb250YWluZXJSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBQdWxsSW1hZ2VSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBSZWFkU3RyZWFtUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVtb3ZlQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVtb3ZlU3RhbGVWaXJ0aW9mc1NoYXJlTW91bnRzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgUmVzZWVkUmFuZG9tRGV2UmVxdWVzdCA6PSBmYWxzZQpkZWZhdWx0IFJlc3VtZUNvbnRhaW5lclJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFNldEd1ZXN0RGF0ZVRpbWVSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBTZXRQb2xpY3lSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBTaWduYWxQcm9jZXNzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgU3RhcnRDb250YWluZXJSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBTdGFydFRyYWNpbmdSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBTdGF0c0NvbnRhaW5lclJlcXVlc3QgOj0gdHJ1ZQpkZWZhdWx0IFN0b3BUcmFjaW5nUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgVHR5V2luUmVzaXplUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgVXBkYXRlQ29udGFpbmVyUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgVXBkYXRlRXBoZW1lcmFsTW91bnRzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgVXBkYXRlSW50ZXJmYWNlUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgVXBkYXRlUm91dGVzUmVxdWVzdCA6PSB0cnVlCmRlZmF1bHQgV2FpdFByb2Nlc3NSZXF1ZXN0IDo9IHRydWUKZGVmYXVsdCBXcml0ZVN0cmVhbVJlcXVlc3QgOj0gdHJ1ZQoKZGVmYXVsdCBFeGVjUHJvY2Vzc1JlcXVlc3QgOj0gZmFsc2UK"
}

@test "Kubectl exec rejected by policy" {
	# Add to the YAML file a policy that rejects ExecProcessRequest.
	yq write -i "${pod_yaml}" \
		'metadata.annotations."io.katacontainers.config.agent.policy"' \
		"${allow_all_except_exec_policy}"

	# Create the pod
	kubectl create -f "${pod_yaml}"

	# Wait for pod to start
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Try executing a command in the Pod - an action rejected by the agent policy.
	kubectl exec "$pod_name" -- date 2>&1 | grep "ExecProcessRequest is blocked by policy"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
