# Datadog guest overlay (`datadog-files`)

Overlay copied verbatim into the kata **guest** rootfs at build time by the
rootfs builder. It adds three things to the microVM guest, described by use-case
below. (The `datadog-agent` and `apparmor` packages themselves are installed via
[`../ubuntu/config.sh`](../ubuntu/config.sh), not copied from here.)

## Runtime security (system-probe / CWS)

Enables CWS monitoring inside the microVM: a `system-probe` service
(`etc/systemd/system/system-probe.service`, `usr/local/bin/start-system-probe`,
`etc/datadog-agent/system-probe.env`) runs in the guest and communicates with the
host's security-agent over vsock.

## Guest system tuning

Tunes the guest for performance and hardening via kernel sysctls
(`etc/sysctl.d/99-datadog.conf`) and systemd resource limits
(`etc/systemd/system.conf.d/10-rlimits.conf`).

## AppArmor confinement

Two profiles in [`etc/apparmor.d/`](etc/apparmor.d/) confine the guest so
untrusted container workloads can't read the Datadog auth token or mount the
host-shared virtiofs device. They are loaded at boot (`datadog-apparmor.service`)
before the kata-agent starts; the agent runs under one profile and transitions
every process it launches into the other.

- **`usr.bin.kata-agent`** — confines the kata-agent itself. It keeps the broad
  access it needs to set up sandboxes, but is denied the auth token, and every
  program it execs transitions to `kata-container`.

- **`kata-container`** — confines the kata-agent's container-init helper and all
  container processes. Same broad base, additionally denied the auth token and
  denied mounting the host-shared virtiofs device themselves (sandbox-escape
  prevention).

system-probe is intentionally left unconfined — it is the legitimate reader of
the auth token.
