#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Inference on a passthrough GPU with vLLM serving an open, ungated model. The
# NIM equivalents in k8s-nvidia-nim.bats need an NGC API key, so they only run
# on the nightly CI.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-nvidia-gpu-runtime-rs}"

# Mirrors of docker.io/vllm/vllm-openai:v0.11.0 and of the qwen2.5-0.5b-instruct
# and all-minilm-l6-v2 tags of quay.io/redhat-ai-services/modelcar-catalog.
export VLLM_IMAGE="${VLLM_IMAGE:-quay.io/kata-containers/test-images/vllm/vllm-openai:sha256-014a95f21c9edf6abe0aea6b07353f96baa4ec291c427bb1176dc7c93a85845c}"
export MODEL_IMAGE_INSTRUCT="${MODEL_IMAGE_INSTRUCT:-quay.io/kata-containers/test-images/redhat-ai-services/modelcar-catalog:sha256-e20441c23be795838409137576b9016e6bc941eec79e5765cf9de6319c0ec769}"
export MODEL_IMAGE_EMBED="${MODEL_IMAGE_EMBED:-quay.io/kata-containers/test-images/redhat-ai-services/modelcar-catalog:sha256-01ff684fd1f61a1d493ff912660ed5c9f4d860bef35d94f88b6ac0ef46abb604}"

export MODEL_NAME_INSTRUCT="qwen2.5-0.5b-instruct"
export MODEL_NAME_EMBED="all-minilm-l6-v2"

SKIP_MULTI_GPU_TESTS=${SKIP_MULTI_GPU_TESTS:-false}

TEE=false
if is_confidential_gpu_hardware; then
    TEE=true
fi
export TEE

POD_NAME_INSTRUCT="vllm-qwen2-5-0-5b-instruct"
POD_NAME_EMBED="vllm-all-minilm-l6-v2"

# Sandbox boot and image pull get their own budget so that a slow sandbox does
# not eat into the model-load budgets below, which are counted from the moment
# the container starts running and stay under the manifests' startupProbe
# windows.  The vLLM image is ~12GiB compressed, so on a runner that has not
# cached it yet this budget is dominated by the pull.
VLLM_CONTAINER_START_TIMEOUT_PREDEFINED=900s
POD_READY_TIMEOUT_EMBED_PREDEFINED=300s
POD_READY_TIMEOUT_INSTRUCT_PREDEFINED=360s
if [[ "${TEE}" = "true" ]]; then
    POD_NAME_INSTRUCT="${POD_NAME_INSTRUCT}-tee"
    POD_NAME_EMBED="${POD_NAME_EMBED}-tee"
    # Guest-pulling the vLLM image into encrypted storage dominates here.
    VLLM_CONTAINER_START_TIMEOUT_PREDEFINED=1200s
    POD_READY_TIMEOUT_EMBED_PREDEFINED=600s
    POD_READY_TIMEOUT_INSTRUCT_PREDEFINED=600s
fi
export POD_NAME_INSTRUCT
export POD_NAME_EMBED
export VLLM_CONTAINER_START_TIMEOUT=${VLLM_CONTAINER_START_TIMEOUT:-${VLLM_CONTAINER_START_TIMEOUT_PREDEFINED}}
export POD_READY_TIMEOUT_EMBED=${POD_READY_TIMEOUT_EMBED:-${POD_READY_TIMEOUT_EMBED_PREDEFINED}}
export POD_READY_TIMEOUT_INSTRUCT=${POD_READY_TIMEOUT_INSTRUCT:-${POD_READY_TIMEOUT_INSTRUCT_PREDEFINED}}
export SKIP_MULTI_GPU_TESTS

# Static corpus for the retrieval test. The NIM version scraped a live GitHub
# page, which tied the assertions to that page's markup and to network reach.
export RAG_CORPUS_PY='[
    "Kata Containers is an open source container runtime that starts each pod inside a lightweight virtual machine, so workloads get hardware isolation while still being managed like ordinary containers.",
    "The Kata Containers runtime implements the containerd shim v2 interface, which is how Kubernetes selects it through a RuntimeClass.",
    "Kata Containers can run on several hypervisors. The supported ones are QEMU, Cloud Hypervisor, Firecracker, Dragonball and StratoVirt.",
    "Inside every Kata Containers virtual machine there is a process called the kata-agent, which manages containers on behalf of the runtime.",
    "Confidential Containers builds on Kata Containers to run pods inside hardware trusted execution environments such as Intel TDX and AMD SEV-SNP."
]'

