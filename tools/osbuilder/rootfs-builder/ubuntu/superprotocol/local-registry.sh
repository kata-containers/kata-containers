#!/bin/bash
SUPER_REGISTRY_HOST="registry.superprotocol.local"
SUPER_CERTS_DIR="/opt/super/certs"
SUPER_CERT_FILEPATH="${SUPER_CERTS_DIR}/${SUPER_REGISTRY_HOST}"

pkill hauler
sleep 3
mkdir -p /opt/hauler/.hauler
/usr/local/bin/hauler store load --store /opt/hauler/store /etc/super/opt/hauler/*.zst
nohup /usr/local/bin/hauler store serve fileserver --store /opt/hauler/store --directory /opt/hauler/registry &
/usr/local/bin/hauler store serve registry --store /opt/hauler/store --directory /opt/hauler/registry --tls-cert=${SUPER_CERT_FILEPATH}.bundle.crt --tls-key=${SUPER_CERT_FILEPATH}.key
