#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-nvidia-gpu}"

export LOCAL_NIM_CACHE="/opt/nim/.cache"

SKIP_MULTI_GPU_TESTS=${SKIP_MULTI_GPU_TESTS:-false}

TEE=false
if is_confidential_gpu_hardware; then
    TEE=true
fi
export TEE

POD_NAME_EMBEDQA="nvidia-nim-llama-3-2-nv-embedqa-1b-v2"
POD_NAME_INSTRUCT="nvidia-nim-llama-3-1-8b-instruct"
POD_READY_TIMEOUT_EMBEDQA_PREDEFINED=500s
POD_READY_TIMEOUT_INSTRUCT_PREDEFINED=600s
if [[ "${TEE}" = "true" ]]; then
    POD_NAME_EMBEDQA="${POD_NAME_EMBEDQA}-tee"
    POD_NAME_INSTRUCT="${POD_NAME_INSTRUCT}-tee"
    POD_READY_TIMEOUT_EMBEDQA_PREDEFINED=1000s
    POD_READY_TIMEOUT_INSTRUCT_PREDEFINED=1000s
fi
export POD_NAME_EMBEDQA
export POD_NAME_INSTRUCT
export POD_READY_TIMEOUT_EMBEDQA=${POD_READY_TIMEOUT_EMBEDQA:-${POD_READY_TIMEOUT_EMBEDQA_PREDEFINED}}
export POD_READY_TIMEOUT_INSTRUCT=${POD_READY_TIMEOUT_INSTRUCT:-${POD_READY_TIMEOUT_INSTRUCT_PREDEFINED}}
export SKIP_MULTI_GPU_TESTS

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

# Base64 encoding for use as Kubernetes Secret in pod manifests (non-TEE)
NGC_API_KEY_BASE64=$(
    echo -n "${NGC_API_KEY}" | base64 -w0
)
export NGC_API_KEY_BASE64

# Sealed secret format for TEE pods (vault type pointing to KBS resource)
# Format: sealed.<base64url JWS header>.<base64url payload>.<base64url signature>
# IMPORTANT: JWS uses base64url encoding WITHOUT padding (no trailing '=')
# We use tr to convert standard base64 (+/) to base64url (-_) and remove padding (=)
# For vault type, header and signature can be placeholders since the payload
# contains the KBS resource path where the actual secret is stored.
#
# Vault type sealed secret payload for instruct pod:
# {
#   "version": "0.1.0",
#   "type": "vault",
#   "name": "kbs:///default/ngc-api-key/instruct",
#   "provider": "kbs",
#   "provider_settings": {},
#   "annotations": {}
# }
NGC_API_KEY_SEALED_SECRET_INSTRUCT_PAYLOAD=$(
    echo -n '{"version":"0.1.0","type":"vault","name":"kbs:///default/ngc-api-key/instruct","provider":"kbs","provider_settings":{},"annotations":{}}' |
    base64 -w0 | tr '+/' '-_' | tr -d '='
)
NGC_API_KEY_SEALED_SECRET_INSTRUCT="sealed.fakejwsheader.${NGC_API_KEY_SEALED_SECRET_INSTRUCT_PAYLOAD}.fakesignature"
export NGC_API_KEY_SEALED_SECRET_INSTRUCT

# Base64 encode the sealed secret for use in Kubernetes Secret data field
# (genpolicy only supports the 'data' field which expects base64 values)
NGC_API_KEY_SEALED_SECRET_INSTRUCT_BASE64=$(echo -n "${NGC_API_KEY_SEALED_SECRET_INSTRUCT}" | base64 -w0)
export NGC_API_KEY_SEALED_SECRET_INSTRUCT_BASE64

# Vault type sealed secret payload for embedqa pod:
# {
#   "version": "0.1.0",
#   "type": "vault",
#   "name": "kbs:///default/ngc-api-key/embedqa",
#   "provider": "kbs",
#   "provider_settings": {},
#   "annotations": {}
# }
NGC_API_KEY_SEALED_SECRET_EMBEDQA_PAYLOAD=$(
    echo -n '{"version":"0.1.0","type":"vault","name":"kbs:///default/ngc-api-key/embedqa","provider":"kbs","provider_settings":{},"annotations":{}}' |
    base64 -w0 | tr '+/' '-_' | tr -d '='
)
NGC_API_KEY_SEALED_SECRET_EMBEDQA="sealed.fakejwsheader.${NGC_API_KEY_SEALED_SECRET_EMBEDQA_PAYLOAD}.fakesignature"
export NGC_API_KEY_SEALED_SECRET_EMBEDQA

