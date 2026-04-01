#!/bin/bash -e
#
# Copyright (c) 2025 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# Setup peer-pods using cloud-api-adaptor on azure
#
# WARNING: When running outside "eastus" region this script creates a new
#          resource group in "eastus" region and peers the network. You
#          have to remove these manually (or use temporary accounts)

SCRIPT_DIR=$(dirname "$0")

##################
# Helper functions
##################

# Sparse "git clone" supporting old git version
# $1  - origin
# $2  - revision
# $3- - sparse checkout paths
# Note: uses pushd to change into the clonned directory!
git_sparse_clone() {
  local origin="$1"
  local revision="$2"
  shift 2
  local sparse_paths=("$@")

  local repo
  repo=$(basename -s .git "${origin}")

  git init "${repo}"
  pushd "${repo}" || exit 1
  git remote add origin "${origin}"
  git fetch --depth 1 origin "${revision}"
  git sparse-checkout init --cone
  git sparse-checkout set "${sparse_paths[@]}"
  git checkout FETCH_HEAD
}

#######################
# Install prerequisites
#######################
if ! command -v helm &>/dev/null; then
	echo "Helm not installed, installing in current location..."
	PATH="${PWD}:${PATH}"
	curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | HELM_INSTALL_DIR='.' bash -s -- --no-sudo
fi
if ! command -v yq &>/dev/null; then
	echo "yq not installed, installing in current location..."
	PATH="${PWD}:${PATH}"
	curl -fsSL https://github.com/mikefarah/yq/releases/latest/download/yq_linux_amd64 -o ./yq
	chmod +x yq
fi

###############################
# Disable security to allow e2e
###############################

# Disable security
oc adm policy add-scc-to-group privileged system:authenticated system:serviceaccounts
oc adm policy add-scc-to-group anyuid system:authenticated system:serviceaccounts
oc label --overwrite ns default pod-security.kubernetes.io/enforce=privileged pod-security.kubernetes.io/warn=baseline pod-security.kubernetes.io/audit=baseline


####################################
# Get basic credentials from cluster
####################################

oc -n kube-system get secret azure-credentials -o json > azure_credentials.json
AZURE_CLIENT_ID="$(jq -r .data.azure_client_id azure_credentials.json|base64 -d)"
AZURE_CLIENT_SECRET="$(jq -r .data.azure_client_secret azure_credentials.json|base64 -d)"
AZURE_TENANT_ID="$(jq -r .data.azure_tenant_id azure_credentials.json|base64 -d)"
AZURE_SUBSCRIPTION_ID="$(jq -r .data.azure_subscription_id azure_credentials.json|base64 -d)"
rm -f azure_credentials.json
AZURE_RESOURCE_GROUP=$(oc get infrastructure/cluster -o jsonpath='{.status.platformStatus.azure.resourceGroupName}')
az login --service-principal -u "${AZURE_CLIENT_ID}" -p "${AZURE_CLIENT_SECRET}" --tenant "${AZURE_TENANT_ID}"
# Recommended on az sites to refresh the subscription
az account set --subscription "${AZURE_SUBSCRIPTION_ID}"
# This command still sometimes fails directly after login
for I in {1..30}; do
	AZURE_VNET_NAME=$(az network vnet list --resource-group "${AZURE_RESOURCE_GROUP}" --query "[].{Name:name}" --output tsv ||:)
	if [[ -z "${AZURE_VNET_NAME}" ]]; then
		sleep "${I}"
	else	# VNET set, we are done
		break
	fi
done
if [[ -z "${AZURE_VNET_NAME}" ]]; then
	echo "Failed to get AZURE_VNET_NAME in 30 iterations"
	exit 1
fi
AZURE_SUBNET_NAME=$(az network vnet subnet list --resource-group "${AZURE_RESOURCE_GROUP}" --vnet-name "${AZURE_VNET_NAME}" --query "[].{Id:name} | [? contains(Id, 'worker')]" --output tsv)
AZURE_SUBNET_ID=$(az network vnet subnet list --resource-group "${AZURE_RESOURCE_GROUP}" --vnet-name "${AZURE_VNET_NAME}" --query "[].{Id:id} | [? contains(Id, 'worker')]" --output tsv)
AZURE_REGION=$(az group show --resource-group "${AZURE_RESOURCE_GROUP}" --query "{Location:location}" --output tsv)

