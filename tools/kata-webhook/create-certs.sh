#! /bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

webhook_dir=$(dirname $0)
source "${webhook_dir}/common.bash"

[ -n "${1:-}" ] && WEBHOOK_NS="$1"
[ -n "${2:-}" ] && WEBHOOK_NAME="$2"

if ! command -v openssl &>/dev/null; then
	echo "ERROR: command 'openssl' not found."
	exit 1
elif ! command -v kubectl &>/dev/null; then
	echo "ERROR: command 'kubectl' not found."
	exit 1
fi

cleanup() {
	rm -rf *.key *.crt *.csr *.srl
	[ -n "${CSR_CONFIG_FILE:-}" ] && rm -f ${CSR_CONFIG_FILE}
}

trap cleanup EXIT

# Create certs for our webhook
touch $HOME/.rnd

# Create a Certificate Signing Request configuration file.
CSR_CONFIG_FILE="$(mktemp)"
cat << EOF >$CSR_CONFIG_FILE
[ req ]
default_bits = 2048
prompt = no
default_md = sha256
req_extensions = req_ext
distinguished_name = dn

[ dn ]
CN = "Kata Containers Webhook"

[ req_ext ]
subjectAltName = @alt_names

[ alt_names ]
DNS.1 = ${WEBHOOK_SVC}.${WEBHOOK_NS}.svc

[ v3_ext ]
authorityKeyIdentifier=keyid,issuer:always
basicConstraints=CA:FALSE
keyUsage=keyEncipherment,dataEncipherment
extendedKeyUsage=serverAuth,clientAuth
subjectAltName=@alt_names
EOF

openssl genrsa -out webhookCA.key 2048
openssl req -x509 -new -nodes -key webhookCA.key \
	-subj "/CN=Kata Containers Webhook" -days 365 -out webhookCA.crt
openssl genrsa -out webhook.key 2048
openssl req -new -key webhook.key -out webhook.csr -config "${CSR_CONFIG_FILE}"
openssl x509 -req -in webhook.csr -CA webhookCA.crt -CAkey webhookCA.key \
	-CAcreateserial -out webhook.crt -days 365 \
	-extensions v3_ext -extfile "${CSR_CONFIG_FILE}"

# Create certs secrets for k8s
kubectl create secret generic \
    ${WEBHOOK_SVC}-certs \
    --from-file=key.pem=./webhook.key \
    --from-file=cert.pem=./webhook.crt \
    --dry-run=client -o yaml > ./deploy/webhook-certs.yaml

# Set the CABundle on the webhook registration
CA_BUNDLE=$(cat ./webhookCA.crt ./webhook.crt | base64 -w0)
sed "s/CA_BUNDLE/${CA_BUNDLE}/" ./deploy/webhook-registration.yaml.tpl > ./deploy/webhook-registration.yaml

