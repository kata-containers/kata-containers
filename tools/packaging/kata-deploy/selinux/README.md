# kata-deploy SELinux policy

`kata-deploy.cil` gives each kata-deploy installer stage its own least-privilege
SELinux domain, so the installer can do its host work on an SELinux-enforcing
node without running as `spc_t`.

## Why it exists

The installer's containers run with `privileged: false`, which means they run as
`container_t` and are denied the host operations they need — writing `/opt/kata`,
writing the CRI drop-in, and driving systemd. See
[issue #13751](https://github.com/kata-containers/kata-containers/issues/13751).

Scope is the **installer only**. The module defines no `filecon` rules and
relabels nothing, so `/opt/kata` keeps the labels it has today (`usr_t`,
inherited from `/opt`) and the Kata runtime sees exactly what it saw before.

## Domains

| Domain | Used by | Can |
| --- | --- | --- |
| `kata_deploy_check_t` | `host-check` | Read only, plus query the CRI unit's status over D-Bus |
| `kata_deploy_artifacts_t` | `artifacts`, `remove-artifacts` | Write `/opt/kata`, the nydus unit, `/etc/modules-load.d` |
| `kata_deploy_cri_t` | `cri`, `revert-cri` | Write the CRI config, manage the nydus unit, restart the CRI |
| `kata_deploy_node_binaries_t` | `node-binaries-install`, `node-binaries-remove` | Write `/usr/local/bin` |
| `kata_deploy_t` | the `daemonset` mode's single container | All of the above except `/usr/local/bin` |

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

```bash
semodule -i kata-deploy.cil
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

Every rule was derived from a permissive harvest of a full install *and*
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
  privileged `load-kernel-modules` stage and never generates a denial.

To attribute a denial to a stage, correlate its audit timestamp against each
container's `startedAt`/`finishedAt`. `comm` alone is not enough: the
`service` and `system` class denials are decided by systemd as a userspace
object manager and carry no `comm`.
