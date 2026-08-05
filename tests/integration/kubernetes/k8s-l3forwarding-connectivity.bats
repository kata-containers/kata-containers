#!/usr/bin/env bats
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	# The l3forwarding network model is only implemented in runtime-rs.
	is_runtime_rs || skip "l3forwarding is only supported by runtime-rs"

	[ "${CONTAINER_RUNTIME}" == "crio" ] && skip "test not working see: https://github.com/kata-containers/kata-containers/issues/10414"

	setup_common || die "setup_common failed"

	busybox_image="quay.io/prometheus/busybox:latest"
	# Matches metadata.name in nginx-deployment.yaml (set_nginx_image only
	# rewrites the image, not the resource name).
	deployment="nginx-deployment"

	# Switch the guest network model to l3forwarding via a runtime config
	# drop-in, so the pods created below exercise that model instead of the
	# default. Paired with removal in teardown to avoid leaking the drop-in
	# onto every subsequent pod on the node.
	local dropin_file="${BATS_FILE_TMPDIR}/99-k8s-l3forwarding.toml"
	cat > "${dropin_file}" <<EOF
[runtime]
internetworking_model = "l3forwarding"
EOF
	runtime_config_dropin="$(set_kata_runtime_config_dropin_file \
		"${node}" \
		"${dropin_file}")" || \
		skip "No Kata runtime config found for ${KATA_HYPERVISOR}"

	# Create test .yaml
	yaml_file="${pod_config_dir}/test-${deployment}.yaml"
	set_nginx_image "${pod_config_dir}/nginx-deployment.yaml" "${yaml_file}"

	auto_generate_policy "${pod_config_dir}" "${yaml_file}"
}

@test "Verify nginx connectivity between pods under l3forwarding" {
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Available --timeout=$timeout deployment/${deployment}
	kubectl expose deployment/${deployment}

	busybox_pod="test-nginx-l3fwd"
	# We need to use `-O index.html` as the busybox' wget has a different behaviour
	# than GNU's wget, which would just append a .n to the file name instead of bailing.
	kubectl run $busybox_pod --restart=Never -it --image="$busybox_image" \
		-- sh -c 'i=1; while [ $i -le '"$wait_time"' ]; do wget -O index.html --timeout=5 '"$deployment"' && break; sleep 1; i=$(expr $i + 1); done'

	# check pod's status, it should be Succeeded.
	[ $(kubectl get pods/$busybox_pod -o jsonpath="{.status.phase}") = "Succeeded" ]
	kubectl logs "$busybox_pod" | grep "index.html"
}

teardown() {
	is_runtime_rs || skip "l3forwarding is only supported by runtime-rs"
	[ "${CONTAINER_RUNTIME}" == "crio" ] && skip "test not working see: https://github.com/kata-containers/kata-containers/issues/10414"

	# Debugging information
	kubectl describe "pod/$busybox_pod" || true
	kubectl logs "$busybox_pod" || true
	kubectl get deployment/${deployment} -o yaml || true

	rm -f "${yaml_file}"
	kubectl delete deployment "$deployment" || true
	kubectl delete service "$deployment" || true
	kubectl delete pod "$busybox_pod" || true

	remove_kata_runtime_config_dropin_file "${node}" "${runtime_config_dropin:-}"
	teardown_common "${node}" "${node_start_time:-}"
}
