#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Check that the cluster's IP family (K8S_IP_FAMILY: ipv4, ipv6 or dual) is
# what both Kubernetes and the Kata guest see, and that traffic flows over
# every family the cluster has.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# Print how many of the given addresses are IPv4 and IPv6, as "<v4> <v6>".
count_families() {
	local v4=0
	local v6=0
	local addr

	for addr in "$@"; do
		if [[ "${addr}" == *:* ]]; then
			v6=$((v6 + 1))
		else
			v4=$((v4 + 1))
		fi
	done
	echo "${v4} ${v6}"
}

# Fetch the nginx welcome page from the client pod.
client_fetch() {
	local host="$1"

	[[ "${host}" == *:* ]] && host="[${host}]"
	kubectl exec "${client_pod}" -- wget -T 5 -O- "http://${host}/" | grep -q "Welcome to nginx"
}

setup() {
	[ "${CONTAINER_RUNTIME}" == "crio" ] && skip "test not working see: https://github.com/kata-containers/kata-containers/issues/10414"
	auto_generate_policy_enabled && skip "execs depend on addresses only known at runtime"

	setup_common || die "setup_common failed"

	server_pod="ip-family-server"
	client_pod="ip-family-client"
	service="ip-family-svc"

	case "${K8S_IP_FAMILY}" in
		ipv4) expected_families="1 0" ;;
		ipv6) expected_families="0 1" ;;
		dual) expected_families="1 1" ;;
		*) die "Unsupported K8S_IP_FAMILY=${K8S_IP_FAMILY}" ;;
	esac

	export IP_FAMILY_POLICY="SingleStack"
	[[ "${K8S_IP_FAMILY}" == "dual" ]] && IP_FAMILY_POLICY="RequireDualStack"

	yaml_file="${pod_config_dir}/test-pod-ip-family.yaml"
	set_nginx_image "${pod_config_dir}/pod-ip-family.yaml" "${yaml_file}"
	enable_nginx_ipv6_listener "${yaml_file}"

	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${server_pod}" "${client_pod}"
}

@test "Pod, Service and guest addresses match the cluster IP family" {
	local -a pod_ips service_ips
	read -r -a pod_ips <<< "$(kubectl get pod "${server_pod}" -o jsonpath='{.status.podIPs[*].ip}')"
	read -r -a service_ips <<< "$(kubectl get service "${service}" -o jsonpath='{.spec.clusterIPs[*]}')"
	echo "pod IPs: ${pod_ips[*]}, service IPs: ${service_ips[*]}"

	[[ "$(count_families "${pod_ips[@]}")" == "${expected_families}" ]]
	[[ "$(count_families "${service_ips[@]}")" == "${expected_families}" ]]

	# Kata guests have their own network stack, so check the addresses the
	# guest itself configured rather than only what Kubernetes reports.
	local guest_v4 guest_v6
	kubectl exec "${client_pod}" -- ip addr show
	kubectl exec "${client_pod}" -- ip -4 route show
	kubectl exec "${client_pod}" -- ip -6 route show
	guest_v4="$(kubectl exec "${client_pod}" -- ip -o -4 addr show scope global | wc -l)"
	guest_v6="$(kubectl exec "${client_pod}" -- ip -o -6 addr show scope global | wc -l)"
	[[ "${guest_v4} ${guest_v6}" == "${expected_families}" ]]

	if [[ "${K8S_IP_FAMILY}" == "ipv6" ]]; then
		[[ -z "$(kubectl exec "${client_pod}" -- ip -4 route show)" ]]
	fi
}

@test "Pod-to-Pod, Service and DNS traffic over every cluster IP family" {
	local -a pod_ips service_ips
	read -r -a pod_ips <<< "$(kubectl get pod "${server_pod}" -o jsonpath='{.status.podIPs[*].ip}')"
	read -r -a service_ips <<< "$(kubectl get service "${service}" -o jsonpath='{.spec.clusterIPs[*]}')"

	local ip host
	for ip in "${pod_ips[@]}"; do
		info "Fetching from ${ip}"
		client_fetch "${ip}"
	done

	# kube-proxy refuses connections to a Service until it has programmed
	# the endpoints of a pod that only just became Ready.
	for host in "${service_ips[@]}" "${service}"; do
		info "Fetching from ${host}"
		waitForProcess 30 1 "client_fetch '${host}'"
	done

	if [[ "${K8S_IP_FAMILY}" == "ipv6" ]]; then
		# Expect the missing route rather than a timeout, so this cannot pass
		# because something merely dropped the packets.
		run kubectl exec "${client_pod}" -- wget -T 5 -O- "http://10.244.0.1/"
		echo "${output}"
		[[ "${status}" -ne 0 ]]
		[[ "${output}" == *"Network is unreachable"* ]]
	fi
}

teardown() {
	[ "${CONTAINER_RUNTIME}" == "crio" ] && skip "test not working see: https://github.com/kata-containers/kata-containers/issues/10414"
	auto_generate_policy_enabled && skip "execs depend on addresses only known at runtime"

	kubectl get pods,services -o wide || true
	kubectl delete -f "${yaml_file}" --ignore-not-found || true
	rm -f "${yaml_file}"
	teardown_common "${node}" "${node_start_time:-}"
}