# Create workload identity
AZURE_WORKLOAD_IDENTITY_NAME="caa-${AZURE_CLIENT_ID}"
az identity create   --name "${AZURE_WORKLOAD_IDENTITY_NAME}"   --resource-group "${AZURE_RESOURCE_GROUP}"   --location "${AZURE_REGION}"


#############################
# Ensure we can run in eastus
#############################

PP_REGION=eastus
if [[ "${AZURE_REGION}" == "${PP_REGION}" ]]; then
    echo "Using the current region ${AZURE_REGION}"
    PEERING=0
    PP_RESOURCE_GROUP="${AZURE_RESOURCE_GROUP}"
    PP_VNET_NAME="${AZURE_VNET_NAME}"
    PP_SUBNET_NAME="${AZURE_SUBNET_NAME}"
    PP_SUBNET_ID="${AZURE_SUBNET_ID}"
else
    echo "Creating peering between ${AZURE_REGION} and ${PP_REGION}"
    PEERING=1
    PP_RESOURCE_GROUP="${AZURE_RESOURCE_GROUP}-eastus"
    PP_VNET_NAME="${AZURE_VNET_NAME}-eastus"
    PP_SUBNET_NAME="${AZURE_SUBNET_NAME}-eastus"
    PP_NSG_NAME="${AZURE_VNET_NAME}-nsg-eastus"
    echo "  creating new PP_RESOURCE_GROUP=${PP_RESOURCE_GROUP}"
    az group create --name "${PP_RESOURCE_GROUP}" --location "${PP_REGION}"
    az network vnet create --resource-group "${PP_RESOURCE_GROUP}" --name "${PP_VNET_NAME}" --location "${PP_REGION}" --address-prefixes 10.2.0.0/16 --subnet-name "${PP_SUBNET_NAME}" --subnet-prefixes 10.2.1.0/24
    az network nsg create --resource-group "${PP_RESOURCE_GROUP}" --name "${PP_NSG_NAME}" --location "${PP_REGION}"
    az network vnet subnet update --resource-group "${PP_RESOURCE_GROUP}" --vnet-name "${PP_VNET_NAME}" --name "${PP_SUBNET_NAME}" --network-security-group "${PP_NSG_NAME}"
    AZURE_VNET_ID=$(az network vnet show --resource-group "${AZURE_RESOURCE_GROUP}" --name "${AZURE_VNET_NAME}" --query id --output tsv)
    PP_VNET_ID=$(az network vnet show --resource-group "${PP_RESOURCE_GROUP}" --name "${PP_VNET_NAME}" --query id --output tsv)
    az network vnet peering create --name westus-to-eastus --resource-group "${AZURE_RESOURCE_GROUP}" --vnet-name "${AZURE_VNET_NAME}" --remote-vnet "${PP_VNET_ID}" --allow-vnet-access
    az network vnet peering create --name eastus-to-westus --resource-group "${PP_RESOURCE_GROUP}" --vnet-name "${PP_VNET_NAME}" --remote-vnet "${AZURE_VNET_ID}" --allow-vnet-access
    PP_SUBNET_ID=$(az network vnet subnet list --resource-group "${PP_RESOURCE_GROUP}" --vnet-name "${PP_VNET_NAME}" --query "[].{Id:id} | [? contains(Id, 'worker')]" --output tsv)
fi

# Peer-pod requires gateway
az network public-ip create \
  --resource-group "${PP_RESOURCE_GROUP}" \
  --name MyPublicIP \
  --sku Standard \
  --allocation-method Static
az network nat gateway create \
  --resource-group "${PP_RESOURCE_GROUP}" \
  --name MyNatGateway \
  --public-ip-addresses MyPublicIP \
  --idle-timeout 10
