#!/bin/bash -e

function script-directory() {
  pushd . > /dev/null
  local script_path="${BASH_SOURCE[0]}"
  if [ -h "${script_path}" ]; then
    while [ -h "${script_path}" ]; do cd "$(dirname "${script_path}")";
    script_path=$(readlink "${script_path}"); done
  fi
  cd "$(dirname "${script_path}")" > /dev/null
  script_path=$(pwd);
  popd  > /dev/null
  echo "${script_path}"
}

RUN_SCRIPT=${1:-"simcloud-unit-test.sh"}

SCRIPT_PATH=$(script-directory)
REPO_PATH=$(builtin cd "$SCRIPT_PATH/../"; pwd)

PR_ID="${GIT_PR_ID:-dev}"
SPEC_ID="${PIPELINE_SPEC_ID:-dev}"
BUILD_NUMBER="${RIO_BUILD_NUMBER:-dev}"
BUILD_ID="${RIO_BUILD_ID:-dev}"
WORKING_PATH="${WORKSPACE:-$REPO_PATH}"
SECRETS_PATH="${BUILD_SECRETS_PATH:-"$HOME/.simcloud"}"

SIMCLOUD_CPU="12"
SIMCLOUD_RAM="16"
SIMCLOUD_DISK="50"
SIMCLOUD_TIMEOUT="60m"

SIMCLOUD_QUOTA_CLASS="p6:iss"
SIMCLOUD_CLUSTER="mr2"

SIMCLOUD_TOKEN_PATH=${SECRETS_PATH}/token

SIMCLOUD_ROOT_PATH="/kata-containers"


echo "Script path: ${SCRIPT_PATH}"
echo "Repo path: ${REPO_PATH}"

echo "Starting Simcloud job with config:"
echo "cpu            : ${SIMCLOUD_CPU}"
echo "memory         : ${SIMCLOUD_RAM}"
echo "disk           : ${SIMCLOUD_DISK}"
echo "job timeout    : ${SIMCLOUD_TIMEOUT}"

TOKEN=$(cat "${SIMCLOUD_TOKEN_PATH}")

SIMCLOUD_COMMAND="simcloud -c ${SIMCLOUD_CLUSTER} -t ${TOKEN}"

cd "${REPO_PATH}"
${SIMCLOUD_COMMAND} quota usage ${SIMCLOUD_QUOTA_CLASS}

JOB_OUTPUT=$(${SIMCLOUD_COMMAND} \
  job post \
  --smi "ubuntu18.04-v1" \
  --owner ${SIMCLOUD_QUOTA_CLASS} \
  --queue-timeout 10m \
  --enable-kvm \
  --description="kata-unit-test#${PR_ID} ${SPEC_ID}#${BUILD_NUMBER}" \
  --memory=${SIMCLOUD_RAM} \
  --cpus=${SIMCLOUD_CPU} \
  --disk=${SIMCLOUD_DISK} \
  --timeout=${SIMCLOUD_TIMEOUT} \
  --files-root ${SIMCLOUD_ROOT_PATH} \
  --files . \
  --command="${SIMCLOUD_ROOT_PATH}/.rio-ci/${RUN_SCRIPT}" \
  --tags="BUILD_ID=${BUILD_ID}" \
  --output="${SIMCLOUD_ROOT_PATH}/logs.tar.gz")

echo "Job submit output: $JOB_OUTPUT"

JOB_OUTPUT=$(echo "$JOB_OUTPUT" | tail -1)
JOB_ID=$(echo "$JOB_OUTPUT" | rev | cut -d' ' -f1 | rev)
echo "Job ID: $JOB_ID"

JOB_STATUS=$(${SIMCLOUD_COMMAND} job info "${JOB_ID}" -f '{{(index .Tasks 0).Status}}')
echo "Job Status: $JOB_STATUS"
while [ "${JOB_STATUS}" -lt 1 ]
do
  sleep 30
  JOB_STATUS=$(${SIMCLOUD_COMMAND} job info "${JOB_ID}" -f '{{(index .Tasks 0).Status}}')
  echo "Job Status: $JOB_STATUS"
done

${SIMCLOUD_COMMAND} job console --live "${JOB_ID}"

JOB_EXIT_CODE=$(${SIMCLOUD_COMMAND} job info "${JOB_ID}" -f '{{(index .Tasks 0).ExitCode}}')
echo "Job ${JOB_ID} completed with exit code: $JOB_EXIT_CODE"

if [ "${JOB_EXIT_CODE}" -gt 0 ]
then
  exit 1
fi

OUTPUT_AVAILABLE=$(${SIMCLOUD_COMMAND} job info "${JOB_ID}" -f '{{(index .Tasks 1).Status}}')
while [ "${OUTPUT_AVAILABLE}" -lt 2 ]
do
  sleep 30
  OUTPUT_AVAILABLE=$(${SIMCLOUD_COMMAND} job info "${JOB_ID}" -f '{{(index .Tasks 1).Status}}')
done

sleep 10

${SIMCLOUD_COMMAND} job download "${JOB_ID}"

if [ ! -f "./${JOB_ID}/${SIMCLOUD_ROOT_PATH}/logs.tar.gz" ]
then
  echo "./${JOB_ID}/${SIMCLOUD_ROOT_PATH}/logs.tar.gz not found!"
  exit 1
fi

mv "${JOB_ID}/${SIMCLOUD_ROOT_PATH}/logs.tar.gz" "${WORKING_PATH}/logs.tar.gz"
