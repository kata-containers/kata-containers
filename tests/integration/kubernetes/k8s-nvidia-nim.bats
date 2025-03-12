#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

export POD_NAME_INSTRUCT="nvidia-nim-llama-3-1-8b-instruct"
export POD_NAME_EMBEDQA="nvidia-nim-llama-3-2-nv-embedqa-1b-v2"

export POD_SECRET_INSTRUCT="ngc-secret-instruct"

export DOCKER_CONFIG_JSON=$(
    echo -n "{\"auths\":{\"nvcr.io\":{\"username\":\"\$oauthtoken\",\"password\":\"${NGC_API_KEY}\",\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" |
        base64 -w0
)

setup_file() {
    dpkg -s jq 2>&1 >/dev/null || sudo apt -y install jq

    export PYENV_ROOT="$HOME/.pyenv"
    [[ -d $PYENV_ROOT/bin ]] && export PATH="$PYENV_ROOT/bin:$PATH"
    eval "$(pyenv init - bash)"

    python3 -m venv ${HOME}/.cicd/venv

    get_pod_config_dir

    pod_instruct_yaml_in="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml.in"
    pod_instruct_yaml="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml"

    envsubst <"${pod_instruct_yaml_in}" >"${pod_instruct_yaml}"

    export POD_INSTRUCT_YAML="${pod_instruct_yaml}"
}

@test "NVIDIA NIM Llama 3.1-8b Instruct" {
    kubectl apply -f "${POD_INSTRUCT_YAML}"
    kubectl wait --for=condition=Ready --timeout=500s pod "${POD_NAME_INSTRUCT}"
    POD_IP_INSTRUCT=$(kubectl get pod "${POD_NAME_INSTRUCT}" -o jsonpath='{.status.podIP}')
    [ -n "${POD_IP_INSTRUCT}" ]

    echo POD_IP_INSTRUCT=${POD_IP_INSTRUCT} >"$BATS_SUITE_TMPDIR/env"
    echo "# POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >&3
}

@test "List of models available for inference" {
    source "$BATS_SUITE_TMPDIR/env"
    [ -n "${POD_IP_INSTRUCT}" ]

    run curl -sX GET "http://${POD_IP_INSTRUCT}:8000/v1/models"
    [ "$status" -eq 0 ]

    export MODEL_NAME=$(echo "${output}" | jq '.data[0].id' | tr -d '"')
    [ -n "${MODEL_NAME}" ]
    echo MODEL_NAME=${MODEL_NAME} >>"$BATS_SUITE_TMPDIR/env"
    echo "# MODEL_NAME=${MODEL_NAME}" >&3

}

@test "Simple OpenAI completion request" {
    source "$BATS_SUITE_TMPDIR/env"
    [ -n ${POD_IP_INSTRUCT} ]
    [ -n ${MODEL_NAME} ]

    QUESTION="What are Kata Containers?"

    run curl -sX 'POST' \
        "http://${POD_IP_INSTRUCT}:8000/v1/completions" \
        -H "accept: application/json" \
        -H "Content-Type: application/json" \
        -d "{\"model\": \"${MODEL_NAME}\", \"prompt\": \"${QUESTION}\", \"max_tokens\": 64}"

    ANWSER=$(echo ${output} | jq '.choices[0].text')
    [ -n "${ANWSER}" ]

    echo "# QUESTION: ${QUESTION}" >&3
    echo "# ANWSER: ${ANWSER}" >&3
}

teardown_file() {
        kubectl delete -f "${POD_INSTRUCT_YAML}"
}
