#!/bin/bash
#
# Create rsa private and public keys 
#
CA_KEY=ca.key
CA_PEM=ca.pem

SERVER_KEY=server.key
SERVER_CSR=server.csr
SERVER_PEM=server.pem

CLIENT_KEY=client.key
CLIENT_CSR=client.csr
CLIENT_PEM=client.pem

KEY_SIZE=2048
COUNTRY="AA"
STATE="Default State"
LOCALITY="Default City"
ORG="Default Unit"
ORG_UNIT="Default Unit"
CERT_EXT=cert.ext

#1. Create CA key
openssl genrsa -out ${CA_KEY} ${KEY_SIZE} 

#2. Create self-signed cert valid for ten years
openssl req -x509 -new -nodes -key ${CA_KEY} -sha256 -days 3650 -out ${CA_PEM} \
	 -subj "/C=${COUNTRY}/ST=${STATE}/L=${LOCALITY}/O=${ORG}/OU=${ORG_UNIT}/CN=grpc-tls CA"

#3. Create server key (kata_agent)
openssl genrsa -out ${SERVER_KEY} ${KEY_SIZE}

#4. Create cert request
openssl req -new -sha256 -key ${SERVER_KEY} -out ${SERVER_CSR} \
	 -subj "/C=${COUNTRY}/ST=${STATE}/L=${LOCALITY}/O=${ORG}/OU=${ORG_UNIT}/CN=server" \

# 5. Create server cert
openssl x509 -req -in ${SERVER_CSR} -CA ${CA_PEM} -CAkey ${CA_KEY} -CAcreateserial -out ${SERVER_PEM} -days 3650 -sha256 -extfile ${CERT_EXT} 2> /dev/null

## Repeat steps 3 - 5 for client (tenant)

#3. Create client key
openssl genrsa -out ${CLIENT_KEY} ${KEY_SIZE} 

#4. Create cert request
openssl req -new -sha256 -key ${CLIENT_KEY} -out ${CLIENT_CSR} \
	 -subj "/C=${COUNTRY}/ST=${STATE}/L=${LOCALITY}/O=${ORG}/OU=${ORG_UNIT}/CN=client"

#5. Create cert cert
openssl x509 -req -in ${CLIENT_CSR} -CA ${CA_PEM} -CAkey ${CA_KEY} -CAcreateserial -out ${CLIENT_PEM} -days 3650 -sha256 -extfile ${CERT_EXT} 2> /dev/null
