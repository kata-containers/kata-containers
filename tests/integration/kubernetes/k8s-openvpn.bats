#!/usr/bin/env bats
#
# Copyright (c) 2025 Microsoft Corporation
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
    # cannot build the container image: ERROR: unable to select packages: easy-rsa (no such package) ...
    [ "$(uname -m)" == "ppc64le" ] && skip "required packages for openvpn test not available for ppc64le"
    # built the container image only for x86 and arm64 so far
    [ "$(uname -m)" == "s390x" ] && skip "container image not built for s390x"

    setup_common
    get_pod_config_dir

    init_pod_name="openvpn-init-secrets"
    server_pod_name="openvpn-server"
    client_pod_name="openvpn-client"

    init_pod_yaml="${pod_config_dir}/openvpn/pod-openvpn-init-secrets.yaml"

    server_pod_yaml="${pod_config_dir}/openvpn/openvpn-server-pod.yaml"
    server_service_yaml="${pod_config_dir}/openvpn/openvpn-server-service.yaml"
    server_configmap_yaml="${pod_config_dir}/openvpn/openvpn-server-configmap.yaml"
    server_secret_template_yaml="${pod_config_dir}/openvpn/openvpn-server-secret.yaml.in"
    server_secret_instance_yaml="${pod_config_dir}/openvpn/openvpn-server-secret-instance.yaml"

    client_pod_yaml="${pod_config_dir}/openvpn/openvpn-client-pod.yaml"
    client_configmap_yaml="${pod_config_dir}/openvpn/openvpn-client-configmap.yaml"
    client_secret_template_yaml="${pod_config_dir}/openvpn/openvpn-client-secret.yaml.in"
    client_secret_instance_yaml="${pod_config_dir}/openvpn/openvpn-client-secret-instance.yaml"

    # TODO: workaround for issue 11777: https://github.com/kata-containers/kata-containers/issues/11777
    # remove allow-all configuration and uncomment below when resolved (or loop over files under openvpn folder)
    add_allow_all_policy_to_yaml "$server_pod_yaml"
    add_allow_all_policy_to_yaml "$client_pod_yaml"
    #policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
    #add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
    #auto_generate_policy "${policy_settings_dir}" "$server_pod_yaml"
    #auto_generate_policy "${policy_settings_dir}" "$client_pod_yaml"
}

@test "Pods establishing a VPN connection using openvpn" {
    # Step 1: Deploy the initialization pod and wait for it to be ready
    kubectl apply -f "$init_pod_yaml"  && kubectl wait --for=condition=Ready --timeout=$timeout pod/$init_pod_name

    # Step 2: Extract base64-encoded certificates from the initialization pod
    export BASE64_CA_CRT="$(kubectl exec $init_pod_name -- cat /etc/openvpn/ca.crt.b64 | tr -d '\n')"
    export BASE64_CLIENT_CRT="$(kubectl exec $init_pod_name -- cat /etc/openvpn/client.crt.b64 | tr -d '\n')"
    export BASE64_CLIENT_KEY="$(kubectl exec $init_pod_name -- cat /etc/openvpn/client.key.b64 | tr -d '\n')"
    export BASE64_SERVER_CRT="$(kubectl exec $init_pod_name -- cat /etc/openvpn/server.crt.b64 | tr -d '\n')"
    export BASE64_SERVER_KEY="$(kubectl exec $init_pod_name -- cat /etc/openvpn/server.key.b64 | tr -d '\n')"

    [ -n "$BASE64_CA_CRT" ]
    [ -n "$BASE64_CLIENT_CRT" ]
    [ -n "$BASE64_CLIENT_KEY" ]
    [ -n "$BASE64_SERVER_CRT" ]
    [ -n "$BASE64_SERVER_KEY" ]

    # Step 3: Substitute environment variables in template files, write to instance files
    envsubst < "$server_secret_template_yaml" > "$server_secret_instance_yaml"
    envsubst < "$client_secret_template_yaml" > "$client_secret_instance_yaml"

    # Step 4: Deploy the OpenVPN server and wait for it to be ready (uses readiness probe)
    kubectl apply -f "$server_service_yaml"
    kubectl apply -f "$server_configmap_yaml"
    kubectl apply -f "$server_secret_instance_yaml"
    kubectl apply -f "$server_pod_yaml" && kubectl wait --for=condition=Ready --timeout=$timeout pod/$server_pod_name

    # Step 5: Deploy the OpenVPN client and wait for it to be ready (uses readiness probe)
    kubectl apply -f "$client_configmap_yaml"
    kubectl apply -f "$client_secret_instance_yaml"
    kubectl apply -f "$client_pod_yaml" && kubectl wait --for=condition=Ready --timeout=$timeout pod/$client_pod_name
}

teardown() {
    [ "$(uname -m)" == "ppc64le" ] && skip "required packages for openvpn test not available for ppc64le"
    [ "$(uname -m)" == "s390x" ] && skip "container image not built for s390x"

    # Debugging information
    echo "=== OpenVPN Init Pod Logs ==="
    kubectl logs "$init_pod_name" --all-containers=true || true
    echo "=== OpenVPN Server Pod Logs ==="
    kubectl logs "$server_pod_name" || true
    echo "=== OpenVPN Client Pod Logs ==="
    kubectl logs "$client_pod_name" || true

    # TODO, see above, workaround for issue 11777. Uncomment when resolved.
    #delete_tmp_policy_settings_dir "${policy_settings_dir}"
    teardown_common "${node}" "${node_start_time:-}"

    # teardown cleans up pods, but not other resources
    kubectl delete -f "$client_secret_instance_yaml" --ignore-not-found=true
    kubectl delete -f "$client_configmap_yaml" --ignore-not-found=true
    kubectl delete -f "$server_secret_instance_yaml" --ignore-not-found=true
    kubectl delete -f "$server_configmap_yaml" --ignore-not-found=true
    kubectl delete -f "$server_service_yaml" --ignore-not-found=true
}
