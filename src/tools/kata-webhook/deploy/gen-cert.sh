#! /bin/bash
# Copyright (c) 2022 Ant Group

# TODO use this one:
# https://github.com/kata-containers/tests/blob/dd74f9b8b49d5bfdab5fca92b49d6d9a4eae83cc/kata-webhook/create-certs.sh

set -x

WEBHOOK_NS=${1:-"default"}
WEBHOOK_SVC="kata-webhook"
HOST="${WEBHOOK_SVC}.${WEBHOOK_NS}.svc"

# Create certs for our webhook
openssl genrsa -out webhookCA.key 2048
openssl req -new -key ./webhookCA.key \
    -subj "/CN=${HOST}" \
    -reqexts SAN \
    -config <(cat /etc/ssl/openssl.cnf \
        <(printf "\n[SAN]\nsubjectAltName=DNS:$HOST")) \
    -out ./webhookCA.csr

openssl x509 -req -days 365 -in webhookCA.csr -signkey webhookCA.key \
    -extensions SAN \
    -extfile <(cat /etc/ssl/openssl.cnf <(printf "[SAN]\nsubjectAltName=DNS:$HOST")) \
    -out webhook.crt

# Create certs secrets for k8s
kubectl create secret generic \
    ${WEBHOOK_SVC}-certs \
    --from-file=key.pem=./webhookCA.key \
    --from-file=cert.pem=./webhook.crt \
    --dry-run -o yaml > webhook-certs.yaml

# Set the CABundle on the webhook registration
CA_BUNDLE=$(cat ./webhook.crt | base64 -w0)
sed -i "s/caBundle.*/caBundle: "${CA_BUNDLE}"/" webhook-registration.yaml

# Clean
rm ./webhookCA* && rm ./webhook.crt