az network vnet subnet update \
  --resource-group "${PP_RESOURCE_GROUP}" \
  --vnet-name "${PP_VNET_NAME}" \
  --name "${PP_SUBNET_NAME}" \
  --nat-gateway MyNatGateway


##########################################
# Setup CAA
#########################################

# Label the nodes
for NODE_NAME in $(kubectl get nodes -o jsonpath='{.items[*].metadata.name}'); do [[ "${NODE_NAME}" =~ 'worker' ]] && kubectl label node "${NODE_NAME}" node.kubernetes.io/worker=; done

# CAA artifacts
if [[ -z "${CAA_TAG}" ]]; then
	if [[ -n "${CAA_IMAGE}" ]]; then
		echo "CAA_IMAGE (${CAA_IMAGE}) is set but CAA_TAG isn't, which is not supported. Please specify both or none"
		exit 1
	fi
	TAGS="$(curl https://quay.io/api/v1/repository/confidential-containers/cloud-api-adaptor/tag/?onlyActiveTags=true)"
	DIGEST=$(echo "${TAGS}" | jq -r '.tags[] | select(.name | contains("latest-amd64")) | .manifest_digest')
	CAA_TAG="$(echo "${TAGS}" | jq -r '.tags[] | select(.manifest_digest | contains("'"${DIGEST}"'")) | .name' | grep -v "latest")"
fi
if [[ -z "${CAA_IMAGE}" ]]; then
	CAA_IMAGE="quay.io/confidential-containers/cloud-api-adaptor"
fi

# Get latest PP image
#
# You can list the CI images by:
#     az sig image-version list-community --location "eastus" --public-gallery-name "cocopodvm-d0e4f35f-5530-4b9c-8596-112487cdea85" --gallery-image-definition "podvm_image0" --output table
# or the release images by:
#     az sig image-version list-community --location "eastus" --public-gallery-name "cococommunity-42d8482d-92cd-415b-b332-7648bd978eff" --gallery-image-definition "peerpod-podvm-fedora" --output table
# or the release debug images by:
#     az sig image-version list-community --location "eastus" --public-gallery-name "cococommunity-42d8482d-92cd-415b-b332-7648bd978eff" --gallery-image-definition "peerpod-podvm-fedora-debug" --output table
#
# Note there are other flavours of the released images, you can list them by:
#     az sig image-definition list-community --location "eastus" --public-gallery-name "cococommunity-42d8482d-92cd-415b-b332-7648bd978eff" --output table
if [[ -z "${PP_IMAGE_ID}" ]]; then
	SUCCESS_TIME=$(curl -s \
	  -H "Accept: application/vnd.github+json" \
	  "https://api.github.com/repos/confidential-containers/cloud-api-adaptor/actions/workflows/azure-nightly-build.yml/runs?status=success" \
	  | jq -r '.workflow_runs[0].updated_at')
	PP_IMAGE_ID="/CommunityGalleries/cocopodvm-d0e4f35f-5530-4b9c-8596-112487cdea85/Images/podvm_image0/Versions/$(date -u -jf "%Y-%m-%dT%H:%M:%SZ" "${SUCCESS_TIME}" "+%Y.%m.%d" 2>/dev/null || date -d "${SUCCESS_TIME}" +%Y.%m.%d)"
fi

echo "AZURE_REGION=\"${AZURE_REGION}\""
echo "PP_REGION=\"${PP_REGION}\""
echo "AZURE_RESOURCE_GROUP=\"${AZURE_RESOURCE_GROUP}\""
echo "PP_RESOURCE_GROUP=\"${PP_RESOURCE_GROUP}\""
echo "PP_SUBNET_ID=\"${PP_SUBNET_ID}\""
echo "CAA_IMAGE=\"${CAA_IMAGE}\""
echo "CAA_TAG=\"${CAA_TAG}\""
echo "PP_IMAGE_ID=\"${PP_IMAGE_ID}\""

# Install cert-manager (prerequisit)
helm install cert-manager oci://quay.io/jetstack/charts/cert-manager --namespace cert-manager --create-namespace --set crds.enabled=true