NGC_API_KEY_SEALED_SECRET_EMBEDQA_BASE64=$(echo -n "${NGC_API_KEY_SEALED_SECRET_EMBEDQA}" | base64 -w0)
export NGC_API_KEY_SEALED_SECRET_EMBEDQA_BASE64

setup_langchain_flow() {
    # shellcheck disable=SC1091  # Sourcing virtual environment activation script
    source "${HOME}"/.cicd/venv/bin/activate

    pip install --upgrade pip
    [[ "$(pip show langchain 2>/dev/null | awk '/^Version:/{print $2}')" = "0.2.5" ]] || pip install langchain==0.2.5
    [[ "$(pip show langchain-nvidia-ai-endpoints 2>/dev/null | awk '/^Version:/{print $2}')" = "0.1.2" ]] || pip install langchain-nvidia-ai-endpoints==0.1.2
    [[ "$(pip show faiss-gpu 2>/dev/null | awk '/^Version:/{print $2}')" = "1.7.2" ]] || pip install faiss-gpu==1.7.2
    [[ "$(pip show langchain-community 2>/dev/null | awk '/^Version:/{print $2}')" = "0.2.5" ]] || pip install langchain-community==0.2.5
    [[ "$(pip show beautifulsoup4 2>/dev/null | awk '/^Version:/{print $2}')" = "4.13.4" ]] || pip install beautifulsoup4==4.13.4
}

# Create Docker config for genpolicy so it can authenticate to nvcr.io when
# pulling image manifests (avoids "UnauthorizedError" from genpolicy's registry pull).
# Genpolicy (src/tools/genpolicy) uses docker_credential::get_credential() in
# src/tools/genpolicy/src/registry.rs build_auth(). The docker_credential crate
# reads config from DOCKER_CONFIG (directory) + "/config.json", so we set
# DOCKER_CONFIG to a directory containing config.json with nvcr.io auth.
setup_genpolicy_registry_auth() {
	if [[ -z "${NGC_API_KEY:-}" ]]; then
		return
	fi
	local auth_dir
	auth_dir="${BATS_SUITE_TMPDIR}/.docker-genpolicy"
	mkdir -p "${auth_dir}"
	# Docker config format: auths -> registry -> auth (base64 of "user:password")
	echo -n "{\"auths\":{\"nvcr.io\":{\"username\":\"\$oauthtoken\",\"password\":\"${NGC_API_KEY}\",\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" \
		> "${auth_dir}/config.json"
	export DOCKER_CONFIG="${auth_dir}"
	# REGISTRY_AUTH_FILE (containers-auth.json format) is the same structure for auths
	export REGISTRY_AUTH_FILE="${auth_dir}/config.json"
}

# Create initdata TOML file for genpolicy with CDH configuration.
# This file is used by genpolicy via --initdata-path. Genpolicy will add the
# generated policy.rego to it and set it as the cc_init_data annotation.
# We must overwrite the default empty file AFTER create_tmp_policy_settings_dir()
# copies it to the temp directory.
create_nim_initdata_file() {
    local output_file="$1"
    local cc_kbs_address
    cc_kbs_address=$(kbs_k8s_svc_http_addr)

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
EOF
}

setup_kbs_credentials() {
    # Export KBS address for use in pod YAML templates (aa_kbc_params)
    CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
    export CC_KBS_ADDR

    # Set up Kubernetes secret for the containerd metadata pull
    kubectl delete secret ngc-secret-instruct --ignore-not-found
    kubectl create secret docker-registry ngc-secret-instruct --docker-server="nvcr.io" --docker-username="\$oauthtoken" --docker-password="${NGC_API_KEY}"

    kbs_set_gpu0_resource_policy

    # KBS_AUTH_CONFIG_JSON is already base64 encoded
    kbs_set_resource_base64 "default" "credentials" "nvcr" "${KBS_AUTH_CONFIG_JSON}"

    # Store the actual NGC_API_KEY in KBS for sealed secret unsealing.
    # The sealed secrets in the pod YAML point to these KBS resource paths.
    kbs_set_resource "default" "ngc-api-key" "instruct" "${NGC_API_KEY}"
    kbs_set_resource "default" "ngc-api-key" "embedqa" "${NGC_API_KEY}"
}

