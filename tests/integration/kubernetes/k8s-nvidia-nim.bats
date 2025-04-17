#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

export POD_NAME="nvidia-nim-llama-3-1-8b-instruct"
export DOCKER_CONFIG_JSON=$(
		echo -n "{\"auths\":{\"nvcr.io\":{\"username\":\"\$oauthtoken\",\"password\":\"${NGC_API_KEY}\",\"auth\":\"$(echo -n "\$oauthtoken:${NGC_API_KEY}" | base64 -w0)\"}}}" \
		| base64 -w0
	)

setup() {
	dpkg -s python3-pip 2>&1 >/dev/null || sudo apt -y install python3-pip
	dpkg -s python3-venv 2>&1 >/dev/null || sudo apt -y install python3-venv

	python3 -m venv ${HOME}/.cicd/venv

	get_pod_config_dir

	pod_yaml_in="${pod_config_dir}/pod-nvidia-nim-llama-3.1-8b-instruct.yaml.in"
	pod_yaml="${pod_config_dir}/pod-nvidia-nim-llama-3.1-8b-instruct.yaml"

	envsubst < "${pod_yaml_in}" > "${pod_yaml}"
}


@test "NVIDIA NIM Llama 3.1-8b Instruct" {
	kubectl apply -f "${pod_yaml}"
	kubectl wait --for=condition=Ready --timeout=500s pod "${POD_NAME}"
	export POD_IP=$(kubectl get pod "${POD_NAME}" -o jsonpath='{.status.podIP}')
}

@test "List of models available for inference" {
	export MODEL_NAME=$(curl -sX GET "http://${POD_IP}:8000/v1/models" | jq .data[0].id | tr -d '"')
	echo $MODEL_NAME
}

@test "Simple OpenAI completion request" {
	curl -X 'POST' \
		"http://${POD_IP}:8000/v1/completions" \
		-H "accept: application/json" \
		-H "Content-Type: application/json" \
		-d "{\"model\": \"${MODEL_NAME}\", \"prompt\": \"Once upon a time\", \"max_tokens\": 64}" | jq .choices[0].text
}


@test "Setup the LangChain flow" {
	source ${HOME}/.cicd/venv/bin/activate
	pip install --upgrade pip
	pip install langchain=="0.2.5"
	pip install langchain-nvidia-ai-endpoints=="0.1.2"
	pip install faiss-cpu=="1.10.0"
}

@test "LangChain NVIDIA AI Endpoints" {
	source ${HOME}/.cicd/venv/bin/activate
	cat <<-EOF > ${HOME}/.cicd/venv/langchain_nim.py
		from langchain_nvidia_ai_endpoints import ChatNVIDIA

		llm = ChatNVIDIA(base_url="http://${POD_IP}:8000/v1", model="${MODEL_NAME}", temperature=0.1, max_tokens=1000, top_p=1.0)

		result = llm.invoke("What is the capital of France?")
		print(result.content)
	EOF
	run python3.10 ${HOME}/.cicd/venv/langchain_nim.py

  	[ "$status" -eq 0 ]
  	[ "$output" = "The capital of France is Paris." ]
}

@test "Kata Documentation RAG" {
	source ${HOME}/.cicd/venv/bin/activate
	cat <<EOF > ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
import os
from langchain.chains import ConversationalRetrievalChain, LLMChain
from langchain.chains.conversational_retrieval.prompts import CONDENSE_QUESTION_PROMPT, QA_PROMPT
from langchain.chains.question_answering import load_qa_chain
from langchain.memory import ConversationBufferMemory
from langchain_community.vectorstores import FAISS
from langchain_text_splitters import RecursiveCharacterTextSplitter
from langchain_nvidia_ai_endpoints import ChatNVIDIA
from langchain_nvidia_ai_endpoints import NVIDIAEmbeddings
EOF

	cat <<EOF >> ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
import re
from typing import List, Union

import requests
from bs4 import BeautifulSoup

def html_document_loader(url: Union[str, bytes]) -> str:
	"""
	Loads the HTML content of a document from a given URL and return it's content.

	Args:
		url: The URL of the document.

	Returns:
		The content of the document.

	Raises:
		Exception: If there is an error while making the HTTP request.

	"""
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

	cat <<EOF >> ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
def create_embeddings(embedding_path: str = "./data/nv_embedding"):

	embedding_path = "./data/nv_embedding"
	print(f"Storing embeddings to {embedding_path}")

	# List of web pages containing Kata technical documentation
	urls = [
		"https://katacontainers.io/",
		"https://katacontainers.io/learn/",
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

	cat <<EOF >> ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
def index_docs(url: Union[str, bytes], splitter, documents: List[str], dest_embed_dir) -> None:
	"""
	Split the document into chunks and create embeddings for the document

	Args:
		url: Source url for the document.
		splitter: Splitter used to split the document
		documents: list of documents whose embeddings needs to be created
		dest_embed_dir: destination directory for embeddings

	Returns:
		None
	"""
	embeddings = NVIDIAEmbeddings(model="NV-Embed-QA", truncate="END")

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

	cat <<EOF >> ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
create_embeddings()

embedding_model = NVIDIAEmbeddings(model="NV-Embed-QA", truncate="END")
EOF

	cat <<EOF >> ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
# Embed documents
embedding_path = "./data/nv_embedding"
docsearch = FAISS.load_local(folder_path=embedding_path, embeddings=embedding_model, allow_dangerous_deserialization=True)
EOF

	cat <<EOF >> ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
llm = ChatNVIDIA(base_url="http://${POD_IP}:8000/v1", model="${MODEL_NAME}", temperature=0.1, max_tokens=1000, top_p=1.0)

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

	cat <<EOF >> ${HOME}/.cicd/venv/langchain_nim_kata_rag.py
query = "What is Kata Containers?"
result = qa({"question": query})
print(result.get("answer"))
EOF

	run python3.10 ${HOME}/.cicd/venv/langchain_nim_kata_rag.py

#  	[ "$status" -eq 0 ]
#  	[ "$output" = "The NVIDIA Jetson Nano Developer Kit is a small, powerful computer designed for AI and robotics applications." ]
}
teardown() {
	kubectl describe "pod/$POD_NAME"
	kubectl delete pod "$POD_NAME"
}