# A venv of its own: the NIM suite pins an older langchain into ~/.cicd/venv and
# installs faiss-gpu, which owns the same 'faiss' module as faiss-cpu below.
setup_python_env() {
    ensure_cicd_python_venv "${HOME}/.cicd/venv-vllm"

    pip install --upgrade pip
    # faiss runs here on the runner, whose GPUs are bound to VFIO for
    # passthrough, so the CPU build is the only one that could work.
    pip install \
        'langchain-openai>=0.2,<0.4' \
        'langchain-community>=0.3,<0.4' \
        'faiss-cpu>=1.8'
}

# 'kubectl wait' counts from when it is called, while the startupProbe budget
# only starts once the container is running. Waiting for the container first
# keeps the model-load budget from being spent on sandbox startup.
wait_for_vllm_pod_ready() {
    local pod="$1"
    local ready_timeout="${2%s}"
    local start_timeout="${VLLM_CONTAINER_START_TIMEOUT%s}"
    local since="${SECONDS}"
    local started_at=""

    while ((SECONDS - since < start_timeout)); do
        started_at="$(kubectl get pod "${pod}" -o jsonpath='{.status.containerStatuses[0].state.running.startedAt}' 2>/dev/null || true)"
        [[ -n "${started_at}" ]] && break
        sleep 5
    done
    [[ -n "${started_at}" ]] ||
        die "${pod}: container did not start within ${VLLM_CONTAINER_START_TIMEOUT}"
    echo "# ${pod}: container started after $((SECONDS - since))s" >&3

    since="${SECONDS}"
    local ready="" terminated=""
    while ((SECONDS - since < ready_timeout)); do
        ready="$(kubectl get pod "${pod}" -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}')"
        if [[ "${ready}" = "True" ]]; then
            echo "# ${pod}: ready $((SECONDS - since))s after the container started" >&3
            # Both models are small enough to serve from a CPU, so an inference
            # test passing is not on its own proof of GPU passthrough.  Match on
            # the whole log rather than through a pipe, which pipefail would
            # fail on the SIGPIPE that a matching grep leaves behind.
            [[ "$(kubectl logs "${pod}")" == *"Automatically detected platform cuda"* ]] ||
                die "${pod}: vLLM is not running on a GPU"
            return 0
        fi

        # restartPolicy is Never, so an exit here is final and the rest of the
        # budget would only be spent losing the logs that explain it.
        terminated="$(kubectl get pod "${pod}" -o jsonpath='{.status.containerStatuses[0].state.terminated.reason}')"
        [[ -n "${terminated}" ]] && break
        sleep 10
    done

    echo "# ${pod}: ${terminated:-not ready} $((SECONDS - since))s after the container started" >&3
    echo "# === ${pod} logs ===" >&3
    kubectl logs "${pod}" --all-containers=true >&3 || true
    return 1
}

create_inference_pod() {
    envsubst <"${POD_INSTRUCT_YAML_IN}" >"${POD_INSTRUCT_YAML}"
    auto_generate_policy "${policy_settings_dir}" "${POD_INSTRUCT_YAML}"

    kubectl apply -f "${POD_INSTRUCT_YAML}"
    wait_for_vllm_pod_ready "${POD_NAME_INSTRUCT}" "${POD_READY_TIMEOUT_INSTRUCT}"

    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    POD_IP_INSTRUCT=$(kubectl get pod "${POD_NAME_INSTRUCT}" -o jsonpath='{.status.podIP}')
    [[ -n "${POD_IP_INSTRUCT}" ]]

    echo "POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >"${BATS_SUITE_TMPDIR}/env"
    echo "# POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >&3
}

create_embed_pod() {
    envsubst <"${POD_EMBED_YAML_IN}" >"${POD_EMBED_YAML}"
    auto_generate_policy "${policy_settings_dir}" "${POD_EMBED_YAML}"

    kubectl apply -f "${POD_EMBED_YAML}"
    wait_for_vllm_pod_ready "${POD_NAME_EMBED}" "${POD_READY_TIMEOUT_EMBED}"

    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    POD_IP_EMBED=$(kubectl get pod "${POD_NAME_EMBED}" -o jsonpath='{.status.podIP}')
    [[ -n "${POD_IP_EMBED}" ]]

    echo "POD_IP_EMBED=${POD_IP_EMBED}" >>"${BATS_SUITE_TMPDIR}/env"
    echo "# POD_IP_EMBED=${POD_IP_EMBED}" >&3
}

# On a confidential node images are pulled inside the guest, and the vLLM image
# does not fit in the guest's default scratch space.
create_trusted_storage() {
    local pvc="$1"
    local pv="$2"
    local img="$3"
    local size_mib="$4"
    local template="${pod_config_dir}/confidential/trusted-storage.yaml.in"
    local rendered local_device

    local_device=$(create_loop_device "${img}" "${size_mib}")
    rendered=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${template}").XXX")
    PV_NAME="${pv}" PVC_NAME="${pvc}" \
        PV_STORAGE_CAPACITY="${size_mib}Mi" PVC_STORAGE_REQUEST="${size_mib}Mi" \
        LOCAL_DEVICE="${local_device}" NODE_NAME="${node}" \
        envsubst < "${template}" > "${rendered}"
    retry_kubectl_apply "${rendered}"
}