create_inference_pod() {
    envsubst <"${POD_INSTRUCT_YAML_IN}" >"${POD_INSTRUCT_YAML}"
    auto_generate_policy "${policy_settings_dir}" "${POD_INSTRUCT_YAML}"

    kubectl apply -f "${POD_INSTRUCT_YAML}"
    kubectl wait --for=condition=Ready --timeout="${POD_READY_TIMEOUT_INSTRUCT}" pod "${POD_NAME_INSTRUCT}"

    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    kubectl get pod "${POD_NAME_INSTRUCT}" -o jsonpath='{.status.podIP}'
    POD_IP_INSTRUCT=$(kubectl get pod "${POD_NAME_INSTRUCT}" -o jsonpath='{.status.podIP}')
    [[ -n "${POD_IP_INSTRUCT}" ]]

    echo "POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >"${BATS_SUITE_TMPDIR}/env"
    echo "# POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >&3
}

create_embedqa_pod() {
    envsubst <"${POD_EMBEDQA_YAML_IN}" >"${POD_EMBEDQA_YAML}"
    auto_generate_policy "${policy_settings_dir}" "${POD_EMBEDQA_YAML}"

    kubectl apply -f "${POD_EMBEDQA_YAML}"
    kubectl wait --for=condition=Ready --timeout="${POD_READY_TIMEOUT_EMBEDQA}" pod "${POD_NAME_EMBEDQA}"

    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    kubectl get pod "${POD_NAME_EMBEDQA}" -o jsonpath='{.status.podIP}'
    POD_IP_EMBEDQA=$(kubectl get pod "${POD_NAME_EMBEDQA}" -o jsonpath='{.status.podIP}')

    [[ -n "${POD_IP_EMBEDQA}" ]]

    echo "POD_IP_EMBEDQA=${POD_IP_EMBEDQA}" >>"${BATS_SUITE_TMPDIR}/env"
    echo "# POD_IP_EMBEDQA=${POD_IP_EMBEDQA}" >&3
}

