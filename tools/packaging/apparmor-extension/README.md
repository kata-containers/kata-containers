# Guest AppArmor policy extension

This directory holds the AppArmor policy text that ships in the
`kata-containers-apparmor-extension.img` guest extension image.

The kata-agent loads workload AppArmor profiles from this image inside the
guest. The profile *parser* is not here: `apparmor_parser` and its runtime
dependencies belong to the base image, which is what the plan in
`upstream/可组合Guest镜像AppArmor实施方案.md` calls for.

## Why an extension and not the base image

Profiles are policy, and policy changes far more often than the base
operating system does. Shipping them in the base image would mean rebuilding
and redistributing a whole rootfs for every policy change. A guest extension
image is attached at VM start and mounted read-only before the agent runs, so
it can be versioned, reviewed and replaced independently.

## Layout: the extension root is a policy root

The contents of `policy/` become the root of the mounted extension, and the
agent passes that root to `apparmor_parser` as its include search path:

```text
policy/                        ->  /run/kata-extensions/apparmor/
├── profiles/                      profiles/<name>     requested by name
├── abstractions/                  abstractions/...    #include <abstractions/...>
├── tunables/                      tunables/global     #include <tunables/...>
├── local/                         local/...           site specific includes
└── metadata/manifest              version and provenance
```

Everything a profile can include must live under this root. Sharing
`abstractions/` or `tunables/` with the base image would make the meaning of a
profile depend on whichever distribution release the base image happens to
carry, which is the coupling this layout exists to avoid.

## Profile files

A file under `profiles/` is named after the profile it is expected to define.
The agent requests a profile by name, loads `profiles/<name>`, and then
verifies from the kernel that a profile with that name is loaded and in
enforce mode. That final check is what makes the name authoritative: a file
named `kata-apparmor-test` that declares `profile something-else` will not
satisfy a request for `kata-apparmor-test`.

## What belongs here

Only reviewed policy text. Not the parser, not shared libraries, not
distribution packages.

`profiles/kata-apparmor-test` is a test profile used by the integration tests.
It is not a default policy and nothing applies it unless a workload asks for
it by name. There is deliberately no `kata-default` here yet: a default
profile is a security promise about what a workload may reach, and that needs
its own review rather than arriving as a side effect of this work.
