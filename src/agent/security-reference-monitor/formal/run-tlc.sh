#!/usr/bin/env bash
# FR-15: model-check the SRM lifecycle specification with TLC.
#
# Requires a Java runtime and tla2tools.jar. In CI this can run in any JRE image:
#   docker run --rm -v "$PWD:/w" -w /w eclipse-temurin:21-jre ./run-tlc.sh
#
# tla2tools.jar is fetched next to this script if absent. Deadlock checking is disabled
# on purpose: the model legitimately terminates (all operations reach a terminal state,
# or the monitor quarantines), so a "deadlock" is an expected end state, not a bug. The
# checked properties are the Safety invariant and the QuarantineSticky temporal property.
set -euo pipefail
cd "$(dirname "$0")"

JAR=${TLA2TOOLS_JAR:-tla2tools.jar}
if [[ ! -f "$JAR" ]]; then
  echo "fetching tla2tools.jar..."
  curl -fsSL -o "$JAR" \
    https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar
fi

exec java -cp "$JAR" tlc2.TLC -deadlock -config SRM.cfg SRM.tla