# With setup_file and teardown_file being used, we use >&3 in some places to direct output to the terminal
setup_file() {
    setup_common || die "setup_common failed"

    export POD_INSTRUCT_YAML_IN="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml.in"
    export POD_INSTRUCT_YAML="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml"
    export POD_EMBEDQA_YAML_IN="${pod_config_dir}/${POD_NAME_EMBEDQA}.yaml.in"
    export POD_EMBEDQA_YAML="${pod_config_dir}/${POD_NAME_EMBEDQA}.yaml"

    dpkg -s jq >/dev/null 2>&1 || sudo apt -y install jq

    export PYENV_ROOT="${HOME}/.pyenv"
    [[ -d ${PYENV_ROOT}/bin ]] && export PATH="${PYENV_ROOT}/bin:${PATH}"
    eval "$(pyenv init - bash)"

    # shellcheck disable=SC1091  # Virtual environment will be created during test execution
    python3 -m venv "${HOME}"/.cicd/venv

    setup_langchain_flow

    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
    add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

    if [ "${TEE}" = "true" ]; then
        # So genpolicy can pull nvcr.io image manifests when generating policy (avoids UnauthorizedError).
        setup_genpolicy_registry_auth

        setup_kbs_credentials
        # Overwrite the empty default-initdata.toml with our CDH configuration.
        # This must happen AFTER create_tmp_policy_settings_dir() copies the empty
        # file and BEFORE auto_generate_policy() runs.
        create_nim_initdata_file "${policy_settings_dir}/default-initdata.toml"
    fi

    create_inference_pod

    if [ "${SKIP_MULTI_GPU_TESTS}" != "true" ]; then
         create_embedqa_pod
    fi
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


@test "LangChain NVIDIA AI Endpoints" {
    # shellcheck disable=SC1091  # File is created by previous test
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${MODEL_NAME}" ]]

    QUESTION="What is the capital of France?"
    ANSWER="The capital of France is Paris."

    # shellcheck disable=SC1091  # Sourcing virtual environment activation script
    source "${HOME}"/.cicd/venv/bin/activate
    # shellcheck disable=SC2031  # Variables are used in heredoc, not subshell
    cat <<EOF >"${HOME}"/.cicd/venv/langchain_nim.py
from langchain_nvidia_ai_endpoints import ChatNVIDIA

llm = ChatNVIDIA(base_url="http://${POD_IP_INSTRUCT}:8000/v1", model="${MODEL_NAME}", temperature=0.1, max_tokens=1000, top_p=1.0)

result = llm.invoke("${QUESTION}")
print(result.content)
EOF

    run python3 "${HOME}"/.cicd/venv/langchain_nim.py

    [[ "${status}" -eq 0 ]]
    [[ "${output}" = "${ANSWER}" ]]

    echo "# QUESTION: ${QUESTION}" >&3
    echo "# ANSWER: ${ANSWER}" >&3
}

@test "Kata Documentation RAG" {
    [ "${SKIP_MULTI_GPU_TESTS}" = "true" ] && skip "indicated to skip tests requiring multiple GPUs"

    # shellcheck disable=SC1091  # File is created by previous test
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_EMBEDQA}" ]]
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${MODEL_NAME}" ]]

    # shellcheck disable=SC1091  # Sourcing virtual environment activation script
    source "${HOME}"/.cicd/venv/bin/activate
    cat <<EOF >"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
import os
from langchain.chains import ConversationalRetrievalChain, LLMChain
from langchain.chains.conversational_retrieval.prompts import CONDENSE_QUESTION_PROMPT, QA_PROMPT
from langchain.chains.question_answering import load_qa_chain
from langchain.memory import ConversationBufferMemory
from langchain_community.vectorstores import FAISS
from langchain.text_splitter import RecursiveCharacterTextSplitter
from langchain_nvidia_ai_endpoints import ChatNVIDIA
from langchain_nvidia_ai_endpoints import NVIDIAEmbeddings
EOF

    # shellcheck disable=SC2129  # Multiple heredocs are intentional for building the Python script
    cat <<EOF >>"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
import re
from typing import List, Union

import requests
from bs4 import BeautifulSoup

def html_document_loader(url: Union[str, bytes]) -> str:
    try:
        response = requests.get(url)
        html_content = response.text
    except Exception as e:
        print(f"Failed to load {url} due to exception {e}")
        return ""

    try:
        # Create a Beautiful Soup object to parse html
        soup = BeautifulSoup(html_content, "html.parser")

        # Remove script and style tags
        for script in soup(["script", "style"]):
            script.extract()

        # Get the plain text from the HTML document
        text = soup.get_text()

        # Remove excess whitespace and newlines
        text = re.sub("\s+", " ", text).strip()

        return text
    except Exception as e:
        print(f"Exception {e} while loading document")
        return ""

EOF

    cat <<EOF >>"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
def create_embeddings(embedding_path: str = "./data/nv_embedding"):

    embedding_path = "./data/nv_embedding"
    print(f"Storing embeddings to {embedding_path}")

    # List of web pages containing Kata technical documentation
    urls = [
        "https://github.com/kata-containers/kata-containers/releases",
    ]

    documents = []
    for url in urls:
        document = html_document_loader(url)
        documents.append(document)


    text_splitter = RecursiveCharacterTextSplitter(
        chunk_size=1000,
        chunk_overlap=0,
        length_function=len,
    )
    texts = text_splitter.create_documents(documents)
    index_docs(url, text_splitter, texts, embedding_path)
    print("Generated embedding successfully")
EOF

    # shellcheck disable=SC2031  # POD_IP_EMBEDQA is shared via file between BATS tests
    cat <<EOF >>"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
def index_docs(url: Union[str, bytes], splitter, documents: List[str], dest_embed_dir) -> None:
    embeddings = NVIDIAEmbeddings(base_url="http://${POD_IP_EMBEDQA}:8000/v1", model="nvidia/llama-3.2-nv-embedqa-1b-v2")

    for document in documents:
        texts = splitter.split_text(document.page_content)

        # metadata to attach to document
        metadatas = [document.metadata]

        # create embeddings and add to vector store
        if os.path.exists(dest_embed_dir):
            update = FAISS.load_local(folder_path=dest_embed_dir, embeddings=embeddings, allow_dangerous_deserialization=True)
            update.add_texts(texts, metadatas=metadatas)
            update.save_local(folder_path=dest_embed_dir)
        else:
            docsearch = FAISS.from_texts(texts, embedding=embeddings, metadatas=metadatas)
            docsearch.save_local(folder_path=dest_embed_dir)
EOF

    # shellcheck disable=SC2031  # POD_IP_EMBEDQA is shared via file between BATS tests
    cat <<EOF >>"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
create_embeddings()

embedding_model = NVIDIAEmbeddings(base_url="http://${POD_IP_EMBEDQA}:8000/v1", model="nvidia/llama-3.2-nv-embedqa-1b-v2")
EOF

    cat <<EOF >>"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
# Embed documents
embedding_path = "./data/nv_embedding"
docsearch = FAISS.load_local(folder_path=embedding_path, embeddings=embedding_model, allow_dangerous_deserialization=True)
EOF

    # shellcheck disable=SC2031  # Variables are used in heredoc, not subshell
    cat <<EOF >>"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
llm = ChatNVIDIA(base_url="http://${POD_IP_INSTRUCT}:8000/v1", model="${MODEL_NAME}", temperature=0.1, max_tokens=1000, top_p=1.0)

memory = ConversationBufferMemory(memory_key="chat_history", return_messages=True)

qa_prompt=QA_PROMPT

doc_chain = load_qa_chain(llm, chain_type="stuff", prompt=QA_PROMPT)

qa = ConversationalRetrievalChain.from_llm(
    llm=llm,
    retriever=docsearch.as_retriever(),
    chain_type="stuff",
    memory=memory,
    combine_docs_chain_kwargs={'prompt': qa_prompt},
)

EOF

    QUESTION="What is the latest Kata Containers release?"

    cat <<EOF >>"${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
query = "${QUESTION}"
result = qa.invoke({"question": query})
print("#"+ result.get("answer"))

EOF

    run python3 "${HOME}"/.cicd/venv/langchain_nim_kata_rag.py
    [[ "${status}" -eq 0 ]]

    ANSWER=$(echo "${output}" | cut -d '#' -f2)
    [[ -n "${ANSWER}" ]]

    echo "# QUESTION: ${QUESTION}" >&3
    echo "# ANSWER: ${ANSWER}" >&3
}

