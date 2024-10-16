#!/bin/bash
set -x

SUPER_REGISTRY_HOST="registry.superprotocol.local"
SUPER_CERT_INITIALIZER_URL="https://ca-subroot1.tee-dev.superprotocol.com:44443"
SUPER_CERTS_DIR="/opt/super/certs"
SUPER_CERT_FILEPATH="${SUPER_CERTS_DIR}/${SUPER_REGISTRY_HOST}"

mkdir -p ${SUPER_CERTS_DIR}

# generate CA & CSR & cert with key
#openssl genrsa -out ${SUPER_CERTS_DIR}/ca.key 2048
#openssl req -x509 -new -nodes -key ${SUPER_CERTS_DIR}/ca.key -sha256 -days 3650 -out ${SUPER_CERTS_DIR}/ca.crt -subj "/ST=Milk Galaxy/L=Planet Earth/O=SuperProtocol/OU=MyUnit/CN=SuperProtocol.com"
#openssl genrsa -out ${SUPER_CERT_FILEPATH}.key 2048
#openssl req -new -key ${SUPER_CERT_FILEPATH}.key -out ${SUPER_CERT_FILEPATH}.csr -subj "/ST=Milk Galaxy/L=Planet Earth/O=SuperProtocol/OU=MyUnit/CN=${SUPER_REGISTRY_HOST}"
#openssl x509 -req -CA ${SUPER_CERTS_DIR}/ca.crt -CAkey ${SUPER_CERTS_DIR}/ca.key -CAcreateserial -in ${SUPER_CERT_FILEPATH}.csr -out ${SUPER_CERT_FILEPATH}.crt -days 3650 -sha256

# copy cert to local trusted store
#cp ${SUPER_CERTS_DIR}/ca.crt /usr/local/share/ca-certificates/ca.crt
#update-ca-certificates --verbose

ca-initializer-linux ${SUPER_CERT_INITIALIZER_URL} /usr/local/share/ca-certificates/superprotocol-ca.crt ${SUPER_REGISTRY_HOST} ${SUPER_CERTS_DIR}
ls -la ${SUPER_CERTS_DIR}

# create kubernetes secret with TLS for docker registry
/var/lib/rancher/rke2/bin/kubectl create secret tls docker-registry-tls --namespace super-protocol --cert=${SUPER_CERT_FILEPATH}.crt --key=${SUPER_CERT_FILEPATH}.key --dry-run=client --output=yaml > /var/lib/rancher/rke2/server/manifests/docker-registry-tls.yaml

set +x
