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

RUNTIME_CLASS_NAME=${RUNTIME_CLASS_NAME:-kata-qemu-nvidia-gpu}
export RUNTIME_CLASS_NAME

export POD_NAME_INSTRUCT="nvidia-nim-llama-3-1-8b-instruct"
export POD_NAME_EMBEDQA="nvidia-nim-llama-3-2-nv-embedqa-1b-v2"

export LOCAL_NIM_CACHE="/opt/nim/.cache"

DOCKER_CONFIG_JSON=$(
    echo -n "{\"auths\":{\"nvcr.io\":{\"username\":\"\$oauthtoken\",\"password\":\"${NGC_API_KEY}\",\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" |
        base64 -w0
)
export DOCKER_CONFIG_JSON

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

create_inference_embedqa_pods() {
    kubectl apply -f "${POD_INSTRUCT_YAML}"
    kubectl apply -f "${POD_EMBEDQA_YAML}"

    kubectl wait --for=condition=Ready --timeout=500s pod "${POD_NAME_INSTRUCT}"
    kubectl wait --for=condition=Ready --timeout=500s pod "${POD_NAME_EMBEDQA}"

    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    POD_IP_INSTRUCT=$(kubectl get pod "${POD_NAME_INSTRUCT}" -o jsonpath='{.status.podIP}')
    [[ -n "${POD_IP_INSTRUCT}" ]]

    # shellcheck disable=SC2030  # Variable is shared via file between BATS tests
    POD_IP_EMBEDQA=$(kubectl get pod "${POD_NAME_EMBEDQA}" -o jsonpath='{.status.podIP}')
    [[ -n "${POD_IP_EMBEDQA}" ]]

    echo "POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >"${BATS_SUITE_TMPDIR}/env"
    echo "# POD_IP_INSTRUCT=${POD_IP_INSTRUCT}" >&3

    echo "POD_IP_EMBEDQA=${POD_IP_EMBEDQA}" >>"${BATS_SUITE_TMPDIR}/env"
    echo "# POD_IP_EMBEDQA=${POD_IP_EMBEDQA}" >&3
}

enable_nvrc_trace() {
    local config_file=""
    if [[ ${RUNTIME_CLASS_NAME} == "kata-qemu-nvidia-gpu" ]]; then
        config_file="/opt/kata/share/defaults/kata-containers/configuration-qemu-nvidia-gpu.toml"
    elif [[ ${RUNTIME_CLASS_NAME} == "kata-qemu-nvidia-gpu-snp" ]]; then
        config_file="/opt/kata/share/defaults/kata-containers/configuration-qemu-nvidia-gpu-snp.toml"
    fi
    sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 nvrc.log=trace"/g' "${config_file}"
}

setup_file() {
    dpkg -s jq >/dev/null 2>&1 || sudo apt -y install jq

    export PYENV_ROOT="${HOME}/.pyenv"
    [[ -d ${PYENV_ROOT}/bin ]] && export PATH="${PYENV_ROOT}/bin:${PATH}"
    eval "$(pyenv init - bash)"

    # shellcheck disable=SC1091  # Virtual environment will be created during test execution
    python3 -m venv "${HOME}"/.cicd/venv

    get_pod_config_dir

    pod_instruct_yaml_in="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml.in"
    pod_instruct_yaml="${pod_config_dir}/${POD_NAME_INSTRUCT}.yaml"

    pod_embedqa_yaml_in="${pod_config_dir}/${POD_NAME_EMBEDQA}.yaml.in"
    pod_embedqa_yaml="${pod_config_dir}/${POD_NAME_EMBEDQA}.yaml"

    envsubst <"${pod_instruct_yaml_in}" >"${pod_instruct_yaml}"
    envsubst <"${pod_embedqa_yaml_in}" >"${pod_embedqa_yaml}"

    export POD_INSTRUCT_YAML="${pod_instruct_yaml}"
    export POD_EMBEDQA_YAML="${pod_embedqa_yaml}"

    enable_nvrc_trace

    setup_langchain_flow
    create_inference_embedqa_pods
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
    # shellcheck disable=SC1091  # File is created by previous test
    source "${BATS_SUITE_TMPDIR}/env"
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_EMBEDQA}" ]]
    # shellcheck disable=SC2031  # Variables are shared via file between BATS tests
    [[ -n "${POD_IP_INSTRUCT}" ]]

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
llm = ChatNVIDIA(base_url="http://${POD_IP_INSTRUCT}:8000/v1", model="meta/llama3-8b-instruct", temperature=0.1, max_tokens=1000, top_p=1.0)

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
        kubectl delete -f "${POD_INSTRUCT_YAML}"
        kubectl delete -f "${POD_EMBEDQA_YAML}"
}