setup_file() {
    setup_common || die "setup_common failed"

    local instruct_template="vllm-qwen2-5-0-5b-instruct"
    local embed_template="vllm-all-minilm-l6-v2"
    if [[ "${TEE}" = "true" ]]; then
        instruct_template="${instruct_template}-tee"
        embed_template="${embed_template}-tee"

        CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
        export CC_KBS_ADDR
    fi

    export POD_INSTRUCT_YAML_IN="${pod_config_dir}/${instruct_template}.yaml.in"
    export POD_INSTRUCT_YAML="${pod_config_dir}/${instruct_template}.yaml"
    export POD_EMBED_YAML_IN="${pod_config_dir}/${embed_template}.yaml.in"
    export POD_EMBED_YAML="${pod_config_dir}/${embed_template}.yaml"

    dpkg -s jq >/dev/null 2>&1 || sudo apt -y install jq

    setup_python_env

    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
    add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

    # Both pods guest-pull the same ~12GiB compressed vLLM image, so both need
    # room for it once decompressed.
    if [[ "${TEE}" = "true" ]]; then
        create_trusted_storage "trusted-pvc-instruct" "trusted-block-pv-instruct" \
            /tmp/trusted-image-storage-instruct.img 57344

        if [[ "${SKIP_MULTI_GPU_TESTS}" != "true" ]]; then
            create_trusted_storage "trusted-pvc-embed" "trusted-block-pv-embed" \
                /tmp/trusted-image-storage-embed.img 57344
        fi
    fi

    create_inference_pod

    if [[ "${SKIP_MULTI_GPU_TESTS}" != "true" ]]; then
        create_embed_pod
    fi

    # BATS_TEST_COMPLETED is per-test and remains empty in teardown_file.
    # Persist file-level state so success does not trigger journal dumps.
    touch "${BATS_FILE_TMPDIR}/setup-file-completed"
}

teardown() {
    if [[ "${BATS_TEST_COMPLETED:-}" != "1" && -z "${BATS_TEST_SKIPPED:-}" ]]; then
        touch "${BATS_FILE_TMPDIR}/test-failed"
    fi
}

@test "List of models available for inference" {
    # shellcheck disable=SC1091  # File is created by setup_file
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variable is shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]

    # shellcheck disable=SC2031  # Variable is shared via file between BATS tests
    run curl -sX GET "http://${POD_IP_INSTRUCT}:8000/v1/models"
    [[ "${status}" -eq 0 ]]

    MODEL_NAME=$(echo "${output}" | jq -r '.data[0].id')
    [[ "${MODEL_NAME}" = "${MODEL_NAME_INSTRUCT}" ]]

    echo "# MODEL_NAME=${MODEL_NAME}" >&3
}

@test "Simple OpenAI completion request" {
    # shellcheck disable=SC1091  # File is created by setup_file
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]

    QUESTION="What are Kata Containers?"

    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    run curl -sX 'POST' \
        "http://${POD_IP_INSTRUCT}:8000/v1/completions" \
        -H "accept: application/json" \
        -H "Content-Type: application/json" \
        -d "{\"model\": \"${MODEL_NAME_INSTRUCT}\", \"prompt\": \"${QUESTION}\", \"max_tokens\": 64}"
    [[ "${status}" -eq 0 ]]

    ANSWER=$(echo "${output}" | jq -r '.choices[0].text')
    [[ -n "${ANSWER}" && "${ANSWER}" != "null" ]]

    echo "# QUESTION: ${QUESTION}" >&3
    echo "# ANSWER: ${ANSWER}" >&3
}

@test "LangChain OpenAI-compatible chat completion" {
    # shellcheck disable=SC1091  # File is created by setup_file
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]

    QUESTION="What is the capital of France? Answer with just the city name."

    # shellcheck disable=SC2031  # Variables are used in heredoc, not subshell
    cat <<EOF >"${BATS_FILE_TMPDIR}"/langchain_vllm.py
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    base_url="http://${POD_IP_INSTRUCT}:8000/v1",
    api_key="not-used-by-vllm",
    model="${MODEL_NAME_INSTRUCT}",
    temperature=0.1,
    max_tokens=1000,
    top_p=1.0,
)

print(llm.invoke("${QUESTION}").content)
EOF

    run python3 "${BATS_FILE_TMPDIR}"/langchain_vllm.py

    [[ "${status}" -eq 0 ]] || { echo "# ${output}" >&3; false; }
    [[ "${output}" == *"Paris"* ]]

    echo "# QUESTION: ${QUESTION}" >&3
    echo "# ANSWER: ${output}" >&3
}