teardown_file() {
    # Debugging information
    echo "=== Instruct Pod Logs ===" >&3
    kubectl logs "${POD_NAME_INSTRUCT}"  >&3 || true

    if [ "${SKIP_MULTI_GPU_TESTS}" != "true" ]; then
        echo "=== EmbedQA Pod Logs ===" >&3
        kubectl logs "${POD_NAME_EMBEDQA}" >&3 || true
    fi

    if [[ "${TEE}" = "true" ]]; then
        echo "=== KBS Pod Logs ===" >&3
        kbs_k8s_print_logs "${node_start_time}" >&3
    fi

    delete_tmp_policy_settings_dir "${policy_settings_dir}"
    kubectl describe pods >&3

    # Clean up resources (manifests contain both secrets and pods)
    [ -f "${POD_INSTRUCT_YAML}" ] && kubectl delete -f "${POD_INSTRUCT_YAML}" --ignore-not-found=true

    if [ "${SKIP_MULTI_GPU_TESTS}" != "true" ]; then
        [ -f "${POD_EMBEDQA_YAML}" ] && kubectl delete -f "${POD_EMBEDQA_YAML}" --ignore-not-found=true
    fi

    print_node_journal_since_test_start "${node}" "${node_start_time:-}" "${BATS_TEST_COMPLETED:-}" >&3
}
