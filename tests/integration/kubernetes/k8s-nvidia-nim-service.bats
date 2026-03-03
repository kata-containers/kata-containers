#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This file is modeled after k8s-nvidia-nim.bats which contains helpful in-line documentation.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-nvidia-gpu}"

TEE=false
if is_confidential_gpu_hardware; then
    TEE=true
fi
export TEE

NIM_SERVICE_NAME="meta-llama-3-2-1b-instruct"
[[ "${TEE}" = "true" ]] && NIM_SERVICE_NAME="meta-llama-3-2-1b-instruct-tee"
export NIM_SERVICE_NAME

POD_READY_TIMEOUT_LLAMA_3_2_1B_PREDEFINED=600s
[[ "${TEE}" = "true" ]] && POD_READY_TIMEOUT_LLAMA_3_2_1B_PREDEFINED=1200s
export POD_READY_TIMEOUT_LLAMA_3_2_1B=${POD_READY_TIMEOUT_LLAMA_3_2_1B:-${POD_READY_TIMEOUT_LLAMA_3_2_1B_PREDEFINED}}

export LOCAL_NIM_CACHE_LLAMA_3_2_1B="${LOCAL_NIM_CACHE_LLAMA_3_2_1B:-${LOCAL_NIM_CACHE:-/opt/nim/.cache}-llama-3-2-1b}"

DOCKER_CONFIG_JSON=$(
    echo -n "{\"auths\":{\"nvcr.io\":{\"username\":\"\$oauthtoken\",\"password\":\"${NGC_API_KEY}\",\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" |
        base64 -w0
)
export DOCKER_CONFIG_JSON

KBS_AUTH_CONFIG_JSON=$(
    echo -n "{\"auths\":{\"nvcr.io\":{\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" |
        base64 -w0
)
export KBS_AUTH_CONFIG_JSON

NGC_API_KEY_BASE64=$(
    echo -n "${NGC_API_KEY}" | base64 -w0
)
export NGC_API_KEY_BASE64

# Points to kbs:///default/ngc-api-key/instruct and thus re-uses the secret from k8s-nvidia-nim.bats.
NGC_API_KEY_SEALED_SECRET_LLAMA_3_2_1B="${SEALED_SECRET_PRECREATED_NIM_INSTRUCT}"
export NGC_API_KEY_SEALED_SECRET_LLAMA_3_2_1B

NGC_API_KEY_SEALED_SECRET_LLAMA_3_2_1B_BASE64=$(echo -n "${NGC_API_KEY_SEALED_SECRET_LLAMA_3_2_1B}" | base64 -w0)
export NGC_API_KEY_SEALED_SECRET_LLAMA_3_2_1B_BASE64

# NIM Operator (k8s-nim-operator) install/uninstall for NIMService CRD.
NIM_OPERATOR_NAMESPACE="${NIM_OPERATOR_NAMESPACE:-nim-operator}"
NIM_OPERATOR_RELEASE_NAME="nim-operator"

install_nim_operator() {
	command -v helm &>/dev/null || die "helm is required but not installed"
	echo "Installing NVIDIA NIM Operator (latest chart)"
	helm repo add nvidia https://helm.ngc.nvidia.com/nvidia
	helm repo update

	kubectl create namespace "${NIM_OPERATOR_NAMESPACE}" --dry-run=client -o yaml | kubectl apply -f -

	helm upgrade --install "${NIM_OPERATOR_RELEASE_NAME}" nvidia/k8s-nim-operator \
		-n "${NIM_OPERATOR_NAMESPACE}" \
		--wait

	local deploy_name
	deploy_name=$(kubectl get deployment -n "${NIM_OPERATOR_NAMESPACE}" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || true)
	if [[ -n "${deploy_name}" ]]; then
		kubectl wait --for=condition=available --timeout=300s "deployment/${deploy_name}" -n "${NIM_OPERATOR_NAMESPACE}"
	fi
	echo "NIM Operator install complete."
}

uninstall_nim_operator() {
	echo "Uninstalling NVIDIA NIM Operator (release: ${NIM_OPERATOR_RELEASE_NAME}, namespace: ${NIM_OPERATOR_NAMESPACE})"
	if helm status "${NIM_OPERATOR_RELEASE_NAME}" -n "${NIM_OPERATOR_NAMESPACE}" &>/dev/null; then
		helm uninstall "${NIM_OPERATOR_RELEASE_NAME}" -n "${NIM_OPERATOR_NAMESPACE}" || true
		kubectl delete namespace "${NIM_OPERATOR_NAMESPACE}" --ignore-not-found=true --timeout=60s || true
		echo "NIM Operator uninstall complete."
	else
		echo "NIM Operator release not found, nothing to uninstall."
	fi
}

setup_kbs_credentials() {
    CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
    export CC_KBS_ADDR

    kubectl delete secret ngc-secret-llama-3-2-1b --ignore-not-found
    kubectl create secret docker-registry ngc-secret-llama-3-2-1b --docker-server="nvcr.io" --docker-username="\$oauthtoken" --docker-password="${NGC_API_KEY}"

    kbs_set_gpu0_resource_policy
    kbs_set_resource_base64 "default" "credentials" "nvcr" "${KBS_AUTH_CONFIG_JSON}"
    kbs_set_resource "default" "ngc-api-key" "instruct" "${NGC_API_KEY}"
}

