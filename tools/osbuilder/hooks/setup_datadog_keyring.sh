#!/bin/sh
# Fetch the Datadog APT signing key

set -e

curl -fsSL https://keys.datadoghq.com/DATADOG_APT_KEY_CURRENT.public \
    | gpg --dearmor > /tmp/datadog-archive-keyring.gpg