@test "Kata Documentation RAG" {
    [[ "${SKIP_MULTI_GPU_TESTS}" = "true" ]] && skip "indicated to skip tests requiring multiple GPUs"

    # shellcheck disable=SC1091  # File is created by setup_file
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_EMBED}" ]]
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]

    QUESTION="Which hypervisors can Kata Containers run on?"

    # shellcheck disable=SC2031  # Variables are used in heredoc, not subshell
    cat <<EOF >"${BATS_FILE_TMPDIR}"/langchain_vllm_kata_rag.py
from langchain_community.vectorstores import FAISS
from langchain_openai import ChatOpenAI, OpenAIEmbeddings

documents = ${RAG_CORPUS_PY}

# check_embedding_ctx_length=False keeps LangChain from tokenizing locally and
# sending token arrays, which vLLM's embeddings endpoint does not accept.
embeddings = OpenAIEmbeddings(
    base_url="http://${POD_IP_EMBED}:8000/v1",
    api_key="not-used-by-vllm",
    model="${MODEL_NAME_EMBED}",
    check_embedding_ctx_length=False,
)

store = FAISS.from_texts(documents, embedding=embeddings)

query = "${QUESTION}"
context = "\n".join(doc.page_content for doc in store.as_retriever().invoke(query))

llm = ChatOpenAI(
    base_url="http://${POD_IP_INSTRUCT}:8000/v1",
    api_key="not-used-by-vllm",
    model="${MODEL_NAME_INSTRUCT}",
    temperature=0.1,
    max_tokens=1000,
    top_p=1.0,
)

prompt = (
    "Answer the question using only the context below.\n\n"
    f"Context:\n{context}\n\nQuestion: {query}\nAnswer:"
)
print("#" + llm.invoke(prompt).content.replace("\n", " "))
EOF

    run python3 "${BATS_FILE_TMPDIR}"/langchain_vllm_kata_rag.py
    [[ "${status}" -eq 0 ]] || { echo "# ${output}" >&3; false; }

    ANSWER=$(echo "${output}" | cut -d '#' -f2)
    [[ -n "${ANSWER}" ]]
    # The corpus is static, so the retrieved context always names QEMU.
    [[ "${ANSWER}" == *"QEMU"* || "${ANSWER}" == *"qemu"* ]]

    echo "# QUESTION: ${QUESTION}" >&3
    echo "# ANSWER: ${ANSWER}" >&3
}

teardown_file() {
    echo "=== Instruct Pod Logs ===" >&3
    kubectl logs "${POD_NAME_INSTRUCT}" --all-containers=true >&3 || true

    if [[ "${SKIP_MULTI_GPU_TESTS}" != "true" ]]; then
        echo "=== Embedding Pod Logs ===" >&3
        kubectl logs "${POD_NAME_EMBED}" --all-containers=true >&3 || true
    fi

    if [[ "${TEE}" = "true" ]]; then
        echo "=== KBS Pod Logs ===" >&3
        kbs_k8s_print_logs "${node_start_time:-}" >&3 || true
    fi

    # setup_file may have failed before creating any of what follows, and a
    # teardown_file that dies here takes bats' report of that failure with it.
    if [[ -n "${policy_settings_dir:-}" ]]; then
        delete_tmp_policy_settings_dir "${policy_settings_dir}"
    fi
    kubectl describe pods >&3 || true

    if [[ -f "${POD_INSTRUCT_YAML:-}" ]]; then
        kubectl delete -f "${POD_INSTRUCT_YAML}" --ignore-not-found=true
    fi

    if [[ "${SKIP_MULTI_GPU_TESTS}" != "true" && -f "${POD_EMBED_YAML:-}" ]]; then
        kubectl delete -f "${POD_EMBED_YAML}" --ignore-not-found=true
    fi

    if [[ "${TEE}" = "true" ]]; then
        kubectl delete --ignore-not-found pvc trusted-pvc-instruct trusted-pvc-embed
        kubectl delete --ignore-not-found pv trusted-block-pv-instruct trusted-block-pv-embed
        kubectl delete --ignore-not-found storageclass local-storage
        cleanup_loop_device /tmp/trusted-image-storage-instruct.img || true
        cleanup_loop_device /tmp/trusted-image-storage-embed.img || true
    fi

    local bats_test_completed=1
    if [[ ! -f "${BATS_FILE_TMPDIR}/setup-file-completed" || -f "${BATS_FILE_TMPDIR}/test-failed" ]]; then
        bats_test_completed=
    fi
    # This one reaches the node through a debugger pod and dies if it cannot get
    # one, which would replace bats' report of the real failure with its own.
    (print_node_journal_since_test_start "${node:-}" "${node_start_time:-}" "${bats_test_completed}") >&3 || true
}