# Clone and configure caa
git_sparse_clone "https://github.com/confidential-containers/cloud-api-adaptor.git" "${CAA_GIT_SHA:-main}" "src/cloud-api-adaptor/install/charts/" "src/peerpod-ctrl/chart" "src/webhook/chart"
echo "CAA_GIT_SHA=\"$(git rev-parse HEAD)\""
pushd src/cloud-api-adaptor/install/charts/peerpods
# Use the latest kata-deploy
yq -i '( .dependencies[] | select(.name == "kata-deploy") ) .version = "0.0.0-dev"' Chart.yaml
helm dependency update .
# Create secrets
kubectl apply -f - << EOF
apiVersion: v1
kind: Namespace
metadata:
    name: confidential-containers-system
    labels:
        app.kubernetes.io/managed-by: Helm
    annotations:
        meta.helm.sh/release-name: peerpods
        meta.helm.sh/release-namespace: confidential-containers-system
EOF
kubectl create secret generic my-provider-creds \
       -n confidential-containers-system \
       --from-literal=AZURE_CLIENT_ID="$AZURE_CLIENT_ID" \
       --from-literal=AZURE_CLIENT_SECRET="$AZURE_CLIENT_SECRET" \
       --from-literal=AZURE_TENANT_ID="$AZURE_TENANT_ID"
helm install peerpods . -f providers/azure.yaml --set secrets.mode=reference --set secrets.existingSecretName=my-provider-creds --set providerConfigs.azure.AZURE_SUBSCRIPTION_ID="${AZURE_SUBSCRIPTION_ID}" --set providerConfigs.azure.AZURE_REGION="${PP_REGION}" --set providerConfigs.azure.AZURE_INSTANCE_SIZE="Standard_D2as_v5" --set providerConfigs.azure.AZURE_RESOURCE_GROUP="${PP_RESOURCE_GROUP}" --set providerConfigs.azure.AZURE_SUBNET_ID="${PP_SUBNET_ID}" --set providerConfigs.azure.AZURE_IMAGE_ID="${PP_IMAGE_ID}" --set providerConfigs.azure.DISABLECVM="true" --set providerConfigs.azure.PEERPODS_LIMIT_PER_NODE="50" --set kata-deploy.snapshotter.setup= --dependency-update -n confidential-containers-system --create-namespace --wait
popd	# charts
popd	# git_sparse_clone CAA

# Wait for runtimeclass
SECONDS=0
( while [[ "${SECONDS}" -lt 360 ]]; do
    kubectl get runtimeclass | grep -q kata-remote && exit 0
done; exit 1 ) || { echo "kata-remote runtimeclass not initialized in 60s"; kubectl -n confidential-containers-system get all; echo; echo "kubectl -n confidential-containers-system describe all"; kubectl -n confidential-containers-system describe all; echo; echo CAA; kubectl -n confidential-containers-system logs daemonset.apps/cloud-api-adaptor-daemonset; echo pre-install; kubectl -n confidential-containers-system logs daemonset.apps/cc-operator-pre-install-daemon; echo install; kubectl -n confidential-containers-system logs daemonset.apps/cc-operator-daemon-install; exit 1; }


################
# Deploy webhook
################
pushd "${SCRIPT_DIR}/cluster/"
kubectl create ns default || true
kubectl config set-context --current --namespace=default
KATA_RUNTIME=kata-remote ./deploy_webhook.sh
popd


##################################
# Log warning when peering created
##################################
if [[ ${PEERING} -ne 0 ]]; then
    echo "This script created additional resources to create peering between ${AZURE_REGION} and ${PP_REGION}. Ensure you release those resources after the testing (or use temporary subscription)"
    PP_VARS=("PP_RESOURCE_GROUP" "PP_VNET_NAME" "PP_SUBNET_NAME" "PP_NSG_NAME" "AZURE_VNET_ID" "PP_VNET_ID" "PP_SUBNET_ID")
    for PP_VAR in "${PP_VARS[@]}"; do
        echo "${PP_VAR}=${!PP_VAR}"
    done
    echo
    echo "by running 'az group delete --name ${PP_RESOURCE_GROUP}'"
fi
