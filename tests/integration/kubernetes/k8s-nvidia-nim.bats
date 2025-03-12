#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# shellcheck disable=SC2154  # BATS variables are not assigned in this file
load "${BATS_TEST_DIRNAME}/../../common.bash"
# shellcheck disable=SC1091
load "${BATS_TEST_DIRNAME}/tests_common.sh"

export POD_NAME_INSTRUCT="nvidia-nim-llama-3-1-8b-instruct"
export POD_NAME_EMBEDQA="nvidia-nim-llama-3-2-nv-embedqa-1b-v2"

export POD_SECRET_INSTRUCT="ngc-secret-instruct"

DOCKER_CONFIG_JSON=$(
    echo -n "{\"auths\":{\"nvcr.io\":{\"username\":\"\$oauthtoken\",\"password\":\"${NGC_API_KEY}\",\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" |
        base64 -w0
)
export DOCKER_CONFIG_JSON

setup_file() {
    dpkg -s jq >/dev/null 2>&1 || sudo apt -y install jq

    export PYENV_ROOT="${HOME}/.pyenv"
    [[ -d ${PYENV_ROOT}/bin ]] && export PATH="${PYENV_ROOT}/bin:${PATH}"
    eval "$(pyenv init - bash)"

    python3 -m venv "${HOME}"/.cicd/venv

    get_pod_config_dir

    pod_instruct_yaml_in="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml.in"
    pod_instruct_yaml="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml"

    envsubst <"${pod_instruct_yaml_in}" >"${pod_instruct_yaml}"

    export POD_INSTRUCT_YAML="${pod_instruct_yaml}"
}

@test "NVIDIA NIM Llama 3.1-8b Instruct" {
    kubectl apply -f "${POD_INSTRUCT_YAML}"
    kubectl wait --for=condition=Ready --timeout=500s pod "${POD_NAME_INSTRUCT}"
    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    POD_IP_INSTRUCT=$(kubectl get pod "${POD_NAME_INSTRUCT}" -o jsonpath='{.status.podIP}')
    [[ -n "${POD_IP_INSTRUCT}" ]]

    echo "POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >"${BATS_SUITE_TMPDIR}/env"
    echo "# POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >&3
}

@test "List of models available for inference" {
    # shellcheck disable=SC1091  # File is created by previous test
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variable is shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]

    # shellcheck disable=SC2031  # Variable is shared via file between BATS tests
    run curl -sX GET "http://${POD_IP_INSTRUCT}:8000/v1/models"
    [[ "${status}" -eq 0 ]]

    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    MODEL_NAME=$(echo "${output}" | jq '.data[0].id' | tr -d '"')
    export MODEL_NAME
    [[ -n "${MODEL_NAME}" ]]
    echo "MODEL_NAME=${MODEL_NAME}" >>"${BATS_SUITE_TMPDIR}/env"
    echo "# MODEL_NAME=${MODEL_NAME}" >&3

}

@test "Simple OpenAI completion request" {
    # shellcheck disable=SC1091  # File is created by previous test
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${MODEL_NAME}" ]]

    QUESTION="What are Kata Containers?"

    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    run curl -sX 'POST' \
        "http://${POD_IP_INSTRUCT}:8000/v1/completions" \
        -H "accept: application/json" \
        -H "Content-Type: application/json" \
        -d "{\"model\": \"${MODEL_NAME}\", \"prompt\": \"${QUESTION}\", \"max_tokens\": 64}"

    ANSWER=$(echo "${output}" | jq '.choices[0].text')
    [[ -n "${ANSWER}" ]]

    echo "# QUESTION: ${QUESTION}" >&3
    echo "# ANSWER: ${ANSWER}" >&3
}

teardown_file() {
        kubectl delete -f "${POD_INSTRUCT_YAML}"
}
