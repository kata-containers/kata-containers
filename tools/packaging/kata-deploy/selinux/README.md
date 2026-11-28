# kata-deploy and kata-monitor SELinux policy

`kata-deploy.cil` gives each kata-deploy installer stage its own least-privilege
SELinux domain, so the installer can do its host work on an SELinux-enforcing
node without running as `spc_t`. It also provides `kata_monitor_t` for the
monitor to reach shim management sockets across sandbox MCS categories.

## Why it exists

The installer's containers run with `privileged: false`, which means they run as
`container_t` and are denied the host operations they need — writing `/opt/kata`,
writing the CRI drop-in, and driving systemd. See
[issue #13751](https://github.com/kata-containers/kata-containers/issues/13751).

Scope is the **installer and monitor**. The module defines no `filecon` rules and
relabels nothing, so `/opt/kata` keeps the labels it has today (`usr_t`,
inherited from `/opt`) and the Kata runtime sees exactly what it saw before.

## Domains

| Domain | Used by | Can |
| --- | --- | --- |
| `kata_deploy_check_t` | `host-check` | Read only, plus query the CRI unit's status over D-Bus |
| `kata_deploy_artifacts_t` | `artifacts`, `remove-artifacts` | Write `/opt/kata`, the nydus unit, `/etc/modules-load.d`, and remove the rootless udev rule |
| `kata_deploy_cri_t` | `cri`, `revert-cri` | Write the CRI config, manage the nydus unit, restart the CRI |
| `kata_deploy_node_binaries_t` | `node-binaries-install`, `node-binaries-remove` | Write `/usr/local/bin` |
| `kata_deploy_t` | the `daemonset` mode's single container | All of the above except `/usr/local/bin` |
| `kata_monitor_t` | `kata-monitor` | Watch runtime directories and connect to shim management and CRI sockets across sandbox MCS categories |

Every domain but `kata_deploy_check_t` also takes the node mutation lock at
`/run/lock/kata-deploy.lock`.

The `job` mode runs each stage in its own container, so each gets the narrowest
domain its own work needs. The `daemonset` mode runs the whole install in one
container, so it needs the union — minus `kata_deploy_node_binaries_t`, because
`nodeBinaries` requires `job` mode and so can never happen there.

`kata_deploy_cri_t` writing `/opt/kata` is not a slip: `revert-cri` removes the
nydus binaries from under it, so the CRI domain and the artifacts domain overlap
there by necessity.

Some containers deliberately get **no** domain from this module:

- the `load-kernel-modules` stage, which is privileged and already runs as `spc_t`;
- the `rootless-devices` stage, privileged for the same reason: it creates host
  groups and reconciles device nodes;
- the `nodeBinaries` *staging* containers, one per entry, which only write a
  pod-local `emptyDir`. They are the containers in the pipeline running images
  Kata does not build, and plain `container_t` is both sufficient for them and
  the right blast radius.

Reading host binaries needs no rule: `/usr/local/bin` is `bin_t`, a
`base_ro_file_type`, which the base policy already lets any domain read. That is
what makes `host-check`'s `mkfs.erofs` probe work. Only writing it needs granting.

All five domains join the same `container_domain` attributes as `container_t`,
inheriting container-selinux's baseline; the rules here are only the *delta* that
`container_t` lacked.

### Monitor access across sandbox categories

With both `selinux.enabled` and `monitor.enabled`, the Helm chart defaults the
monitor to `kata_monitor_t:s0`, keeping `privileged: false`. Explicit settings in
`monitor.securityContext.seLinuxOptions` override these defaults. With
`selinux.enabled: false`, the chart leaves the monitor's SELinux settings alone;
an enforcing deployment then needs an administrator-provided policy and domain.

The monitor inherits the container baseline but deliberately does not join
`mcs_constrained_type`: a single monitor must reach sandboxes with different
categories. Its additional type permissions allow reading and watching
`container_var_run_t` directories, writing socket files of that type to connect,
and connecting to `container_runtime_t` Unix stream sockets. SELinux permissions
are granted by type, so these cover other runtime endpoints with the same labels;
they do not distinguish a shim management socket from the CRI API socket.
The chart limits the mounted host paths and mounts them read-only. Read-only
mounts do not prevent socket API operations.

The monitor does not join the installer domains and receives none of their host
installation permissions. Sandbox directory labels and VMM MCS confinement stay
in place. Host administration from an unconfined domain continues to work.

For the standalone monitor manifest, load this module on each enforcing node and
set the monitor container's `securityContext.seLinuxOptions` to
`type: kata_monitor_t`, `level: s0`.

Validate on an enforcing host with two sandboxes using different MCS category
pairs. Confirm their directory and VMM labels with `ls -Zd /run/kata/*` and
`ps -eZ`. Request `/metrics?sandbox=<sandbox-id>` from the monitor for each
sandbox and check for guest or agent metrics, rather than only the monitor's
own metrics. Confirm it discovers both directories without watch denials, and
inspect `ausearch -m AVC,USER_AVC -ts recent -i` for monitor denials. A monitor
in ordinary `container_t` with unrelated categories provides a negative control;
host `kata-ctl monitor` provides an administrative control.

## Directories the chart can move

`env.installationPrefix` and `containerd.configDir` put the install directory and
the CRI configuration anywhere on the node, where they carry whatever type that
location has — `etc_t` for a hand-made `/etc/my-containerd`, say, rather than
`container_config_t`.

So the rules for those two are written against attributes,
`kata_deploy_install_target` and `kata_deploy_cri_config_target`, whose default
members are the types the default locations have. The installer reads the label
each directory actually has and, when it is not already a member, loads a module
of one line adding it:

```lisp
(typeattributeset kata_deploy_install_target (etc_t))
```

Named after the type (`kata-deploy-install-etc_t`) rather than after the release,
so two releases with different prefixes each get their own and neither takes the
other's away.

## Installing it by hand

Use `400 + policy-revision` as the priority, taking the revision from the header
of the CIL file you are installing. For revision 4, run:

```bash
semodule -X 404 -i kata-deploy.cil
```

It is CIL rather than a compiled `.pp` so the host's own `secilc` compiles it at
install time, which keeps one artifact working across `selinux-policy` versions
and distributions.

kata-deploy itself installs it at `400 + policy-revision`, the revision the file's
own header carries, because `semodule` keeps the highest-priority module of a name
and that is what stops an older image from replacing a newer release's rules on a
shared node. **Bump `policy-revision` in the same commit as any change here**, or
an older image will do exactly that. To remove:

```bash
semodule --list-modules=full | grep kata-deploy
semodule -X <priority> -r kata-deploy
```

## Changing it

The installer rules were derived from a permissive harvest of a full install *and*
uninstall cycle, in *both* deployment modes. If you add host operations to the
installer, re-harvest rather than guessing:

```bash
# 1. Unmask denials that dontaudit rules would otherwise hide, and go permissive
#    so a full cycle completes instead of dying at the first denial.
semodule -DB
setenforce 0

# 2. Run a full install and uninstall.

# 3. Read the log file explicitly. The time-filtered forms (-ts today/recent)
#    can miss records that auditd has not flushed yet.
audit2allow -i /var/log/audit/audit.log
ausearch --input /var/log/audit/audit.log -m AVC -i

# 4. Restore.
setenforce 1
semodule -B
```

Two traps worth knowing, both of which produce a policy that looks complete and
then fails:

- **`semodule -DB` matters for verification, not just harvesting.** A missing
  permission whose denial is covered by a `dontaudit` rule shows up as a silent
  functional failure with *no* AVC at all — an install that "succeeds" while
  leaving the nydus unit behind. Verify with `dontaudit` disabled.
- **Harvest installs *and* uninstalls, in both modes.** No single run is a
  superset. The uninstall path is the only place `etc_t` removal appears, because
  on install `/etc/modules-load.d/kata-containers-default.conf` is written by the
  privileged `load-kernel-modules` stage and never generates a denial. The
  rootless udev rule under `udev_rules_t` is the second instance of this, so
  harvest with a rootless shim enabled as well.

To attribute a denial to a stage, correlate its audit timestamp against each
container's `startedAt`/`finishedAt`. `comm` alone is not enough: the
`service` and `system` class denials are decided by systemd as a userspace
object manager and carry no `comm`.