# CDH initdata for guest-pull: KBS URL, registry credentials URI, and allow-all policy.
# NIMService is not supported by genpolicy; add_allow_all_policy_to_yaml only supports Pod/Deployment.
# Build initdata with policy inline so TEE pods get both CDH config and policy.
create_nim_initdata_file_llama_3_2_1b() {
    local output_file="$1"
    local cc_kbs_address
    cc_kbs_address=$(kbs_k8s_svc_http_addr)
    local allow_all_rego="${BATS_TEST_DIRNAME}/../../../src/kata-opa/allow-all.rego"

    cat > "${output_file}" << EOF
version = "0.1.0"
algorithm = "sha256"

[data]
"aa.toml" = '''
[token_configs]
[token_configs.kbs]
url = "${cc_kbs_address}"
'''

"cdh.toml" = '''
[kbc]
name = "cc_kbc"
url = "${cc_kbs_address}"

[image]
authenticated_registry_credentials_uri = "kbs:///default/credentials/nvcr"
'''

"policy.rego" = '''
$(cat "${allow_all_rego}")
'''
EOF
}

setup() {
    setup_common || die "setup_common failed"
    install_nim_operator || die "NIM Operator install failed"

    dpkg -s jq >/dev/null 2>&1 || sudo apt -y install jq

    # Same pattern as k8s-nvidia-nim.bats: choose manifest by TEE; each YAML has literal secret names.
    local tee_suffix=""
    [[ "${TEE}" = "true" ]] && tee_suffix="-tee"
    export NIM_YAML_IN="${pod_config_dir}/nvidia-nim-llama-3-2-1b-instruct-service${tee_suffix}.yaml.in"
    export NIM_YAML="${pod_config_dir}/nvidia-nim-llama-3-2-1b-instruct-service${tee_suffix}.yaml"

    if [[ "${TEE}" = "true" ]]; then
        setup_kbs_credentials
        setup_sealed_secret_signing_public_key
        initdata_file="${BATS_SUITE_TMPDIR}/nim-initdata-llama-3-2-1b.toml"
        create_nim_initdata_file_llama_3_2_1b "${initdata_file}"
        NIM_INITDATA_BASE64=$(gzip -c "${initdata_file}" | base64 -w0)
        export NIM_INITDATA_BASE64
    fi

    envsubst < "${NIM_YAML_IN}" > "${NIM_YAML}"
}

@test "NIMService llama-3.2-1b-instruct serves /v1/models" {
    echo "NIMService test: Applying NIM YAML"
    kubectl apply -f "${NIM_YAML}"
    echo "NIMService test: Waiting for deployment to exist (operator creates it from NIMService)"
    local wait_exist_timeout=30
    local elapsed=0
    while ! kubectl get deployment "${NIM_SERVICE_NAME}" &>/dev/null; do
        if [[ ${elapsed} -ge ${wait_exist_timeout} ]]; then
            echo "Deployment ${NIM_SERVICE_NAME} did not appear within ${wait_exist_timeout}s" >&2
            kubectl get deployment "${NIM_SERVICE_NAME}" 2>&1 || true
            false
        fi
        sleep 5
        elapsed=$((elapsed + 5))
    done
    local pod_name
    pod_name=$(kubectl get pods --no-headers -o custom-columns=":metadata.name" | head -1)
    echo "NIMService test: POD_NAME=${pod_name} (waiting for pod ready, timeout ${POD_READY_TIMEOUT_LLAMA_3_2_1B})"
    [[ -n "${pod_name}" ]]
    kubectl wait --for=condition=ready --timeout="${POD_READY_TIMEOUT_LLAMA_3_2_1B}" "pod/${pod_name}"
    local pod_ip
    pod_ip=$(kubectl get pod "${pod_name}" -o jsonpath='{.status.podIP}')
    echo "NIMService test: POD_IP=${pod_ip}"
    [[ -n "${pod_ip}" ]]

    echo "NIMService test: Curling http://${pod_ip}:8000/v1/models"
    run curl -sS --connect-timeout 10 "http://${pod_ip}:8000/v1/models"
    echo "NIMService test: /v1/models response: ${output}"
    [[ "${status}" -eq 0 ]]
    [[ "$(echo "${output}" | jq -r '.object')" == "list" ]]
    [[ "$(echo "${output}" | jq -r '.data[0].id')" == "meta/llama-3.2-1b-instruct" ]]
    [[ "$(echo "${output}" | jq -r '.data[0].object')" == "model" ]]

    echo "NIMService test: Curling http://${pod_ip}:8000/v1/chat/completions"
    run curl -sS --connect-timeout 30 "http://${pod_ip}:8000/v1/chat/completions" \
        -H "Content-Type: application/json" \
        -d '{"model":"meta/llama-3.2-1b-instruct","messages":[{"role":"user","content":"ping"}],"max_tokens":8}'
    echo "NIMService test: /v1/chat/completions response: ${output}"
    [[ "${status}" -eq 0 ]]
    [[ "$(echo "${output}" | jq -r '.object')" == "chat.completion" ]]
    [[ "$(echo "${output}" | jq -r '.model')" == "meta/llama-3.2-1b-instruct" ]]
    [[ "$(echo "${output}" | jq -r '.choices[0].message | has("content") or has("reasoning_content")')" == "true" ]]
}

teardown() {
    if kubectl get nimservice "${NIM_SERVICE_NAME}" &>/dev/null; then
        POD_NAME=$(kubectl get pods --no-headers -o custom-columns=":metadata.name" | head -1)
        if [[ -n "${POD_NAME}" ]]; then
            echo "=== NIMService pod logs ==="
            kubectl logs "${POD_NAME}" || true
            kubectl describe pod "${POD_NAME}" || true
        fi
        kubectl describe nimservice "${NIM_SERVICE_NAME}" || true
    fi

    [ -f "${NIM_YAML}" ] && kubectl delete -f "${NIM_YAML}" --ignore-not-found=true

    uninstall_nim_operator || true
    print_node_journal_since_test_start "${node}" "${node_start_time:-}" "${BATS_TEST_COMPLETED:-}"
}
